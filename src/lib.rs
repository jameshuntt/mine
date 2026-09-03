//! A Unix domain socket that is yours alone.
//!
//! A socket path is a file, and files have the usual problems: something
//! else may already be there, the mode bits may let any local user connect,
//! and once a client is in nothing says who it is. This crate closes those
//! three gaps and stops there.
//!
//! * **The path is treated with care.** Binding refuses to remove anything
//!   at the path that is not a socket (a symlink or a regular file is an
//!   error, not a casualty), the parent directory is created, the socket
//!   file is chmod'ed to owner-only by default, and a [`SocketPathGuard`]
//!   removes it when the listener drops.
//! * **The peer is named before it is trusted** (feature `peercred`, on by
//!   default). [`peer_of`] reads the uid, gid and, where the OS offers it,
//!   the pid of the process on the other end, and
//!   [`BoundListener::accept_from`] refuses a connection that an [`Admit`]
//!   policy does not cover before a single byte is read.
//! * **Frames, not streams.** [`FramedReader`] and [`FramedWriter`] come
//!   from [`abut`]: a length prefix, a maximum frame size, and a stream that
//!   stays aligned after a refused frame. With feature `classified`, a frame
//!   can be received straight into a zeroizing buffer.
//!
//! ```no_run
//! use mine::{StreamBuilder, FramedReader, FramedWriter};
//!
//! let bound = StreamBuilder::new().bind_listener("/run/user/1000/myapp/control.sock")?;
//! let (stream, _addr) = bound.accept()?;                   // or accept_from(&Admit::me()) with peercred
//! let mut reader = FramedReader::new(stream.try_clone()?);
//! let mut writer = FramedWriter::new(stream);
//! let mut frame = Vec::new();
//! reader.recv_into(&mut frame)?;
//! writer.write_frame(b"ok")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Datagram sockets get the same path handling through [`DatagramBuilder`].
//!
//! Socket operations return `io::Error` so the kinds stay matchable; when the
//! refusal is this crate's own (a symlink at the path, a peer outside the
//! policy, a bad config) the error carries a [`MineCode`] rendered by
//! `liaise` as `[MINE0004] Refused connection from uid 1000 gid 1000`, and
//! [`code_of`] gets the typed code back out.
//!
//! What is not here: message schemas, request routing, TCP. Unix only.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

// The `liaise` derive expands to paths under `alloc::`.
extern crate alloc;

#[cfg(not(unix))]
compile_error!("mine is a Unix domain socket crate and builds on Unix only");

mod config;
mod datagram;
mod error;
mod path;
#[cfg(feature = "peercred")]
mod peer;
mod stream;

#[cfg(feature = "classified")]
pub mod classified_frames;

pub use abut;
pub use abut::{AbutCode, AbutError, FrameSink, FrameSource, FramedReader, FramedWriter, ReaderConfig};
pub use liaise;

pub use error::{code_of, MineCode};

pub use config::{Config, ConfigBuilder};
pub use datagram::{BoundDatagram, DatagramBuilder, DatagramSink, DatagramSource};
pub use path::{
    apply_socket_mode, default_socket_path, ensure_parent_dir, ensure_runtime_subdir, runtime_dir, unlink_if_socket,
    SocketPathGuard,
};
#[cfg(feature = "peercred")]
pub use peer::{peer_of, Admit, Peer};
pub use stream::{framed, BoundListener, StreamBuilder};

#[cfg(all(doctest, feature = "peercred"))]
#[doc = include_str!("../README.md")]
mod readme_doctests {}
