use std::borrow::Cow;
use std::io;
use std::path::{Path, PathBuf};

use futures::stream::{FuturesUnordered, StreamExt};
use thiserror::Error;
use uuid::Uuid;

pub const PYRODACTYL_USER: &str = "pyrodactyl";

#[cfg(unix)]
pub type User = unix::User;

#[cfg(windows)]
pub type User = windows::User;

#[cfg(unix)]
pub type DataDirectory = unix::DataDirectory;

#[cfg(windows)]
pub type DataDirectory = windows::DataDirectory;

#[cfg(unix)]
pub type ConfigFile = unix::ConfigFile;

#[cfg(windows)]
pub type ConfigFile = windows::ConfigFile;

#[cfg(unix)]
pub type OsLibraryError = unix::LibcError;

#[cfg(windows)]
pub type OsLibraryError = windows::WinapiError;

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

pub trait DataDirectoryImpl {
    /// The path the data directory uses.  
    ///
    /// `/var/lib/alerion` on Unix.
    fn path() -> Cow<'static, Path>;

    /// Ensures the directory exists and sets the correct permissions.
    fn initialize() -> Result<(), OsError>;

    /// A handle on bind mounts.
    fn mounts() -> Mounts;
}

pub trait ConfigFileImpl {
    fn path() -> Cow<'static, Path>;
    fn read() -> io::Result<String>;
    fn write(contents: &str) -> io::Result<()>;
}

pub struct Mounts {
    path: PathBuf,
}

impl Mounts {
    pub async fn create_clean(&self, uuid: Uuid) -> io::Result<PathBuf> {
        let uuid = format!("{}", uuid.as_hyphenated());
        let path = self.path.join(&uuid);

        tokio::fs::create_dir_all(&path).await?;
        clear_directory(&path).await?;

        Ok(path)
    }
}

async fn clear_directory(path: impl AsRef<Path>) -> io::Result<()> {
    let mut read_dir = tokio::fs::read_dir(path.as_ref()).await?;

    let mut file_futs = FuturesUnordered::new();
    let mut dir_futs = FuturesUnordered::new();

    while let Some(e) = read_dir.next_entry().await? {
        // should be free on most Unix platforms
        let ft = e.file_type().await?;

        if !ft.is_dir() {
            // the file is either a file, a symlink or unknown
            let rm_fut = tokio::fs::remove_file(e.path());
            file_futs.push(rm_fut);
        } else {
            let rm_fut = tokio::fs::remove_dir_all(e.path());
            dir_futs.push(rm_fut);
        }
    }

    let f1 = async move {
        while let Some(e) = file_futs.next().await {
            e?;
        }

        Ok::<(), io::Error>(())
    };

    let f2 = async move {
        while let Some(e) = dir_futs.next().await {
            e?;
        }

        Ok::<(), io::Error>(())
    };

    let (result1, result2) = tokio::join!(f1, f2);

    result1.and(result2)
}

#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub mod windows;
