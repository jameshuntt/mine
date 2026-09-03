//! Who is on the other end of a connection (feature `peercred`).
//!
//! The kernel knows the credentials of the process that connected to a Unix
//! socket, and lies to no one about them. Linux and Android report pid,
//! uid and gid; macOS and iOS report uid, gid and pid; the other BSDs report
//! uid and gid.

use std::io;
use std::os::unix::net::UnixStream;

/// The credentials of the process at the other end of a stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Peer {
    /// Effective user id.
    pub uid: u32,
    /// Effective group id.
    pub gid: u32,
    /// Process id, where the OS reports it.
    pub pid: Option<i32>,
}

/// The credentials of the peer of `stream`.
pub fn peer_of(stream: &UnixStream) -> io::Result<Peer> {
    platform::peer_of(stream)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod platform {
    use super::*;
    use nix::sys::socket::{getsockopt, sockopt::PeerCredentials};

    pub fn peer_of(stream: &UnixStream) -> io::Result<Peer> {
        let cred = getsockopt(stream, PeerCredentials).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(Peer { uid: cred.uid(), gid: cred.gid(), pid: Some(cred.pid()) })
    }
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
mod platform {
    use super::*;
    use nix::sys::socket::{getsockopt, sockopt::LocalPeerCred, sockopt::LocalPeerPid};

    pub fn peer_of(stream: &UnixStream) -> io::Result<Peer> {
        let cred = getsockopt(stream, LocalPeerCred).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        let gid = cred.groups().first().copied().unwrap_or(u32::MAX);
        let pid = getsockopt(stream, LocalPeerPid).ok();
        Ok(Peer { uid: cred.uid(), gid, pid })
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android", target_os = "macos", target_os = "ios")))]
mod platform {
    use super::*;

    pub fn peer_of(stream: &UnixStream) -> io::Result<Peer> {
        let (uid, gid) = nix::unistd::getpeereid(stream).map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        Ok(Peer { uid: uid.as_raw(), gid: gid.as_raw(), pid: None })
    }
}

/// Which peers a listener accepts.
///
/// A peer is admitted when its uid is listed, or its gid is listed. The
/// socket file's mode already keeps most strangers out; this is the check
/// that does not depend on the filesystem, and it is what says "mine".
///
/// ```
/// use mine::{Admit, Peer};
///
/// let policy = Admit::me().also_gid(27);
/// let me = Peer { uid: policy.uids()[0], gid: 1, pid: None };
/// let staff = Peer { uid: 9999, gid: 27, pid: None };
/// let stranger = Peer { uid: 9999, gid: 9999, pid: None };
/// assert!(policy.admits(&me));
/// assert!(policy.admits(&staff));
/// assert!(!policy.admits(&stranger));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Admit {
    uids: Vec<u32>,
    gids: Vec<u32>,
}

impl Admit {
    /// Only this process's effective uid.
    pub fn me() -> Self {
        Self { uids: vec![nix::unistd::geteuid().as_raw()], gids: Vec::new() }
    }

    /// Only the given uid.
    pub fn uid(uid: u32) -> Self {
        Self { uids: vec![uid], gids: Vec::new() }
    }

    /// Only the given gid.
    pub fn gid(gid: u32) -> Self {
        Self { uids: Vec::new(), gids: vec![gid] }
    }

    /// Also admit this uid.
    pub fn also_uid(mut self, uid: u32) -> Self {
        self.uids.push(uid);
        self
    }

    /// Also admit this gid.
    pub fn also_gid(mut self, gid: u32) -> Self {
        self.gids.push(gid);
        self
    }

    /// The admitted uids.
    pub fn uids(&self) -> &[u32] {
        &self.uids
    }

    /// The admitted gids.
    pub fn gids(&self) -> &[u32] {
        &self.gids
    }

    /// Whether `peer` is admitted.
    pub fn admits(&self, peer: &Peer) -> bool {
        self.uids.contains(&peer.uid) || self.gids.contains(&peer.gid)
    }
}
