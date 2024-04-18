use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use actix_web::HttpResponse;
use alerion_datamodel::websocket::{PerformanceStatisics, NetworkStatistics, ServerStatus};
use alerion_datamodel::remote::server::{ContainerConfig, ServerSettings};
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::RwLock;
use uuid::Uuid;
use bollard::Docker;
use bollard::container::{Config, CreateContainerOptions};
use thiserror::Error;
use serde::{Serialize, Deserialize};

use crate::config::AlerionConfig;
use crate::websocket::conn::{ConnectionAddr , PanelMessage, ServerMessage};
use crate::websocket::relay::{AuthTracker, ClientConnection, ServerConnection};

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
            ).await?;
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
    websockets: RwLock<HashMap<u32, ClientConnection>>,
    sender_copy: Sender<(u32, PanelMessage)>,
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
        let (send, recv) = channel(128);

        let server = Arc::new(Self {
            start_time: Instant::now(),
            uuid,
            container_name: format!("{}_container", uuid.as_hyphenated()),
            websocket_id_counter: AtomicU32::new(0),
            websockets: RwLock::new(HashMap::new()),
            sender_copy: send,
            server_info,
            remote_api,
            docker,
        });

        tokio::spawn(task_websocket_receiver(recv));
        tokio::spawn(monitor_performance_metrics(Arc::clone(&server)));

        server.create_docker_container().await?;

        Ok(server)
    }

    async fn create_docker_container(&self) -> Result<(), ServerError> {
        log::info!("Creating docker container for server {}", self.uuid.as_hyphenated());

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

    pub async fn setup_new_websocket<F>(&self, start_websocket: F) -> actix_web::Result<HttpResponse>
    where
        F: FnOnce(ServerConnection) -> actix_web::Result<(ConnectionAddr, HttpResponse)>,
    {
        let id = self.websocket_id_counter.fetch_add(1, Ordering::SeqCst);

        // setup the request channel for the websocket
        let auth_tracker = Arc::new(AuthTracker::new(self.server_time()));
        let sender = self.sender_copy.clone();
        let server_conn = ServerConnection::new(Arc::clone(&auth_tracker), sender, id);

        // setup a websocket connection through the user-provided closure
        let (addr, response) = start_websocket(server_conn)?;

        // add the obtained reply channel to the list of websocket connections
        let client_conn = ClientConnection::new(auth_tracker, addr);
        let mut websockets = self.websockets.write().await;
        websockets.insert(id, client_conn);

        // give back the HTTP 101 response
        Ok(response)
    }

    pub async fn send_to_available_websockets(&self, msg: ServerMessage) {
        let lock = self.websockets.read().await;

        for sender in lock.values() {
            sender.send_if_authenticated(|| msg.clone());
        }
    }

    pub fn server_time(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

async fn monitor_performance_metrics(server: Arc<Server>) {
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        let stats = PerformanceStatisics {
            memory_bytes: server.server_time() as usize,
            memory_limit_bytes: 1024usize.pow(3) * 8,
            cpu_absolute: 50.11,
            network: NetworkStatistics {
                rx_bytes: 1024,
                tx_bytes: 800,
            },
            uptime: 5000 + server.server_time(),
            state: ServerStatus::Running,
            disk_bytes: 100,
        };

        server.send_to_available_websockets(ServerMessage::Stats(stats)).await;
    }
}

async fn task_websocket_receiver(mut receiver: Receiver<(u32, PanelMessage)>) {
    loop {
        if let Some(msg) = receiver.recv().await {
            log::debug!("Server received websocket message: {msg:?}");
        }
    }
}

pub mod remote;
