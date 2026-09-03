//! A framed echo server and its client over one socket path.
//!
//! cargo run --example echo

use mine::{default_socket_path, framed, StreamBuilder};

fn main() -> std::io::Result<()> {
    let path = default_socket_path("mine-example", "echo");
    let bound = StreamBuilder::new().bind_listener(&path)?;
    println!("listening on {} (owner-only)", bound.path().display());

    let server = std::thread::spawn(move || -> std::io::Result<()> {
        let (stream, _) = bound.accept()?;
        let (mut writer, mut reader) = framed(stream)?;
        let mut frame = Vec::new();
        for _ in 0..3 {
            reader.recv_into(&mut frame).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
            frame.make_ascii_uppercase();
            writer.write_frame(&frame).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }
        Ok(())
    });

    let stream = StreamBuilder::new().connect(&path)?;
    let (mut writer, mut reader) = framed(stream)?;
    let mut reply = Vec::new();
    for msg in ["hello", "framed", "world"] {
        writer.write_frame(msg.as_bytes()).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        reader.recv_into(&mut reply).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        println!("{msg} -> {}", String::from_utf8_lossy(&reply));
    }
    server.join().expect("server thread")?;
    println!("socket file removed: {}", !path.exists());
    Ok(())
}
