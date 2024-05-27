use std::io;
use std::path::PathBuf;
use std::fmt;

use bollard::models;
use bollard::secret::MountBindOptions;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use uuid::Uuid;

use crate::os::{DataDirectory, DataDirectoryImpl};

#[derive(Debug, Clone)]
pub struct BindMountName {
    uuid: Uuid,
    purpose: &'static str,
}

impl fmt::Display for BindMountName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}_{}", self.uuid.as_hyphenated(), self.purpose)
    }
}

impl BindMountName {
    pub fn new_installer(uuid: Uuid) -> Self {
        Self { uuid, purpose: "installer" }
    }

    pub fn new_server(uuid: Uuid) -> Self {
        Self { uuid, purpose: "server" }
    }
}

pub struct BindMount {
    name: BindMountName,
    path: PathBuf,
}

impl BindMount {
    /// Creates/resets a bind mount.
    pub async fn new_clean(name: BindMountName) -> io::Result<BindMount> {
        let mounts = DataDirectory::mounts();
        let path = mounts.create_clean(name.uuid).await?;
        std::fs::create_dir_all(&path)?;

        Ok(BindMount { path, name })
    }

    /// Remove everything in the bind mount folder.
    pub async fn clean(&self) -> io::Result<()> {
        let mut read_dir = tokio::fs::read_dir(&self.path).await?;

        let mut futures = FuturesUnordered::new();

        loop {
            let result = read_dir.next_entry().await;
            let Some(e) = result? else {
                break;
            };
            let rm_fut = tokio::fs::remove_file(e.path());
            futures.push(rm_fut);
        }

        while let Some(r) = futures.next().await {
            r?;
        }

        Ok(())
    }

    pub fn to_docker_mount(&self, target: String) -> models::Mount {
        models::Mount {
            target: Some(target),
            source: Some(self.path.to_string_lossy().into_owned()),
            typ: Some(models::MountTypeEnum::BIND),
            read_only: Some(false),
            consistency: None,
            bind_options: Some(MountBindOptions {
                propagation: None,
                non_recursive: Some(true),
                create_mountpoint: None,
                read_only_non_recursive: None,
                read_only_force_recursive: None,
            }),
            volume_options: None,
            tmpfs_options: None,
        }
    }
}
