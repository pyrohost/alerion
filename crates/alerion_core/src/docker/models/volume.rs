use std::collections::HashMap;
use std::fmt;

use bollard::volume::{CreateVolumeOptions, RemoveVolumeOptions};
use bollard::{models, Docker};
use uuid::Uuid;

use crate::docker;
use super::{Inspectable, Inspected};

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
        Self {
            uuid,
            purpose: "install",
        }
    }

    pub fn new_server(uuid: Uuid) -> Self {
        Self {
            uuid,
            purpose: "server",
        }
    }

    pub fn full_name(&self) -> String {
        format!("{}_{}", self.uuid, self.purpose)
    }
}

/// An alerion-created volume
pub struct Volume {
    /// The name of the volume.
    pub name: VolumeName,
    /// Creation time of the volume.
    pub created_at: Option<String>,
}

impl Inspectable for Volume {
    type Model = models::Volume;
    type Ref = VolumeName;

    async fn inspect(
        api: &Docker,
        args: Self::Ref,
    ) -> docker::Result<Inspected<Self>> {
        let result = api.inspect_volume(&args.full_name()).await;

        match result {
            Err(e) if docker::is_404(&e) => Ok(Inspected::None),
            Err(e) => Err(e.into()),
            Ok(response) => Ok({
                match response.labels.get(docker::ALERION_VERSION_LABEL) {
                    None => Inspected::Invalid(Box::new(response)),
                    Some(v) => {
                        let current_version = env!("CARGO_PKG_VERSION");

                        if v != current_version {
                            tracing::warn!("mismatched volume version (found {v}, currently on {current_version})");
                        }

                        Inspected::Some(Volume {
                            name: args,
                            created_at: response.created_at,
                        })
                    }
                }
            }),
        }
    }
}

impl Volume {
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
            created_at: volume.created_at,
        })
    }

    /// Uses the standalone [`force_remove_by_name`] to force
    /// remove a volume.
    pub async fn force_remove(&self, api: &Docker) -> docker::Result<()> {
        force_remove_by_name(api, &self.name.full_name()).await
    }

    pub fn to_docker_mount(&self, target: String) -> models::Mount {
        models::Mount {
            target: Some(target),
            source: Some(self.name.full_name()),
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
    let opts = RemoveVolumeOptions { force: true };

    api.remove_volume(name, Some(opts)).await?;

    Ok(())
}
