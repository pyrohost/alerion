use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Instant;

use actix_web::HttpResponse;
use tokio::sync::mpsc::{channel, Receiver, Sender};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

use crate::config::AlerionConfig;
use crate::websocket::conn::{ConnectionAddr, PanelMessage};
use crate::websocket::relay::{AuthTracker, ClientConnection, ServerConnection};

pub struct ServerPoolBuilder {
    servers: HashMap<Uuid, Arc<Server>>,
    remote_api: Arc<remote::RemoteClient>,
}

impl ServerPoolBuilder {
    pub fn from_config(config: &AlerionConfig) -> Self {

        Self {
            servers: HashMap::new(),
            remote_api: Arc::new(remote::RemoteClient::new(config)),
        }
    }

    pub async fn fetch_servers(mut self) -> Result<ServerPoolBuilder, remote::ResponseError> {
        log::info!("Fetching existing servers on this node");

        let servers = self.remote_api.get_servers().await?;

        for s in servers {
            log::info!("Adding server {}", s.uuid);

            let uuid = s.uuid;
            let info = ServerInfo::from_remote_info(s.settings);
            let server = Server::new(uuid, info, Arc::clone(&self.remote_api));
            self.servers.insert(uuid, Arc::new(server));
        }

        Ok(self)
    }

    pub fn build(self) -> ServerPool {
        ServerPool {
            servers: RwLock::new(self.servers),
            remote_api: self.remote_api,
        }
    }
}

pub struct ServerPool {
    servers: RwLock<HashMap<Uuid, Arc<Server>>>,
    remote_api: Arc<remote::RemoteClient>,
}

impl ServerPool {
    pub fn builder(config: &AlerionConfig) -> ServerPoolBuilder {
        ServerPoolBuilder::from_config(config)
    }

    pub async fn get_or_create_server(&self, uuid: Uuid) -> Result<Arc<Server>, remote::ResponseError> {
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

    pub async fn create_server(&self, uuid: Uuid) -> Result<Arc<Server>, remote::ResponseError> {
        log::info!("Creating server {uuid}");

        let remote_api = Arc::clone(&self.remote_api);

        let config = remote_api.get_server_configuration(uuid).await?;

        let server_info = ServerInfo::from_remote_info(config.settings);

        let server = Arc::new(Server::new(uuid, server_info, remote_api));
        self.servers.write().await.insert(uuid, Arc::clone(&server));

        Ok(server)
    }

    pub async fn get_server(&self, uuid: Uuid) -> Option<Arc<Server>> {
        self.servers.read().await.get(&uuid).cloned()
    }
}

pub struct ServerInfo {
    container: remote::ContainerConfig,
}

impl ServerInfo {
    pub fn from_remote_info(server_settings: remote::ServerSettings) -> Self {
        Self {
            container: server_settings.container,
        }
    }
}

pub struct Server {
    start_time: Instant,
    uuid: Uuid,
    container_id: String,
    websocket_id_counter: AtomicU32,
    websockets: Mutex<HashMap<u32, ClientConnection>>,
    sender_copy: Sender<(u32, PanelMessage)>,
    server_info: ServerInfo,
    remote_api: Arc<remote::RemoteClient>,
}

impl Server {
    pub fn new(uuid: Uuid, server_info: ServerInfo, remote_api: Arc<remote::RemoteClient>) -> Self {
        let (send, recv) = channel(128);

        tokio::spawn(task_websocket_receiver(recv));

        Self {
            start_time: Instant::now(),
            uuid,
            container_id: format!("{}_container", uuid.as_hyphenated()),
            websocket_id_counter: AtomicU32::new(0),
            websockets: Mutex::new(HashMap::new()),
            sender_copy: send,
            server_info,
            remote_api,
        }
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
        let mut websockets = self.websockets.lock().await;
        websockets.insert(id, client_conn);

        // give back the HTTP 101 response
        Ok(response)
    }

    pub fn server_time(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
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
