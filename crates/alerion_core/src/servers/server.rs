use std::collections::HashMap;
use std::sync::atomic::{Ordering, AtomicU32};
use std::time::Instant;
use std::sync::Arc;

use alerion_datamodel as dm;
use bollard::Docker;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::servers::{docker, remote};
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
    pub async fn new(
        uuid: Uuid,
        server_info: ServerInfo,
        remote_api: Arc<remote::RemoteClient>,
        docker: Arc<Docker>,
    ) -> Result<Arc<Self>, ServerError> {
        let mut server = Self {
            start_time: Instant::now(),
            uuid,
            container_name: format!("{}_container", uuid.as_hyphenated()),
            websocket_id_counter: AtomicU32::new(0),
            websocket_connections: Mutex::new(HashMap::new()),
            server_info,
            remote_api,
            docker,
        };

        initiate_server(&mut server).await?;

        Ok(Arc::new(server))
    }

    pub async fn add_websocket(&self) -> mpsc::Receiver<SendWebsocketEvent> {
        let id = self.websocket_id_counter.fetch_add(1, Ordering::SeqCst);

        let (send, recv) = mpsc::channel(64);

        self.websocket_connections.lock().await.insert(id, send);

        recv
    }

    pub fn server_time(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

pub async fn initiate_server(s: &mut Server) -> Result<(), ServerError> {
    let _install = s.remote_api.get_install_instructions(s.uuid).await?;
    // lets pretend we're doing sum with this for now :3

    let _handle = docker::container::initiate_installation(&s.docker, s.uuid).await?;

    Ok(())
}
