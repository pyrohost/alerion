use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use alerion_datamodel::remote::server::{ContainerConfig, ServerSettings};
use bollard::container::{Config, CreateContainerOptions};
use bollard::Docker;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::config::AlerionConfig;
use crate::webserver::websocket::SendWebsocketEvent;

pub struct ServerPool {
    servers: RwLock<HashMap<Uuid, Arc<Server>>>,
    remote_api: Arc<remote::RemoteClient>,
    docker: Arc<Docker>,
}

impl ServerPool {
    #[tracing::instrument(skip(config))]
    pub async fn new(config: &AlerionConfig) -> Result<Self, ServerError> {
        tracing::info!("Initializing managed servers...");

        let remote_api = remote::RemoteClient::new(config)?;

        tracing::info!("Initiating connection to Docker Engine");
        let docker = Docker::connect_with_defaults()?;

        Ok(Self {
            servers: RwLock::new(HashMap::new()),
            remote_api: Arc::new(remote_api),
            docker: Arc::new(docker),
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn fetch_existing_servers(&self) -> Result<(), ServerError> {
        tracing::info!("Fetching existing servers on this node");

        let servers = self.remote_api.get_servers().await?;

        for s in servers {
            tracing::info!("Adding server {}...", s.uuid);

            let uuid = s.uuid;
            let info = ServerInfo::from_remote_info(s.settings);
            let server = Server::new(
                uuid,
                info,
                Arc::clone(&self.remote_api),
                Arc::clone(&self.docker),
            )
            .await?;

            self.servers.write().await.insert(uuid, server);
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn register_server(&self, uuid: Uuid) -> Result<Arc<Server>, ServerError> {
        tracing::info!("Adding server {uuid}...");

        let remote_api = Arc::clone(&self.remote_api);
        let docker = Arc::clone(&self.docker);

        tracing::debug!("Fetching server configuration from remote");
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

//TODO: Remove allow(dead_code) when implemented
#[allow(dead_code)]
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

//TODO: Remove allow(dead_code) when implemented
#[allow(dead_code)]
pub struct Server {
    start_time: Instant,
    uuid: Uuid,
    container_name: String,
    websocket_id_counter: AtomicU32,
    websocket_connections: Mutex<HashMap<u32, mpsc::Sender<SendWebsocketEvent>>>,
    server_info: ServerInfo,
    remote_api: Arc<remote::RemoteClient>,
    docker: Arc<Docker>,
}

impl Server {
    #[tracing::instrument(skip(server_info, remote_api, docker))]
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
            websocket_connections: Mutex::new(HashMap::new()),
            server_info,
            remote_api,
            docker,
        });

        server.create_docker_container().await?;

        tracing::info!("Server {uuid} created");

        Ok(server)
    }

    pub async fn add_websocket_connection(&self) -> mpsc::Receiver<SendWebsocketEvent> {
        let id = self.websocket_id_counter.fetch_add(1, Ordering::SeqCst);

        let (send, recv) = mpsc::channel(64);

        self.websocket_connections
            .lock()
            .await
            .insert(id, send);

        recv
    }


    async fn create_docker_container(&self) -> Result<(), ServerError> {
        tracing::info!(
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

        tracing::debug!("{response:#?}");

        Ok(())
    }

    pub fn server_time(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

pub mod remote;
