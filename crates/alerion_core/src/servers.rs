use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use std::time::Instant;

use alerion_datamodel::remote::server::{ContainerConfig, ServerSettings};
use bollard::container::{Config, CreateContainerOptions};
use bollard::Docker;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::AlerionConfig;

pub struct ServerPoolBuilder {
    servers: HashMap<Uuid, Arc<Server>>,
    remote_api: Arc<remote::RemoteClient>,
    docker: Arc<Docker>,
}

impl ServerPoolBuilder {
    pub fn from_config(config: &AlerionConfig) -> Result<Self, ServerError> {
        let docker = Arc::new(Docker::connect_with_defaults()?);

        Ok(Self {
            servers: HashMap::new(),
            remote_api: Arc::new(remote::RemoteClient::new(config)?),
            docker,
        })
    }

    pub async fn fetch_servers(mut self) -> Result<ServerPoolBuilder, ServerError> {
        log::info!("Fetching existing servers on this node");

        let servers = self.remote_api.get_servers().await?;

        for s in servers {
            log::info!("Adding server {}", s.uuid);

            let uuid = s.uuid;
            let info = ServerInfo::from_remote_info(s.settings);
            let server = Server::new(
                uuid,
                info,
                Arc::clone(&self.remote_api),
                Arc::clone(&self.docker),
            )
            .await?;
            self.servers.insert(uuid, server);
        }

        Ok(self)
    }

    pub fn build(self) -> ServerPool {
        ServerPool {
            servers: RwLock::new(self.servers),
            remote_api: self.remote_api,
            docker: self.docker,
        }
    }
}

pub struct ServerPool {
    servers: RwLock<HashMap<Uuid, Arc<Server>>>,
    remote_api: Arc<remote::RemoteClient>,
    docker: Arc<Docker>,
}

impl ServerPool {
    pub fn builder(config: &AlerionConfig) -> Result<ServerPoolBuilder, ServerError> {
        ServerPoolBuilder::from_config(config)
    }

    pub async fn get_or_create_server(&self, uuid: Uuid) -> Result<Arc<Server>, ServerError> {
        // initially try to read, because most of the times we'll only need to read
        // and we can therefore reduce waiting by a lot using a read-write lock.
        let map = self.servers.read().await;

        match map.get(&uuid) {
            Some(s) => Ok(Arc::clone(s)),

            None => {
                drop(map);
                self.create_server(uuid).await
            }
        }
    }

    pub async fn create_server(&self, uuid: Uuid) -> Result<Arc<Server>, ServerError> {
        log::info!("Creating server {uuid}");

        let remote_api = Arc::clone(&self.remote_api);
        let docker = Arc::clone(&self.docker);

        let config = remote_api.get_server_configuration(uuid).await?;
        let server_info = ServerInfo::from_remote_info(config.settings);

        let server = Server::new(uuid, server_info, remote_api, docker).await?;
        self.servers.write().await.insert(uuid, Arc::clone(&server));

        Ok(server)
    }

    pub async fn get_server(&self, uuid: Uuid) -> Option<Arc<Server>> {
        self.servers.read().await.get(&uuid).cloned()
    }
}

pub struct ServerInfo {
    container: ContainerConfig,
}

impl ServerInfo {
    pub fn from_remote_info(server_settings: ServerSettings) -> Self {
        Self {
            container: server_settings.container,
        }
    }
}

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("docker error: {0}")]
    Docker(#[from] bollard::errors::Error),
    #[error("panel remote API error: {0}")]
    RemoteApi(#[from] remote::ResponseError),
}

#[derive(Serialize, Deserialize, Default, Copy, Clone, PartialEq, Eq, Hash)]
struct IntoStringZst;

impl From<IntoStringZst> for String {
    fn from(_value: IntoStringZst) -> Self {
        String::new()
    }
}

pub struct Server {
    start_time: Instant,
    uuid: Uuid,
    container_name: String,
    websocket_id_counter: AtomicU32,
    server_info: ServerInfo,
    remote_api: Arc<remote::RemoteClient>,
    docker: Arc<Docker>,
}

impl Server {
    pub async fn new(
        uuid: Uuid,
        server_info: ServerInfo,
        remote_api: Arc<remote::RemoteClient>,
        docker: Arc<Docker>,
    ) -> Result<Arc<Self>, ServerError> {
        let server = Arc::new(Self {
            start_time: Instant::now(),
            uuid,
            container_name: format!("{}_container", uuid.as_hyphenated()),
            websocket_id_counter: AtomicU32::new(0),
            server_info,
            remote_api,
            docker,
        });

        server.create_docker_container().await?;

        Ok(server)
    }

    async fn create_docker_container(&self) -> Result<(), ServerError> {
        log::info!(
            "Creating docker container for server {}",
            self.uuid.as_hyphenated()
        );

        let opts = CreateContainerOptions {
            name: self.container_name.clone(),
            platform: None,
        };

        let config: Config<IntoStringZst> = Config {
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Config::default()
        };

        let response = self.docker.create_container(Some(opts), config).await?;

        log::debug!("{response:#?}");

        Ok(())
    }

    pub fn server_time(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

pub mod remote;
