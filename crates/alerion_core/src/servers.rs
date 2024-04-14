use std::time::Instant;
use std::collections::HashMap;
use std::sync::{Arc, atomic::{Ordering, AtomicU32}};
use crate::websocket::message::PanelMessage;
use crate::websocket::relay::{ServerConnection, AuthTracker, ClientConnection};
use uuid::Uuid;
use tokio::sync::{Mutex, RwLock};
use tokio::sync::mpsc::{channel, Sender, Receiver};

#[derive(Default)]
pub struct ServerPool {
    servers: RwLock<HashMap<Uuid, Arc<Server>>>,
}

impl ServerPool {
    pub fn new() -> Self {
        Self {
            servers: RwLock::new(HashMap::new()),
        }
    }

    pub async fn create_server(&self, uuid: Uuid) {
        let server = Server::new(uuid);
        self.servers.write().await.insert(uuid, Arc::new(server));
    }

    pub async fn get(&self, uuid: Uuid) -> Option<Arc<Server>> {
        self.servers.read().await.get(&uuid).map(Arc::clone)
    }
    
    pub async fn get_or_create(&self, uuid: Uuid) -> Arc<Server> {
        // initially try to read, because most of the times we'll only need to read
        // and we can therefore reduce waiting by a lot using a read-write lock.
        let map = self.servers.read().await;

        match map.get(&uuid) {
            Some(s) => {
                Arc::clone(s)
            }

            None => {
                todo!()
            }
        }
    }
}

pub struct Server {
    start_time: Instant,
    uuid: Uuid,
    container_id: String,
    websocket_id_counter: AtomicU32,
    websockets: Mutex<HashMap<u32, ClientConnection>>,
    sender_copy: Sender<PanelMessage>,
}

impl Server {
    pub fn new(uuid: Uuid) -> Self {
        let (send, recv) = channel(128);

        tokio::spawn(task_websocket_receiver(recv));

        Self {
            start_time: Instant::now(),
            uuid,
            container_id: format!("{}_container", uuid.as_hyphenated()),
            websocket_id_counter: AtomicU32::new(0),
            websockets: Mutex::new(HashMap::new()),
            sender_copy: send,
        }
    }

    pub async fn add_websocket(&self, conn: ClientConnection) {
        let id = self.websocket_id_counter.fetch_add(1, Ordering::SeqCst);
        let mut websockets = self.websockets.lock().await;
        websockets.insert(id, conn);
    }

    pub fn new_connection_with_auth_tracker(&self) -> (ServerConnection, Arc<AuthTracker>) {
        let auth_tracker = Arc::new(AuthTracker::new(self.server_time()));
        let auth_tracker_clone = Arc::clone(&auth_tracker);
        let sender = self.sender_copy.clone();

        (ServerConnection::new(auth_tracker, sender), auth_tracker_clone)
    }

    pub fn server_time(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

async fn task_websocket_receiver(mut receiver: Receiver<PanelMessage>) {
    loop {
        match receiver.recv().await {
            Some(msg) => {
                println!("{:?}", msg)
            }

            None => {

            }
        }
    }
}
