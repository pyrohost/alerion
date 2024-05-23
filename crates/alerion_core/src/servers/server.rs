use std::rc::Rc;
use std::time::Instant;
use std::sync::Arc;

use alerion_datamodel as dm;
use bollard::Docker;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::servers::{docker, remote};
use super::ServerError;

pub struct ServerChannel {
    antenna: broadcast::Receiver<ServerMessage>,
}

#[derive(Clone, Debug)]
pub enum ServerMessage {
    ConsoleOutput {
        output: Rc<String>,
    },

    DaemonOutput {
        output: Rc<String>,
    },

    DaemonError {
        output: Rc<String>,
    },
}

/// Pools all websocket connections attached to this server.   
///
/// This is implemented in a sorta-stateless way using tokio's broadcast channels. 
/// The `WebsocketBucket` doesn't uniquely identify websocket connections. Doing so
/// would introduce overhead and complexity, requiring atomics, locks and more channels.
/// Broadcasting to many receivers is ultimately cheap since cloning messages is cheap.  
///
/// Permission handling is done by receivers.
pub struct WebsocketBucket {
    broadcaster: broadcast::Sender<ServerMessage>,
}

impl WebsocketBucket {
    pub fn new() -> Self {
        let (broadcaster, _) = broadcast::channel(64);

        Self {
            broadcaster,
        }
    }

    /// "Adds" a receiver to this bucket. Just drop the returned `ServerChannel` to unsubscribe.
    pub fn add(&self) -> ServerChannel {
        ServerChannel {
            antenna: self.broadcaster.subscribe(),
        }
    }

    /// Broadcast a message to all receivers.
    pub fn broadcast(&self, msg: ServerMessage) {
        self.broadcaster.send(msg);
    }
}

// TODO: Remove allow(dead_code) when implemented
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

// TODO: Remove allow(dead_code) when implemented
#[allow(dead_code)]
pub struct Server {
    pub start_time: Instant,
    pub websocket: WebsocketBucket,
    pub uuid: Uuid,
    docker: Arc<Docker>,
}

impl Server {
    pub fn new(uuid: Uuid, docker: Arc<Docker>) -> Self {
        Server {
            start_time: Instant::now(),
            websocket: WebsocketBucket::new(),
            uuid,
            docker,
        }
    }
}
