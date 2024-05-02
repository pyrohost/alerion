use std::fmt;

use bollard::Docker;
use bollard::service::ContainerInspectResponse;
use bollard::container::{AttachContainerOptions, AttachContainerResults, Config, CreateContainerOptions, InspectContainerOptions, RemoveContainerOptions, StartContainerOptions};
use bollard::models;
use tokio::task::JoinHandle;
use uuid::Uuid;
use futures::StreamExt;
use tokio::fs;

use crate::servers::docker::{self, DockerError, volume::{self, VolumeName, Volume, FoundVolume}};

#[derive(Debug, Clone)]
pub struct ContainerName {
    uuid: Uuid,
    purpose: &'static str,
}

impl fmt::Display for ContainerName {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.full_name())
    }
}

impl ContainerName {
    pub fn new_install(uuid: Uuid) -> Self {
        Self { uuid, purpose: "installer" }
    }

    pub fn new_server(uuid: Uuid) -> Self {
        Self { uuid, purpose: "server" }
    }

    pub fn full_name(&self) -> String {
        format!("{}_{}", self.uuid, self.purpose)
    }
    
    pub fn short_uid(&self) -> String {
        format!("{:08x}", self.uuid.as_fields().0)
    }
}

pub enum FoundContainer {
    Some(Container),
    // ContainerInspectResponse is ~3KB
    Foreign(Box<ContainerInspectResponse>),
    None,
}

pub struct Container {
    id: String,
    created_at: Option<String>,
}

impl Container {
    pub async fn get<'a>(api: &Docker, name: ContainerName) -> docker::Result<FoundContainer> {
        let opts = InspectContainerOptions {
            size: false,
        };

        let result = api.inspect_container(&name.full_name(), Some(opts)).await;

        let response = match result {
            Err(e) if docker::is_404(&e) => {
                return Ok(FoundContainer::None);
            }

            Err(e) => {
                return Err(e.into());
            }

            Ok(r) => r,
        };


        let Some(id) = response.id else {
            tracing::error!("missing container id from Docker Engine response");
            return Err(DockerError::BadResponse);
        };

        // this is to avoid a partial move of `response`
        let version = {
            match response.config {
                Some(ref c) => match c.labels {
                    Some(ref l) => l.get(docker::ALERION_VERSION_LABEL).cloned(),
                    None => None,
                }
                None => None,
            }
        };

        let current_version = env!("CARGO_PKG_VERSION");

        Ok(match version {
            Some(v) => {
                if v != current_version {
                    tracing::warn!("mismatched container version (found {v}, currently on {current_version})");
                }

                FoundContainer::Some(Container {
                    id,
                    created_at: response.created,
                })
            }

            None => {
                FoundContainer::Foreign(Box::new(ContainerInspectResponse {
                    id: Some(id),
                    ..response
                }))
            },
        })
    }

    pub async fn create(
        api: &Docker,
        name: ContainerName,
        host_config: models::HostConfig,
    ) -> docker::Result<Container> {
        let opts = CreateContainerOptions {
            name: name.full_name(),
            platform: None,
        };

        let hostname = name.short_uid();

        let cfg = Config {
            hostname: Some(hostname.as_str()),
            user: Some("1000:1000"),
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            open_stdin: Some(true),
            image: Some("ghcr.io/pterodactyl/installers:alpine"),
            cmd: Some(vec!["ash", "/mnt/install/install.sh"]),
            env: Some(vec!["SUBJECT=world"]),
            host_config: Some(host_config),
            labels: Some(docker::alerion_version_labels()),
            ..Config::default()
        };

        let response = api.create_container(Some(opts), cfg).await?;

        let warnings = &response.warnings;
        if !warnings.is_empty() {
            tracing::warn!("Docker emitted the following warnings after creating container {name:?}:");

            for w in warnings {
                tracing::warn!("{w}");
            }
        }

        Ok(Container {
            id: response.id,
            created_at: None,
        })
    }

    pub async fn start(&self, api: &Docker) -> docker::Result<()> {
        api.start_container(&self.id, None::<StartContainerOptions::<String>>).await?;
        Ok(())
    }

    pub async fn attach(&self, api: &Docker) -> docker::Result<()> {
        let opts = AttachContainerOptions {
            stdin: Some(true),
            stdout: Some(true),
            stderr: Some(true),
            stream: Some(true),
            logs: Some(true),
            detach_keys: None::<String>,
        };

        let streams = api.attach_container(&self.id, Some(opts)).await?;
        let AttachContainerResults { input: _, mut output } = streams;

        while let Some(result) = output.next().await {
            println!("{result:#?}");
        }

        tracing::info!("closed");

        Ok(())
    }

    /// Uses [`force_remove_by_name_or_id`].
    pub async fn force_remove(&self, api: &Docker) -> docker::Result<()> {
        force_remove_by_name_or_id(api, &self.id).await
    }

    pub fn created_at(&self) -> Option<&str> {
        self.created_at.as_deref()
    }
}

/// Deletes a container, stopping it if it's running, deleting any
/// anonymous containers associated with it.  
///
/// Useful if you don't have a [`Container`], but still have metadata
/// about it.
pub async fn force_remove_by_name_or_id(api: &Docker, name_or_id: &str) -> docker::Result<()> {
    let opts = RemoveContainerOptions {
        force: true,
        v: true,
        link: true,
    };

    api.remove_container(name_or_id, Some(opts)).await?;

    Ok(())
}

