use std::fmt;
use std::hash::Hash;
use std::pin::Pin;

use bollard::container::{
    AttachContainerOptions, AttachContainerResults, Config, CreateContainerOptions, InspectContainerOptions, LogOutput, RemoveContainerOptions, StartContainerOptions
};
use bollard::service::ContainerInspectResponse;
use bollard::errors::Error as BollardError;
use bollard::Docker;
use futures::Stream;
use uuid::Uuid;
use serde::Serialize;
use tokio::io::AsyncWrite;

use crate::docker;

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

pub struct Container {
    id: String,
    created_at: Option<String>,
}

async fn inspect_container(api: &Docker, name_or_id: &str) -> Result<ContainerInspectResponse, BollardError> {
    let opts = InspectContainerOptions { size: false };
    api.inspect_container(name_or_id, Some(opts)).await
}

impl Container {
    pub async fn recreate<Z>(
        api: &Docker,
        name: ContainerName,
        config: Config<Z>,
    ) -> docker::Result<Self>
    where
        Z: Into<String> + Eq + Hash + Serialize,
    {
        let result = inspect_container(api, &name.full_name()).await;

        match result {
            Err(e) if docker::is_404(&e) => {
                tracing::debug!("creating container");
                Container::create(api, name, config).await
            }

            Ok(response) => {    
                let version = response.config
                    .as_ref()
                    .and_then(|c| c.labels.as_ref())
                    .and_then(|l| l.get(docker::ALERION_VERSION_LABEL).cloned());

                let current = env!("CARGO_PKG_VERSION");
                if version.as_deref() != Some(current) {
                    tracing::warn!(
                        "mismatched container version (found {}, currently on {})",
                        version.as_deref().unwrap_or("none"),
                        current,
                    );
                }

                tracing::debug!("container already exists, deleting it and creating it again");
                force_remove_by_name_or_id(api, &name.full_name()).await?;

                Container::create(api, name, config).await
            },

            Err(e) => {
                tracing::error!("failed to inspect container: {e}");
                Err(e.into())
            }
        }
    }

    pub async fn create<Z>(
        api: &Docker,
        name: ContainerName,
        config: Config<Z>,
    ) -> docker::Result<Container>
    where
        Z: Into<String> + Eq + Hash + Serialize,
    {
        let opts = CreateContainerOptions {
            name: name.full_name(),
            platform: None,
        };

        let response = api.create_container(Some(opts), config).await?;

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

    pub async fn attach(
        &self,
        api: &Docker,
        attach_stdin: bool,
    ) -> docker::Result<(
            Pin<Box<dyn AsyncWrite + Send>>,
            Pin<Box<dyn Stream<Item = Result<LogOutput, BollardError>> + Send>>,
        )>
    {
        let opts = AttachContainerOptions {
            stdin: Some(attach_stdin),
            stdout: Some(true),
            stderr: Some(true),
            stream: Some(true),
            logs: Some(true),
            detach_keys: None::<String>,
        };

        let streams = api.attach_container(&self.id, Some(opts)).await?;
        let AttachContainerResults {
            input,
            output,
        } = streams;

        Ok((input, output))
    }

    pub async fn inspect_existing(&self, api: &Docker) -> docker::Result<ContainerInspectResponse> {
        let opts = InspectContainerOptions { size: false };
        let resp = api.inspect_container(&self.id, Some(opts)).await?;
        Ok(resp)
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
        link: false,
    };

    println!("deleting");
    api.remove_container(name_or_id, Some(opts)).await?;
    println!("deleted");

    Ok(())
}

