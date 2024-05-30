use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::io;
use std::fmt;
use std::sync::Arc;
use std::ops::{DerefMut, Deref};

use uuid::Uuid;
use futures::stream::{StreamExt, FuturesUnordered};
use chrono::offset::Local;
use serde::{Serialize, Deserialize};
use tokio::fs;
use tokio::io::{AsyncWriteExt, BufWriter};

use crate::os;

const MOUNTS: &str = "mounts";
const BACKUPS: &str = "backups";
const LOGS: &str = "logs";
const DB_FILE: &str = "db.json";

#[derive(Debug, Clone)]
pub struct LocalDataHandle {
    path: PathBuf,
    database: db::Handle,
}

impl LocalDataHandle {
    pub async fn new(path: PathBuf) -> io::Result<Self> {
        tokio::try_join!(
            fs::create_dir_all(path.join(MOUNTS)),
            fs::create_dir_all(path.join(BACKUPS)),
            fs::create_dir_all(path.join(LOGS)),
        )?;

        Ok(LocalDataHandle {
            database: db::Handle::new(path.join(DB_FILE)),
            path,
        })
    }

    pub fn mounts(&self) -> Mounts {
        Mounts { path: self.path.join(MOUNTS) }
    }

    pub fn db(&self) -> db::Handle {
        self.database.clone()
    }

    pub async fn logger(&self, uuid: Uuid) -> io::Result<FsLogger> {
        let root = self.path.join(LOGS).join(format!("{}", uuid.as_hyphenated()));
        let install = root.join("install");
        let server = root.join("server");

        tokio::try_join!(
            fs::create_dir_all(&install),
            fs::create_dir_all(&server),
        )?;

        Ok(FsLogger {
            install,
            server,
        })
    }
}


pub struct FsLogger {
    install: PathBuf,
    server: PathBuf,
}

impl FsLogger {
    pub async fn open_install(&self) -> LogFileInterface {
        let (fd, path) = self.open(&self.install).await;
        LogFileInterface::new(fd, path)
    }

    pub async fn open_server(&self) -> LogFileInterface {
        let (fd, path) = self.open(&self.server).await;
        LogFileInterface::new(fd, path)
    }

    async fn open(&self, path: &Path) -> (Option<fs::File>, PathBuf) {
        let time = Local::now();
        let filename = format!("{}.log", time.format("%Y-%m-%dt%H-%M-%S"));
        let logpath = path.join(filename);

        let result = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&logpath)
            .await;

        let logfile = match result {
            Ok(f) => f,
            Err(e) => {
                tracing::error!("failed to open logfile at '{}': {e}", path.display());
                return (None, logpath);
            }
        };

        let latestpath = path.join("latest.log");
        // don't care if it fails
        let _ = fs::remove_file(&latestpath).await;

        // it's blocking but it's fineeeee
        if let Err(e) = os::symlink(&logpath, &latestpath) {
            tracing::error!("cannot create latest.log symlink: {e}");
        }
        
        (Some(logfile), logpath)
    }
}

const LOGFILE_BUF_SIZE: usize = 4096;
const LOGFILE_ROT_COUNT: usize = 8;

/// Buffered, nullable handle to a log file.  
///
/// Buffers writes, but always flushes after a certain number of writes to
/// avoid withholding too many lines in case of slow traffic.  
///
/// May hold no file descriptor because it couldn't be opened, in which
/// case all I/O methods are no-ops.
pub struct LogFileInterface {
    fd: Option<BufWriter<fs::File>>,
    path: PathBuf,
    rot: usize,
    len: usize,
}

impl LogFileInterface {
    fn new(fd: Option<fs::File>, path: PathBuf) -> Self {
        Self {
            fd: fd.map(|fd| BufWriter::with_capacity(LOGFILE_BUF_SIZE, fd)),
            path,
            rot: 0,
            len: 0,
        }
    }

    pub fn exists(&self) -> bool {
        self.fd.is_some()
    }
    
    pub fn path(&self) -> &Path {
        self.path.as_ref()
    }

    pub async fn write(&mut self, contents: &str) {
        if let Some(ref mut fd) = self.fd {
            if let Err(e) = fd.write_all(contents.as_bytes()).await {
                tracing::error!("failed to write to logfile at '{}': {e}", self.path.display());
                return;
            }

            self.len += contents.len();
            self.rot += 1;

            if self.rot == LOGFILE_ROT_COUNT || self.len >= LOGFILE_BUF_SIZE {
                self.rot = 0;
                if let Err(e) = fd.flush().await {
                    tracing::error!("failed to flush logfile '{}' buffer: {e}", self.path.display());
                    return;
                }
                self.len = 0;
            }
        }
    }

    pub async fn flush(&mut self) {
        if let Some(ref mut fd) = self.fd {
            if let Err(e) = fd.flush().await {
                tracing::error!("failed to flush logfile '{}' buffer: {e}", self.path.display());
            }
            self.rot = 0;
            self.len = 0;
        }
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

        if !fs::try_exists(&path).await? {
            fs::create_dir_all(&path).await?;    
        }

        Ok(path)
    }

    pub async fn force_recreate(&self, uuid: Uuid, typ: MountType) -> io::Result<PathBuf> {
        let name = format!("{}_{}", uuid.as_hyphenated(), typ);
        let path = self.path.join(&name).to_owned();

        fs::create_dir_all(&path).await?; 
        clear_directory(&path).await?;

        Ok(path)
    }
}

async fn clear_directory(path: impl AsRef<Path>) -> io::Result<()> {
    let mut read_dir = fs::read_dir(path.as_ref()).await?;

    let mut file_futs = FuturesUnordered::new();
    let mut dir_futs = FuturesUnordered::new();

    while let Some(e) = read_dir.next_entry().await? {
        // should be free on most Unix platforms
        let ft = e.file_type().await?;

        if !ft.is_dir() {
            // the file is either a file, a symlink or unknown
            let rm_fut = fs::remove_file(e.path());
            file_futs.push(rm_fut);
        } else {
            let rm_fut = fs::remove_dir_all(e.path());
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
pub mod db;
