use std::path::{Path, PathBuf};
use std::fs;
use std::io;
use std::fmt;

use uuid::Uuid;
use futures::stream::{StreamExt, FuturesUnordered};

const MOUNTS: &str = "mounts";
const BACKUPS: &str = "backups";

#[derive(Debug, Clone)]
pub struct LocalData {
    path: PathBuf,
}

impl LocalData {
    pub fn new(path: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(path.join(MOUNTS))?; 
        fs::create_dir_all(path.join(BACKUPS))?; 

        Ok(LocalData {
            path
        })
    }

    pub fn mounts(&self) -> Mounts {
        Mounts { path: self.path.join(MOUNTS) }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MountType {
    Installer,
    Server,
}

impl fmt::Display for MountType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", match self {
            MountType::Installer => "installer",
            MountType::Server => "server",
        })
    }
}

#[derive(Debug, Clone)]
pub struct Mounts {
    path: PathBuf,
}

impl Mounts {
    pub async fn retrive(&self, uuid: Uuid, typ: MountType) -> io::Result<PathBuf> {
        let name = format!("{}_{}", uuid.as_hyphenated(), typ);
        let path = self.path.join(&name).to_owned();

        if !tokio::fs::try_exists(&path).await? {
            tokio::fs::create_dir_all(&path).await?;    
        }

        Ok(path)
    }

    pub async fn force_recreate(&self, uuid: Uuid, typ: MountType) -> io::Result<PathBuf> {
        let name = format!("{}_{}", uuid.as_hyphenated(), typ);
        let path = self.path.join(&name).to_owned();

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

pub use config::Config;

#[cfg(feature = "wings_compat")]
mod wings_compat;
pub mod config;
