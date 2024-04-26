use std::collections::HashMap;
use std::sync::atomic::{Ordering, AtomicU32};
use std::time::Instant;
use std::sync::Arc;

use alerion_datamodel as dm;
use bollard::Docker;
use bollard::container::{Config, CreateContainerOptions};
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::servers::remote;
use crate::webserver::websocket::SendWebsocketEvent;
use super::ServerError;

//TODO: Remove allow(dead_code) when implemented
#[allow(dead_code)]
pub struct ServerInfo {
    container: dm::remote::server::ContainerConfig,
}

impl ServerInfo {
    pub fn from_remote_info(server_settings: dm::remote::server::ServerSettings) -> Self {
        Self {
            container: server_settings.container,
        }
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
        tracing::debug!("Creating new server {uuid}");

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

        Ok(server)
    }

    pub async fn add_websocket_connection(&self) -> mpsc::Receiver<SendWebsocketEvent> {
        let id = self.websocket_id_counter.fetch_add(1, Ordering::SeqCst);

        let (send, recv) = mpsc::channel(64);

        self.websocket_connections.lock().await.insert(id, send);

        recv
    }

    #[allow(dead_code)]
    async fn create_docker_container(&self) -> Result<String, ServerError> {
        tracing::info!(
            "Creating docker container for server {}",
            self.uuid.as_hyphenated()
        );

        let opts = CreateContainerOptions {
            name: self.container_name.clone(),
            platform: None,
        };

        let config: Config<&str> = Config {
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Config::default()
        };

        let response = self.docker.create_container(Some(opts), config).await?;

        tracing::debug!("Created docker container: {response:#?}");

        Ok(response.id)
    }

    pub fn server_time(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}
