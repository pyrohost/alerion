use std::io;
use std::path::{Path, PathBuf};
use std::fmt;

use bollard::models;
use bollard::secret::MountBindOptions;
use uuid::Uuid;

use crate::fs::{MountType, Mounts};

#[derive(Debug, Clone)]
pub struct BindMountName {
    uuid: Uuid,
    typ: MountType,
}

impl fmt::Display for BindMountName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}_{}", self.uuid.as_hyphenated(), self.typ)
    }
}

impl BindMountName {
    pub fn new_installer(uuid: Uuid) -> Self {
        Self { uuid, typ: MountType::Installer }
    }

    pub fn new_server(uuid: Uuid) -> Self {
        Self { uuid, typ: MountType::Server }
    }
}

#[allow(dead_code)]
pub struct BindMount {
    name: BindMountName,
    path: PathBuf,
}

impl BindMount {
    /// Creates/resets a bind mount.
    pub async fn new_clean(mounts: &Mounts, name: BindMountName) -> io::Result<BindMount> {
        let path = mounts.force_recreate(name.typ).await?;

        Ok(BindMount { path, name })
    }

    pub async fn get(mounts: &Mounts, name: BindMountName) -> io::Result<BindMount> {
        let path = mounts.get(name.typ).await?;
        Ok(BindMount { path, name })
    }

    pub fn path(&self) -> &Path {
        self.path.as_path()
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
