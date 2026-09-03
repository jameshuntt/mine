use std::io;
use std::os::unix::net::{SocketAddr, UnixListener, UnixStream};
use std::path::{Path, PathBuf};

use abut::{FramedReader, FramedWriter};

use crate::config::Config;
use crate::path::{apply_socket_mode, ensure_parent_dir, unlink_if_socket, SocketPathGuard};

/// A listening stream socket whose path is guarded.
#[derive(Debug)]
pub struct BoundListener {
    /// The listener itself, for `incoming()` or non-blocking loops.
    pub listener: UnixListener,
    guard: SocketPathGuard,
    cfg: Config,
}

impl BoundListener {
    /// The socket path.
    pub fn path(&self) -> &Path {
        self.guard.path()
    }

    /// Leave the socket file in place when this drops.
    pub fn keep_path(mut self, keep: bool) -> Self {
        self.guard.set_keep(keep);
        self
    }

    /// The listener and its path; the file is no longer guarded.
    pub fn into_parts(self) -> (UnixListener, PathBuf) {
        let path = self.guard.disarm();
        (self.listener, path)
    }

    /// Accept one connection and apply this listener's timeouts and
    /// blocking mode to it.
    pub fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        let (stream, addr) = self.listener.accept()?;
        apply_stream_opts(&stream, &self.cfg)?;
        Ok((stream, addr))
    }

    /// Accept one connection from a peer the policy admits. A connection
    /// from anyone else is closed and reported as
    /// [`io::ErrorKind::PermissionDenied`] before any byte is read.
    #[cfg(feature = "peercred")]
    pub fn accept_from(&self, admit: &crate::Admit) -> io::Result<(UnixStream, crate::Peer)> {
        let (stream, _) = self.accept()?;
        let peer = crate::peer_of(&stream)?;
        if !admit.admits(&peer) {
            drop(stream);
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("refused connection from uid {} gid {} pid {:?}", peer.uid, peer.gid, peer.pid),
            ));
        }
        Ok((stream, peer))
    }
}

/// Binds listeners and opens connections with one [`Config`].
#[derive(Clone, Debug, Default)]
pub struct StreamBuilder {
    cfg: Config,
}

impl StreamBuilder {
    /// The default config: blocking, no timeouts, owner-only socket file.
    pub fn new() -> Self {
        Self::default()
    }

    /// A specific config.
    pub fn with_config(cfg: Config) -> Self {
        Self { cfg }
    }

    /// Bind at `path`: create the parent, remove a stale socket (and only a
    /// socket), bind, set the mode, and guard the path.
    pub fn bind_listener(self, path: impl AsRef<Path>) -> io::Result<BoundListener> {
        self.cfg.validate()?;
        let path = path.as_ref().to_path_buf();
        ensure_parent_dir(&path)?;
        unlink_if_socket(&path)?;
        let listener = UnixListener::bind(&path)?;
        let guard = SocketPathGuard::new(path.clone());
        listener.set_nonblocking(self.cfg.nonblocking)?;
        if let Some(mode) = self.cfg.socket_mode {
            apply_socket_mode(&path, mode)?;
        }
        Ok(BoundListener { listener, guard, cfg: self.cfg })
    }

    /// Connect to the socket at `path` with this config's options.
    pub fn connect(self, path: impl AsRef<Path>) -> io::Result<UnixStream> {
        self.cfg.validate()?;
        let stream = UnixStream::connect(path.as_ref())?;
        apply_stream_opts(&stream, &self.cfg)?;
        Ok(stream)
    }
}

/// Split a stream into a framed writer and a framed reader, each owning a
/// handle to the same connection.
pub fn framed(stream: UnixStream) -> io::Result<(FramedWriter<UnixStream>, FramedReader<UnixStream>)> {
    let reader = FramedReader::new(stream.try_clone()?);
    Ok((FramedWriter::new(stream), reader))
}

fn apply_stream_opts(stream: &UnixStream, cfg: &Config) -> io::Result<()> {
    stream.set_nonblocking(cfg.nonblocking)?;
    stream.set_read_timeout(cfg.read_timeout)?;
    stream.set_write_timeout(cfg.write_timeout)?;
    Ok(())
}
