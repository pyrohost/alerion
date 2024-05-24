use std::rc::Rc;
use std::time::Instant;
use std::sync::Arc;

use bollard::Docker;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::servers::{docker, remote};
pub use active::Egg;

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

pub enum State {
    /// Empty server, with no egg set.
    Bare,
    Active(active::ActiveServer),
}

// TODO: Remove allow(dead_code) when implemented
#[allow(dead_code)]
pub struct Server {
    pub start_time: Instant,
    pub websocket: WebsocketBucket,
    pub uuid: Uuid,
    remote: remote::ServerApi,
    docker: Arc<Docker>,
    state: State,
}

impl Server {
    /// Creates a bare, uninitiated server.
    pub fn new(
        uuid: Uuid,
        remote: remote::Api,
        docker: Arc<Docker>,
    ) -> Self {
        Server {
            start_time: Instant::now(),
            websocket: WebsocketBucket::new(),
            uuid,
            remote: remote.server_api(uuid),
            docker,
            state: State::Bare,
        }
    }

    /// Switches the egg this server uses.  
    ///
    /// If it was using another egg before, this will forcefully
    /// stop and uninstall the server.
    pub fn switch_egg(&mut self, egg: Egg) {
        if let State::Active(active) = self.state {
            active.force_uninstall();
        }
    }
}

pub mod active;
