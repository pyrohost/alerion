use std::io;
use std::path::PathBuf;
use std::env;

use thiserror::Error;

pub const PYRODACTYL_USER: &str = "pyrodactyl";

#[cfg(unix)]
pub type User = unix::User;

#[cfg(windows)]
pub type User = windows::User;

#[cfg(unix)]
pub type ConfigPath = unix::ConfigPath;

#[cfg(windows)]
pub type ConfigPath = windows::ConfigPath;

#[cfg(unix)]
pub type OsLibraryError = unix::LibcError;

#[cfg(windows)]
pub type OsLibraryError = windows::WinapiError;

#[cfg(unix)]
pub use unix::symlink;

#[cfg(windows)]
pub use windows::symlink;

#[derive(Error, Debug)]
pub enum OsError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Os(#[from] OsLibraryError),
    #[error("could not continue due to previous failure")]
    Other,
}

pub trait UserImpl: Sized {
    fn ensure_exists() -> Result<Self, OsError>;
    fn host_uname(&self) -> Result<String, OsError>;
}

pub trait ConfigPathImpl {
    fn parent() -> Result<PathBuf, (env::VarError, &'static str)>;
    fn node() -> &'static str;
}

#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub mod windows;
