//! Path care, listener and datagram behaviour, frames across a socket.

use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

use mine::{
    code_of, default_socket_path, ensure_runtime_subdir, framed, unlink_if_socket, AbutCode, Config, ConfigBuilder,
    DatagramBuilder, DatagramSink, DatagramSource, FrameSink, FrameSource, FramedReader, MineCode, ReaderConfig,
    SocketPathGuard, StreamBuilder,
};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mine-tests-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_bind_refuses_to_destroy_anything_that_is_not_a_socket() {
    let dir = scratch("path");
    let file = dir.join("file");
    std::fs::write(&file, b"keep me").unwrap();
    let err = unlink_if_socket(&file).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(code_of(&err), Some(&MineCode::NotASocket { path: file.display().to_string() }));
    assert!(err.to_string().starts_with("[MINE0002] "), "{err}");
    assert_eq!(StreamBuilder::new().bind_listener(&file).unwrap_err().kind(), io::ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&file).unwrap(), b"keep me");

    let link = dir.join("link");
    std::os::unix::fs::symlink(&file, &link).unwrap();
    let err = unlink_if_socket(&link).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
    assert!(matches!(code_of(&err), Some(MineCode::RefusedSymlink { .. })));
    assert!(err.to_string().starts_with("[MINE0001] Refusing to unlink a symlink"), "{err}");
    assert!(link.exists(), "the symlink was not followed or removed");

    assert!(unlink_if_socket(&dir.join("absent")).is_ok(), "nothing there is fine");
}

#[test]
fn a_stale_socket_is_replaced_and_the_guard_removes_the_path() {
    let dir = scratch("stale");
    let path = dir.join("app.sock");
    {
        let bound = StreamBuilder::new().bind_listener(&path).unwrap().keep_path(true);
        assert_eq!(bound.path(), path);
    }
    assert!(path.exists(), "kept on purpose, so it is now stale");
    let bound = StreamBuilder::new().bind_listener(&path).unwrap();
    assert!(path.exists());
    drop(bound);
    assert!(!path.exists(), "removed on drop");

    let mut guard = SocketPathGuard::new(path.clone());
    guard.set_keep(true);
    let back = guard.disarm();
    assert_eq!(back, path);
}

#[test]
fn the_socket_file_is_owner_only_by_default() {
    let dir = scratch("mode");
    let path = dir.join("owner.sock");
    let bound = StreamBuilder::new().bind_listener(&path).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    drop(bound);

    let cfg = ConfigBuilder::new().socket_mode(0o660).build();
    let bound = StreamBuilder::with_config(cfg).bind_listener(&path).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o660);
    drop(bound);
}

#[test]
fn config_refuses_nonblocking_with_timeouts() {
    let bad = ConfigBuilder::new().nonblocking(true).read_timeout(Duration::from_millis(5)).build();
    let err = bad.validate().unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(code_of(&err), Some(&MineCode::NonblockingWithTimeout));
    assert_eq!(err.to_string(), "[MINE0003] nonblocking is incompatible with read_timeout and write_timeout");
    assert_eq!(code_of(&io::Error::new(io::ErrorKind::Other, "not ours")), None);
    // and a bind with it is refused the same way
    let dir = scratch("badcfg");
    let err = StreamBuilder::with_config(bad).bind_listener(dir.join("x.sock")).unwrap_err();
    assert_eq!(code_of(&err), Some(&MineCode::NonblockingWithTimeout));

    let cfg = ConfigBuilder::new().umask_mode().write_timeout(Duration::from_secs(1)).build();
    assert_eq!(cfg.socket_mode, None);
    assert_eq!(Config::default().socket_mode, Some(0o600));
}

#[test]
fn frames_cross_a_stream_socket_and_a_timeout_is_honoured() {
    let dir = scratch("stream");
    let path = dir.join("echo.sock");
    let bound = StreamBuilder::new().bind_listener(&path).unwrap();
    let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();

    let server = std::thread::spawn(move || {
        let (stream, _) = bound.accept().unwrap();
        let (mut writer, mut reader) = framed(stream).unwrap();
        let mut frame = Vec::new();
        reader.recv_into(&mut frame).unwrap();
        frame.reverse();
        writer.write_frame(&frame).unwrap();
        // a second, oversize frame: refused by the client's reader
        writer.write_frame(&[7u8; 100]).unwrap();
        // stay connected, silent, until the client has seen its read time out
        let _ = done_rx.recv();
    });

    let cfg = ConfigBuilder::new().read_timeout(Duration::from_millis(200)).build();
    let stream = StreamBuilder::with_config(cfg).connect(&path).unwrap();
    let (mut writer, mut reader) = framed(stream).unwrap();
    writer.write_frame(b"abc").unwrap();
    let mut frame = Vec::new();
    reader.recv_into(&mut frame).unwrap();
    assert_eq!(frame, b"cba");

    // a reader that refuses frames over 16 bytes but drains a refused frame
    // of up to 128, so the stream stays aligned
    let cfg = ReaderConfig { max_frame_len: 16, drain_oversize_up_to: 128, ..Default::default() };
    let mut small = FramedReader::with_config(reader.into_inner(), cfg);
    let refused = small.recv_into(&mut frame).unwrap_err();
    assert!(matches!(refused.code, AbutCode::FrameTooLarge { len: 100, max: 16 }));

    // the refused frame was drained and the server is silent: the read
    // timeout turns into an error instead of a hang
    let mut raw = small.into_inner();
    let mut byte = [0u8; 1];
    let err = raw.read(&mut byte).unwrap_err();
    assert!(matches!(err.kind(), io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut), "{err:?}");
    done_tx.send(()).unwrap();
    server.join().unwrap();
}

#[test]
fn datagrams_are_frames() {
    let dir = scratch("dgram");
    let path = dir.join("log.sock");
    let bound = DatagramBuilder::new().bind(&path).unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);

    let mut sink = DatagramSink::connect(&path).unwrap();
    sink.send_frame(b"one").unwrap();
    sink.send_frame(b"two").unwrap();

    let (sock, kept) = bound.into_parts();
    let mut source = DatagramSource::from_socket(sock);
    let mut buf = [0u8; 8];
    let n = source.recv_frame(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"one");
    let n = source.recv_frame(&mut buf).unwrap();
    assert_eq!(&buf[..n], b"two");
    assert!(kept.exists(), "into_parts leaves the file to the caller");
    std::fs::remove_file(kept).unwrap();

    // a bound reply socket, connected to a fresh target
    let target = DatagramBuilder::new().bind(dir.join("target.sock")).unwrap();
    let reply = DatagramBuilder::new().connect_bound(dir.join("reply.sock"), target.path()).unwrap();
    assert!(reply.path().exists());
    reply.sock.send(b"x").unwrap();
    let mut buf = [0u8; 4];
    assert_eq!(target.sock.recv(&mut buf).unwrap(), 1);
    drop(reply);
    assert!(!dir.join("reply.sock").exists());
    drop(sink);
}

#[test]
fn runtime_paths_have_the_documented_shape() {
    let p = default_socket_path("myapp", "control");
    assert!(p.ends_with("myapp/control.sock"), "{}", p.display());
    let dir = ensure_runtime_subdir("mine-test-subdir", 0o700).unwrap();
    assert!(dir.is_dir());
    assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
    std::fs::remove_dir_all(dir).unwrap();
}
