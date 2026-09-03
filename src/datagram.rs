use std::io;
use std::os::unix::net::UnixDatagram;
use std::path::{Path, PathBuf};

use abut::{FrameSink, FrameSource};

use crate::config::Config;
use crate::error::{refuse, MineCode};
use crate::path::{apply_socket_mode, ensure_parent_dir, unlink_if_socket, SocketPathGuard};

/// A bound datagram socket whose path is guarded.
#[derive(Debug)]
pub struct BoundDatagram {
    /// The socket itself.
    pub sock: UnixDatagram,
    guard: SocketPathGuard,
}

impl BoundDatagram {
    /// The socket path.
    pub fn path(&self) -> &Path {
        self.guard.path()
    }

    /// Leave the socket file in place when this drops.
    pub fn keep_path(mut self, keep: bool) -> Self {
        self.guard.set_keep(keep);
        self
    }

    /// The socket and its path; the file is no longer guarded.
    pub fn into_parts(self) -> (UnixDatagram, PathBuf) {
        let path = self.guard.disarm();
        (self.sock, path)
    }
}

/// Binds and connects datagram sockets with one [`Config`].
#[derive(Clone, Debug, Default)]
pub struct DatagramBuilder {
    cfg: Config,
}

impl DatagramBuilder {
    /// The default config.
    pub fn new() -> Self {
        Self::default()
    }

    /// A specific config.
    pub fn with_config(cfg: Config) -> Self {
        Self { cfg }
    }

    /// Bind at `path` with the same path care as a stream listener.
    pub fn bind(self, path: impl AsRef<Path>) -> io::Result<BoundDatagram> {
        self.cfg.validate()?;
        let path = path.as_ref().to_path_buf();
        ensure_parent_dir(&path)?;
        unlink_if_socket(&path)?;
        let sock = UnixDatagram::bind(&path)?;
        let guard = SocketPathGuard::new(path.clone());
        apply_datagram_opts(&sock, &self.cfg)?;
        if let Some(mode) = self.cfg.socket_mode {
            apply_socket_mode(&path, mode)?;
        }
        Ok(BoundDatagram { sock, guard })
    }

    /// An unbound socket connected to `dest`: it can send, and only `dest`
    /// can be its peer.
    pub fn connect(self, dest: impl AsRef<Path>) -> io::Result<UnixDatagram> {
        self.cfg.validate()?;
        let sock = UnixDatagram::unbound()?;
        sock.connect(dest.as_ref())?;
        apply_datagram_opts(&sock, &self.cfg)?;
        Ok(sock)
    }

    /// A socket bound at `local` and connected to `dest`, so replies have
    /// somewhere to go. The local path is guarded like any bound socket.
    pub fn connect_bound(self, local: impl AsRef<Path>, dest: impl AsRef<Path>) -> io::Result<BoundDatagram> {
        let bound = self.bind(local)?;
        bound.sock.connect(dest.as_ref())?;
        Ok(bound)
    }
}

fn apply_datagram_opts(sock: &UnixDatagram, cfg: &Config) -> io::Result<()> {
    sock.set_nonblocking(cfg.nonblocking)?;
    sock.set_read_timeout(cfg.read_timeout)?;
    sock.set_write_timeout(cfg.write_timeout)?;
    Ok(())
}

/// A connected datagram socket as a [`FrameSink`]: one datagram per frame.
#[derive(Debug)]
pub struct DatagramSink {
    sock: UnixDatagram,
}

impl DatagramSink {
    /// Connect to `dest`.
    pub fn connect(dest: impl AsRef<Path>) -> io::Result<Self> {
        Ok(Self { sock: DatagramBuilder::new().connect(dest)? })
    }

    /// Wrap an already connected socket.
    pub fn from_socket(sock: UnixDatagram) -> Self {
        Self { sock }
    }

    /// The socket back.
    pub fn into_inner(self) -> UnixDatagram {
        self.sock
    }
}

impl FrameSink for DatagramSink {
    type Error = io::Error;

    fn send_frame(&mut self, bytes: &[u8]) -> Result<(), io::Error> {
        let sent = self.sock.send(bytes)?;
        if sent != bytes.len() {
            return Err(refuse(io::ErrorKind::WriteZero, MineCode::ShortDatagram { sent, len: bytes.len() }));
        }
        Ok(())
    }
}

/// A datagram socket as a [`FrameSource`]: one frame per datagram.
#[derive(Debug)]
pub struct DatagramSource {
    sock: UnixDatagram,
}

impl DatagramSource {
    /// Wrap a bound socket.
    pub fn from_socket(sock: UnixDatagram) -> Self {
        Self { sock }
    }

    /// The socket back.
    pub fn into_inner(self) -> UnixDatagram {
        self.sock
    }
}

impl FrameSource for DatagramSource {
    type Error = io::Error;

    fn recv_frame(&mut self, dst: &mut [u8]) -> Result<usize, io::Error> {
        self.sock.recv(dst)
    }
}
