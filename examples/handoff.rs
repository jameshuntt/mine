//! Hand a secret to another process: it leaves a container, crosses the
//! socket as one frame, and lands in a container on the other side.
//!
//! cargo run --example handoff --features classified

use classified::ClassifiedBuffer;
use mine::classified_frames::{recv_classified, send_classified};
use mine::{default_socket_path, framed, StreamBuilder};

fn main() -> std::io::Result<()> {
    let path = default_socket_path("mine-example", "handoff");
    let bound = StreamBuilder::new().bind_listener(&path)?;

    let receiver = std::thread::spawn(move || -> std::io::Result<()> {
        let (stream, _) = bound.accept()?;
        let (_, mut reader) = framed(stream)?;
        let secret = recv_classified(&mut reader).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        println!("received {secret:?}");
        println!("it has {} bytes; the log line above did not print them", secret.len());
        Ok(())
    });

    let key = ClassifiedBuffer::try_from_slice(&[0x4b; 32], 32).unwrap();
    let stream = StreamBuilder::new().connect(&path)?;
    let (mut writer, _) = framed(stream)?;
    send_classified(&mut writer, &key).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    receiver.join().expect("receiver thread")?;
    Ok(())
}
