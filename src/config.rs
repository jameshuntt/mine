use std::{io, time::Duration};

use crate::error::{refuse, MineCode};

/// How a socket is opened.
///
/// The default is blocking, no timeouts, and a socket file mode of `0o600`:
/// only the owner may connect. Loosen the mode on purpose
/// (`0o660` for a group) rather than by accident.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// Put the socket in non-blocking mode. Incompatible with the timeouts.
    pub nonblocking: bool,
    /// Read timeout applied to every stream this config opens.
    pub read_timeout: Option<Duration>,
    /// Write timeout applied to every stream this config opens.
    pub write_timeout: Option<Duration>,
    /// Mode bits set on the socket file after bind; `None` leaves the umask's result.
    pub socket_mode: Option<u32>,
}

impl Default for Config {
    fn default() -> Self {
        Self { nonblocking: false, read_timeout: None, write_timeout: None, socket_mode: Some(0o600) }
    }
}

impl Config {
    /// Refuse a nonblocking socket with timeouts: the OS ignores one of them.
    /// The error is [`io::ErrorKind::InvalidInput`] carrying
    /// [`MineCode::NonblockingWithTimeout`].
    pub fn validate(&self) -> io::Result<()> {
        if self.nonblocking && (self.read_timeout.is_some() || self.write_timeout.is_some()) {
            return Err(refuse(io::ErrorKind::InvalidInput, MineCode::NonblockingWithTimeout));
        }
        Ok(())
    }
}

/// Builds a [`Config`].
#[derive(Clone, Debug, Default)]
pub struct ConfigBuilder {
    cfg: Config,
}

impl ConfigBuilder {
    /// Start from the defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Non-blocking sockets.
    pub fn nonblocking(mut self, value: bool) -> Self {
        self.cfg.nonblocking = value;
        self
    }

    /// A read timeout for streams.
    pub fn read_timeout(mut self, value: Duration) -> Self {
        self.cfg.read_timeout = Some(value);
        self
    }

    /// A write timeout for streams.
    pub fn write_timeout(mut self, value: Duration) -> Self {
        self.cfg.write_timeout = Some(value);
        self
    }

    /// Mode bits for the socket file, e.g. `0o660`.
    pub fn socket_mode(mut self, mode: u32) -> Self {
        self.cfg.socket_mode = Some(mode);
        self
    }

    /// Leave the socket file's mode to the umask.
    pub fn umask_mode(mut self) -> Self {
        self.cfg.socket_mode = None;
        self
    }

    /// The config, validated.
    pub fn build(self) -> io::Result<Config> {
        self.cfg.validate()?;
        Ok(self.cfg)
    }
}