/// Initiates the installation of a server.  
///
/// Please, ensure check was made to ensure the installation was not properly
/// completeled beforehand, as this will undo and delete any previous installation
/// attempt's progress.  
///
/// If this is a reinstall, delete the involved containers and volumes beforehand
/// to avoid warnings being emitted.  
///
/// 1. Create necessary volumes.
/// 2. Create the installation container.
/// 3. Start the container
/// 4. Watch the container
pub async fn initiate_installation(api: &Docker, uuid: Uuid) -> docker::Result<JoinHandle<docker::Result<()>>> {
    let install_volume = {
        let name = VolumeName::new_install(uuid);

        match Volume::get(api, name.clone()).await? {
            FoundVolume::Some(vol) => {
                tracing::warn!("the installation volume was already created by alerion, but not deleted");
                tracing::warn!("creation time: {}", vol.created_at().unwrap_or("unknown"));
                tracing::warn!("this signals alerion might have crashed during the installation process");
                tracing::warn!("the volume will be force-deleted and the installation process will restart");

                volume::force_remove_by_name(api, &name.full_name()).await?;
            }

            FoundVolume::Foreign(resp) => {
                tracing::warn!("the installation volume was already created, but not by alerion");
                tracing::warn!("this might be an artifact from wings");
                tracing::warn!("the volume will be force-deleted and the installation process will start");

                tracing::debug!("Docker response body: {resp:#?}");

                volume::force_remove_by_name(api, &name.full_name()).await?;
            }

            FoundVolume::None => {
                tracing::debug!("installation volume not found: OK");
            }
        }

        tracing::info!("creating installation volume");

        match Volume::create(api, name).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("failed to create installation volume: {e:?}");
                return Err(e);
            }
        }
    };

    let server_volume = {
        let name = VolumeName::new_server(uuid);

        match Volume::get(api, name.clone()).await? {
            FoundVolume::Some(vol) => {
                tracing::warn!("the server volume already exists and was created by alerion");
                tracing::warn!("creation time: {}", vol.created_at().unwrap_or("unknown"));
                tracing::warn!("this could either mean alerion crashed, or the installation");
                tracing::warn!("process is not supposed to run right now and this is a bug");
                tracing::warn!("TODO: we might want to back up this server, to avoid data loss");
                tracing::warn!("caused by an alerion bug");
                tracing::warn!("the volume will be deleted and the installation process will restart");

                vol.force_remove(api).await?;
            }

            FoundVolume::Foreign(resp) => {
                tracing::warn!("the server volume already exists, but wasn't created by alerion");
                tracing::warn!("this could be an artifact from wings");
                tracing::warn!("the volume will be deleted and the installation process will start");

                tracing::debug!("Docker response body: {resp:#?}");

                volume::force_remove_by_name(api, &name.full_name()).await?;
            }

            FoundVolume::None => {
                tracing::debug!("server volume not found: OK");
            }
        }

        tracing::info!("creating server volume");

        match Volume::create(api, name).await {
            Ok(v) => v,
            Err(e) => {
                tracing::error!("failed to create server volume: {e:?}");
                return Err(e);
            }
        }
    };


    let install_container = {
        let name = ContainerName::new_install(uuid);

        match Container::get(api, name.clone()).await? {
            FoundContainer::Some(cont) => {
                tracing::warn!("the installation container already exists and was created by alerion");
                tracing::warn!("creation time: {}", cont.created_at().unwrap_or("unknown"));
                tracing::warn!("this could either mean alerion crashed, or the installation");
                tracing::warn!("process is not supposed to run right now and this is a bug");
                tracing::warn!("the container will be deleted and the installation process will restart");

                cont.force_remove(api).await?;
            }

            FoundContainer::Foreign(resp) => {
                tracing::warn!("the installation container already exists, but wasn't created by alerion");
                tracing::warn!("this could be an artifact from wings");
                tracing::warn!("the container will be deleted and the installation process will start");

                tracing::debug!("Docker response body: {resp:#?}");

                force_remove_by_name_or_id(api, &name.full_name()).await?;
            }

            FoundContainer::None => {
                tracing::debug!("installation container not found: OK");
            }
        }

        
        let volumes = vec![
            install_volume.to_datamodel_mount("/mnt/install".to_owned()),
            server_volume.to_datamodel_mount("/mnt/server".to_owned()),
        ];

        let host_cfg = models::HostConfig {
            mounts: Some(volumes),
            ..models::HostConfig::default()
        };

        tracing::info!("creating installation container");

        match Container::create(api, name, host_cfg).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("failed to create installation container: {e:?}");
                return Err(e);
            }
        }
    };


    // put the installation script in the install volume
    let path = install_volume.mountpoint.join("install.sh");
    if let Err(e) = fs::write(path, b"echo \"hello, $SUBJECT!\"\n").await {
        tracing::error!("failed to write installation script: {e:?}");
        return Err(e.into());
    };


    if let Err(e) = install_container.start(api).await {
        tracing::error!("container failed to start: {e:?}");
        return Err(e);
    }

    let monitor_api = api.clone();
    let handle = tokio::spawn(async move {
        if let Err(e) = install_container.attach(&monitor_api).await {
            tracing::error!("failed to attach to container: {e:?}");
        }

        Ok(())
    });

    Ok(handle)
}

