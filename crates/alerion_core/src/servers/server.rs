use std::sync::Arc;
use std::time::Instant;

pub use active::Egg;
use bollard::Docker;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::servers::remote;

use super::ServerError;

pub struct ServerChannel {
    antenna: broadcast::Receiver<ServerMessage>,
}

#[derive(Clone, Debug)]
pub enum ServerMessage {
    ConsoleOutput { output: Arc<String> },

    DaemonOutput { output: Arc<String> },

    DaemonError { output: Arc<String> },
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
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let (broadcaster, _) = broadcast::channel(64);

        Self { broadcaster }
    }

    /// "Adds" a receiver to this bucket. Just drop the returned `ServerChannel` to unsubscribe.
    pub fn add(&self) -> ServerChannel {
        ServerChannel {
            antenna: self.broadcaster.subscribe(),
        }
    }

    /// Broadcast a message to all receivers.
    pub fn broadcast(&self, msg: ServerMessage) {
        // an error means there are no receivers ready, which is fine
        let _ = self.broadcaster.send(msg);
    }
}

#[derive(Debug)]
pub enum State {
    /// Empty server, with no egg set.
    Bare,
    Active(active::ActiveServer),
}

impl State {
    pub fn is_bare(&self) -> bool {
        matches!(self, State::Bare)
    }

    pub fn is_active(&self) -> bool {
        matches!(self, State::Active(_))
    }
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
    pub fn new(uuid: Uuid, remote: remote::Api, docker: Arc<Docker>) -> Self {
        Server {
            start_time: Instant::now(),
            websocket: WebsocketBucket::new(),
            uuid,
            remote: remote.server_api(uuid),
            docker,
            state: State::Bare,
        }
    }

    pub async fn installation_process(&self) -> Result<(), ServerError> {
        // Server must not be active
        if self.state.is_active() {
            tracing::error!("tried to begin installation process, but server is already active");
            return Err(ServerError::Conflict);
        }

        // Get remote server configuration
        let cfg = self.remote.get_server_configuration().await?;
        
        

        Ok(())
    }

    /// Switches the egg this server uses.  
    ///
    /// If it was using another egg before, this will forcefully
    /// stop and uninstall the server.
    pub fn switch_egg(&mut self, _egg: Egg) {
        if let State::Active(ref active) = self.state {
            active.force_uninstall();
        }
    }

    pub(crate) fn docker_api(&self) -> &Docker {
        &self.docker
    }
}

pub mod active;
