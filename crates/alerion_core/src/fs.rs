use std::path::PathBuf;
use std::fs;
use std::io;

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

#[derive(Debug, Clone)]
pub struct Mounts {
    path: PathBuf,
}

impl Mounts {

}

#[cfg(feature = "wings_compat")]
mod wings_compat;
pub mod config;
