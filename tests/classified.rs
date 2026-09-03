//! Frames into and out of classified containers (feature `classified`).
#![cfg(feature = "classified")]

use std::path::PathBuf;

use classified::ClassifiedBuffer;
use mine::classified_frames::{recv_classified, send_classified, RecvError};
use mine::{framed, StreamBuilder};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mine-classified-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn a_secret_crosses_the_socket_and_lands_in_a_container() {
    let path = scratch("secret").join("key.sock");
    let bound = StreamBuilder::new().bind_listener(&path).unwrap();
    let secret = ClassifiedBuffer::try_from_slice(b"session key material", 64).unwrap();

    let server = std::thread::spawn(move || {
        let (stream, _) = bound.accept().unwrap();
        let (mut writer, mut reader) = framed(stream).unwrap();
        let received = recv_classified(&mut reader).unwrap();
        assert_eq!(received.capacity_limit(), 20, "bounded to its own length");
        assert_eq!(format!("{received:?}"), r#"ClassifiedBuffer { len: 20, capacity_limit: 20, value: "[REDACTED]" }"#);
        // echo it back, then an empty frame
        send_classified(&mut writer, &received).unwrap();
        writer.write_frame(b"").unwrap();
    });

    let stream = StreamBuilder::new().connect(&path).unwrap();
    let (mut writer, mut reader) = framed(stream).unwrap();
    send_classified(&mut writer, &secret).unwrap();
    let back = recv_classified(&mut reader).unwrap();
    assert!(back.ct_eq(&secret));
    let empty = recv_classified(&mut reader).unwrap_err();
    assert!(matches!(empty, RecvError::Empty(_)), "{empty}");
    assert_eq!(empty.to_string(), "[MINE0011] Empty frame refused by the container");
    let source = std::error::Error::source(&empty).expect("the container's error is the source");
    assert_eq!(source.to_string(), "classified value must not be empty");
    server.join().unwrap();
}
