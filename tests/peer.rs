//! Peer credentials and the admit policy (feature `peercred`).
#![cfg(feature = "peercred")]

use std::io::{self, Read};
use std::path::PathBuf;

use mine::{code_of, peer_of, Admit, MineCode, Peer, StreamBuilder};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mine-peer-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn my_uid() -> u32 {
    Admit::me().uids()[0]
}

#[test]
fn the_peer_of_a_local_connection_is_this_process() {
    let path = scratch("who").join("who.sock");
    let bound = StreamBuilder::new().bind_listener(&path).unwrap();
    let client = StreamBuilder::new().connect(&path).unwrap();
    let (server_side, _) = bound.accept().unwrap();

    let peer = peer_of(&server_side).unwrap();
    assert_eq!(peer.uid, my_uid());
    if let Some(pid) = peer.pid {
        assert_eq!(pid, std::process::id() as i32);
    }
    // symmetric: the client sees the server, which is also us
    assert_eq!(peer_of(&client).unwrap().uid, my_uid());
}

#[test]
fn accept_from_admits_me_and_refuses_a_stranger_policy() {
    let path = scratch("admit").join("admit.sock");
    let bound = StreamBuilder::new().bind_listener(&path).unwrap();

    let client = StreamBuilder::new().connect(&path).unwrap();
    let (stream, peer) = bound.accept_from(&Admit::me()).unwrap();
    assert_eq!(peer.uid, my_uid());
    drop((stream, client));

    // a policy that names someone else: the connection is closed before any read
    let mut client = StreamBuilder::new().connect(&path).unwrap();
    let err = bound.accept_from(&Admit::uid(u32::MAX - 1)).unwrap_err();
    assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
    assert!(err.to_string().starts_with(&format!("[MINE0004] Refused connection from uid {}", my_uid())), "{err}");
    assert!(matches!(code_of(&err), Some(MineCode::PeerRefused { uid, .. }) if *uid == my_uid()));
    let mut byte = [0u8; 1];
    assert_eq!(client.read(&mut byte).unwrap(), 0, "the client sees the connection closed");

    // a group policy admits by gid
    let (_, me) = {
        let _client = StreamBuilder::new().connect(&path).unwrap();
        bound.accept_from(&Admit::me()).unwrap()
    };
    let by_group = Admit::gid(me.gid);
    assert!(by_group.admits(&me));
    assert_eq!(by_group.gids(), &[me.gid]);
    let nobody = Peer { uid: u32::MAX - 1, gid: u32::MAX - 1, pid: None };
    assert!(!by_group.admits(&nobody));
    assert!(Admit::uid(1).also_uid(nobody.uid).admits(&nobody));
    assert!(!Admit::default().admits(&me), "an empty policy admits no one");
}
