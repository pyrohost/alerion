use std::fmt;

use bollard::container::{
    AttachContainerOptions, AttachContainerResults, Config, CreateContainerOptions, InspectContainerOptions, RemoveContainerOptions, StartContainerOptions
};
use bollard::service::ContainerInspectResponse;
use bollard::{models, Docker};
use futures::StreamExt;
use uuid::Uuid;

use crate::os::PYRODACTYL_USER;
use crate::docker::{self, DockerError, is_404};

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
        Self {
            uuid,
            purpose: "installer",
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
        let opts = InspectContainerOptions { size: false };

        let result = api.inspect_container(&name.full_name(), Some(opts)).await;

        let response = match result {
            Err(e) if is_404(&e) => {
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
                },
                None => None,
            }
        };

        let current_version = env!("CARGO_PKG_VERSION");

        Ok(match version {
            Some(v) => {
                if v != current_version {
                    tracing::warn!(
                        "mismatched container version (found {v}, currently on {current_version})"
                    );
                }

                FoundContainer::Some(Container {
                    id,
                    created_at: response.created,
                })
            }

            None => FoundContainer::Foreign(Box::new(ContainerInspectResponse {
                id: Some(id),
                ..response
            })),
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
            user: Some(PYRODACTYL_USER),
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
            tracing::warn!(
                "Docker emitted the following warnings after creating container {name:?}:"
            );

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
        api.start_container(&self.id, None::<StartContainerOptions<String>>)
            .await?;
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
        let AttachContainerResults {
            input: _,
            mut output,
        } = streams;

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

