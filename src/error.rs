//! The crate's own refusals, as stable codes.
//!
//! Socket operations return [`std::io::Error`] so callers keep matching on
//! [`io::ErrorKind`](std::io::ErrorKind). When the refusal is this crate's
//! decision rather than the kernel's, the error carries a [`MineCode`]: its
//! message renders as `[MINE0004] Refused connection from uid 1000 gid 1000`,
//! and the typed code is reachable through
//! [`io::Error::get_ref`](std::io::Error::get_ref) and `downcast_ref`.

use std::io;

use liaise::{Liaise, LiaiseCodes};

/// Why this crate refused, as a code the `liaise` crate renders.
#[derive(LiaiseCodes, Debug, Clone, PartialEq, Eq)]
#[liaise(prefix = "MINE")]
pub enum MineCode {
    /// A symlink sits at the socket path; it is neither followed nor removed.
    #[liaise(code = 1, msg = "Refusing to unlink a symlink at the socket path {path}")]
    RefusedSymlink {
        /// The path in question.
        path: String,
    },

    /// Something that is not a socket sits at the socket path.
    #[liaise(code = 2, msg = "{path} exists and is not a unix socket")]
    NotASocket {
        /// The path in question.
        path: String,
    },

    /// A config asked for non-blocking sockets and timeouts at once.
    #[liaise(code = 3, msg = "nonblocking is incompatible with read_timeout and write_timeout")]
    NonblockingWithTimeout,

    /// A connecting process was not covered by the [`Admit`](crate::Admit) policy.
    #[liaise(code = 4, msg = "Refused connection from uid {uid} gid {gid}")]
    PeerRefused {
        /// The peer's effective uid.
        uid: u32,
        /// The peer's effective gid.
        gid: u32,
    },

    /// A datagram went out shorter than the frame.
    #[liaise(code = 5, msg = "Short datagram send: {sent} of {len} bytes")]
    ShortDatagram {
        /// Bytes the kernel accepted.
        sent: usize,
        /// Bytes in the frame.
        len: usize,
    },
}

/// An `io::Error` of `kind` carrying `code` as its payload.
pub(crate) fn refuse(kind: io::ErrorKind, code: MineCode) -> io::Error {
    io::Error::new(kind, code)
}

/// The [`MineCode`] inside an `io::Error`, if this crate put one there.
///
/// ```
/// use std::io;
/// use mine::{code_of, ConfigBuilder, MineCode};
///
/// let err = ConfigBuilder::new().nonblocking(true).read_timeout(std::time::Duration::from_secs(1)).build().unwrap_err();
/// assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
/// assert_eq!(code_of(&err), Some(&MineCode::NonblockingWithTimeout));
/// assert_eq!(err.to_string(), "[MINE0003] nonblocking is incompatible with read_timeout and write_timeout");
/// ```
pub fn code_of(err: &io::Error) -> Option<&MineCode> {
    err.get_ref().and_then(|inner| inner.downcast_ref::<MineCode>())
}
