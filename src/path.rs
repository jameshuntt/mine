//! The socket path: where it goes, what may be removed there, and who may
//! open it.

use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::{fs, io};

/// Create the parent directory of `path` if it is missing.
pub fn ensure_parent_dir(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Remove whatever is at `path`, but only if it is a Unix socket.
///
/// A stale socket from a previous run is removed. A symlink is refused
/// without following it, and a regular file or directory is refused, both
/// with [`io::ErrorKind::AlreadyExists`]: a bind must never destroy
/// something that is not a socket. Nothing at the path is fine.
pub fn unlink_if_socket(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
        Ok(meta) => {
            let kind = meta.file_type();
            if kind.is_symlink() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("refusing to unlink a symlink at the socket path {}", path.display()),
                ));
            }
            if kind.is_socket() {
                fs::remove_file(path)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{} exists and is not a unix socket", path.display()),
                ))
            }
        }
    }
}

/// Set the mode bits of a bound socket file, e.g. `0o600` for owner-only.
pub fn apply_socket_mode(path: &Path, mode: u32) -> io::Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(mode))
}

/// Removes the socket file when dropped, unless told to keep it.
///
/// Every bound listener and datagram holds one, so a normal shutdown leaves
/// no stale socket behind. Call [`set_keep`](Self::set_keep) or
/// [`disarm`](Self::disarm) when another process is meant to take over the
/// path.
#[derive(Debug)]
pub struct SocketPathGuard {
    path: Option<PathBuf>,
    keep: bool,
}

impl SocketPathGuard {
    /// Guard `path`.
    pub fn new(path: PathBuf) -> Self {
        Self { path: Some(path), keep: false }
    }

    /// The guarded path.
    pub fn path(&self) -> &Path {
        self.path.as_deref().expect("a guard always holds its path until disarmed")
    }

    /// Whether to leave the file in place on drop.
    pub fn set_keep(&mut self, keep: bool) {
        self.keep = keep;
    }

    /// Take the path out; the file is left in place.
    #[must_use]
    pub fn disarm(mut self) -> PathBuf {
        self.keep = true;
        self.path.take().expect("a guard always holds its path until disarmed")
    }
}

impl Drop for SocketPathGuard {
    fn drop(&mut self) {
        if self.keep {
            return;
        }
        if let Some(path) = self.path.take() {
            let _ = fs::remove_file(path);
        }
    }
}

/// The directory for this user's runtime sockets: `$XDG_RUNTIME_DIR` when
/// it is set and exists, else the system temp directory.
pub fn runtime_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        let dir = PathBuf::from(dir);
        if dir.is_dir() {
            return dir;
        }
    }
    std::env::temp_dir()
}

/// `<runtime_dir>/<app>/<name>.sock`, for example
/// `/run/user/1000/myapp/control.sock`.
pub fn default_socket_path(app: &str, name: &str) -> PathBuf {
    let mut path = runtime_dir();
    path.push(app);
    path.push(format!("{name}.sock"));
    path
}

/// Create `<runtime_dir>/<app>` with the given mode (`0o700` keeps it to
/// this user) and return it.
pub fn ensure_runtime_subdir(app: &str, mode: u32) -> io::Result<PathBuf> {
    let mut dir = runtime_dir();
    dir.push(app);
    fs::create_dir_all(&dir)?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(mode))?;
    Ok(dir)
}
