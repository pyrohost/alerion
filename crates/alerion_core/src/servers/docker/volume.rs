use std::path::PathBuf;
use std::collections::HashMap;
use std::fmt;
use std::borrow::Cow;

use bollard::Docker;
use bollard::volume::{RemoveVolumeOptions, CreateVolumeOptions};
use bollard::models;
use uuid::Uuid;

use crate::servers::docker;

#[derive(Debug, Clone)]
pub struct VolumeName {
    pub uuid: Uuid,
    pub purpose: &'static str,
}

impl fmt::Display for VolumeName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

impl VolumeName {
    pub fn new_install(uuid: Uuid) -> Self {
        Self { uuid, purpose: "install" }
    }

    pub fn new_server(uuid: Uuid) -> Self {
        Self { uuid, purpose: "server" }
    }

    pub fn full_name(&self) -> String {
        format!("{}_{}", self.uuid, self.purpose)
    }
}

pub enum FoundVolume {
    Some(Volume),
    // models::Volume is ~600 bytes
    Foreign(Box<models::Volume>),
    None,
}

/// An alerion-created volume
pub struct Volume {
    pub name: VolumeName,
    /// The mountpoint of the volume, for use with `POST /containers/create` and to
    /// write necessary files to.
    pub mountpoint: PathBuf,
    /// Creation time of the volume. Only present if the volume was recovered
    /// and Docker responded with this information.
    created_at: Option<String>,
}

impl Volume {
    fn from_datamodel(name: VolumeName, resp: models::Volume) -> Self {
        Self {
            name,
            mountpoint: PathBuf::from(resp.mountpoint),
            created_at: resp.created_at,
        }
    }

    /// Checks if a specified volume already exists.
    ///
    /// Returns an error if the mountpoint contains invalid utf-8 sequences.
    pub async fn get(api: &Docker, volname: VolumeName) -> docker::Result<FoundVolume> {
        let result = api.inspect_volume(&volname.full_name()).await;

        match result {
            Err(e) if docker::is_404(&e) => Ok(FoundVolume::None),
            Err(e) => Err(e.into()),
            Ok(response) => Ok({
                match response.labels.get(docker::ALERION_VERSION_LABEL) {
                    None => {
                        FoundVolume::Foreign(Box::new(response))
                    },
                    Some(v) => {
                        let current_version = env!("CARGO_PKG_VERSION");

                        if v != current_version {
                            tracing::warn!("mismatched volume version (found {v}, currently on {current_version})");
                        }

                        FoundVolume::Some(Volume::from_datamodel(volname, response))
                    }
                }
            })
        }
    }

    /// Forces the creation of a volume with the given name. If the volume already
    /// exists, an error will be returned.  
    ///
    /// Inserts the proper labels onto the volume.  
    ///
    /// Returns an error if the mountpoint contains invalid utf-8 sequences.
    pub async fn create(api: &Docker, volname: VolumeName) -> docker::Result<Self> {
        let full_name = volname.full_name();

        let opts = CreateVolumeOptions {
            name: full_name.as_str(),
            driver: "local",
            driver_opts: HashMap::default(),
            labels: docker::alerion_version_labels(),
        };

        let volume = api.create_volume(opts).await?;

        Ok(Volume {
            name: volname,
            mountpoint: PathBuf::from(volume.mountpoint),
            created_at: None,
        })
    }

    /// Uses the standalone [`force_remove_by_name`] to force
    /// remove a volume.
    pub async fn force_remove(&self, api: &Docker) -> docker::Result<()> {
        force_remove_by_name(api, &self.name.full_name()).await
    }

    /// Returns the mountpoint as a string. Warns if the mountpoint contains
    /// invalid utf-8 sequences, although that'd be a bug.
    pub fn mountpoint_as_str(&self) -> Cow<str> {
        let os_str = self.mountpoint.as_os_str();
        let out = os_str.to_string_lossy();

        if matches!(out, Cow::Owned(_)) {
            tracing::error!("verified volume mountpoint ({os_str:#?}) still contains invalid utf8 sequences");
            tracing::error!("this is an internal bug: please contact support");
            tracing::error!("alerion may become unstable and/or break")
        }

        out
    }
    
    pub fn to_datamodel_mount(&self, target: String) -> models::Mount {
        models::Mount {
            target: Some(target),
            source: Some(self.mountpoint_as_str().into_owned()),
            typ: Some(models::MountTypeEnum::VOLUME),
            read_only: Some(false),
            consistency: None,
            bind_options: None,
            volume_options: None,
            tmpfs_options: None,
        }
    }

    pub fn created_at(&self) -> Option<&str> {
        self.created_at.as_deref()
    }
}

/// Forces removing a volume by its name.  
///
/// Useful if you don't have a `Volume` but
/// still have volume metadata.
pub async fn force_remove_by_name(api: &Docker, name: &str) -> docker::Result<()> {
    let opts = RemoveVolumeOptions {
        force: true,
    };

    api.remove_volume(name, Some(opts)).await?;

    Ok(())
}

