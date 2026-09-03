# mine

A Unix domain socket that is yours alone.

A socket path is a file, and files have the usual problems: something else
may already be there, the mode bits may let any local user connect, and
once a client is in nothing says who it is. This crate closes those three
gaps and stops there.

- **The path is treated with care.** Binding refuses to remove anything at
  the path that is not a socket (a symlink or a regular file is an error,
  not a casualty), creates the parent directory, sets the socket file to
  owner-only by default, and removes it again when the listener drops.
- **The peer is named before it is trusted** (feature `peercred`, on by
  default). The kernel reports the uid, gid and, where the OS offers it,
  the pid of the connecting process; `accept_from` refuses a connection an
  `Admit` policy does not cover before a single byte is read.
- **Frames, not streams.** `FramedReader` and `FramedWriter` come from
  [`abut`](https://crates.io/crates/abut): a length prefix, a maximum frame
  size, and a stream that stays aligned after a refused frame. With feature
  `classified`, a frame can be received straight into a zeroizing
  [`classified`](https://crates.io/crates/classified) buffer.

```rust
use mine::{Admit, StreamBuilder, framed};

let path = std::env::temp_dir().join(format!("mine-readme-{}.sock", std::process::id()));
let bound = StreamBuilder::new().bind_listener(&path)?;      // parent made, mode 0600, path guarded

let server = std::thread::spawn(move || -> std::io::Result<()> {
    let (stream, peer) = bound.accept_from(&Admit::me())?;   // same uid as us, or refused
    assert_eq!(peer.uid, Admit::me().uids()[0]);
    let (mut writer, mut reader) = framed(stream)?;
    let mut frame = Vec::new();
    reader.recv_into(&mut frame).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    writer.write_frame(b"ok").map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok(())
});

let (mut writer, mut reader) = framed(StreamBuilder::new().connect(&path)?)?;
writer.write_frame(b"hello").map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
let mut reply = Vec::new();
reader.recv_into(&mut reply).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
assert_eq!(reply, b"ok");
server.join().unwrap()?;
assert!(!path.exists(), "removed when the listener dropped");
# Ok::<(), std::io::Error>(())
```

## What is here

| item | does |
|---|---|
| `StreamBuilder`, `BoundListener` | bind and connect stream sockets with one `Config`; `accept`, `accept_from` |
| `DatagramBuilder`, `BoundDatagram`, `DatagramSink`, `DatagramSource` | the same for datagrams, one datagram per frame |
| `Config`, `ConfigBuilder` | blocking mode, timeouts, socket file mode (`0o600` by default) |
| `unlink_if_socket`, `SocketPathGuard`, `runtime_dir`, `default_socket_path` | the path rules on their own |
| `peer_of`, `Peer`, `Admit` | who connected, and who is allowed to |
| `framed` | split a stream into an abut writer and reader |
| `classified_frames::{recv_classified, send_classified}` | frames as zeroizing buffers (feature `classified`) |

Peer credentials: Linux and Android report pid, uid and gid; macOS and iOS
report uid, gid and pid; the other BSDs report uid and gid.

## What is not here

Message schemas, request routing, async runtimes, TCP. Unix only.

## License

MIT OR Apache-2.0.
