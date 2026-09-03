//! Name the peer before trusting it: accept only connections from this uid.
//!
//! cargo run --example owner_only

use mine::{default_socket_path, framed, Admit, StreamBuilder};

fn main() -> std::io::Result<()> {
    let path = default_socket_path("mine-example", "owner-only");
    let bound = StreamBuilder::new().bind_listener(&path)?;

    let server = std::thread::spawn(move || -> std::io::Result<()> {
        // admitted: same uid as this process
        let (stream, peer) = bound.accept_from(&Admit::me())?;
        println!("admitted uid {} gid {} pid {:?}", peer.uid, peer.gid, peer.pid);
        let (mut writer, _) = framed(stream)?;
        writer.write_frame(b"welcome").map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // refused: a policy naming a uid that is not ours, closed before any read
        match bound.accept_from(&Admit::uid(u32::MAX - 1)) {
            Err(e) => println!("second connection: {e}"),
            Ok(_) => println!("second connection admitted (unexpected)"),
        }
        Ok(())
    });

    let first = StreamBuilder::new().connect(&path)?;
    let (_, mut reader) = framed(first)?;
    let mut frame = Vec::new();
    reader.recv_into(&mut frame).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    println!("client got {:?}", String::from_utf8_lossy(&frame));

    let _second = StreamBuilder::new().connect(&path)?;
    server.join().expect("server thread")?;
    Ok(())
}
