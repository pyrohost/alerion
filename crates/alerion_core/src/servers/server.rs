use std::sync::Arc;
use std::time::Instant;

use bollard::Docker;
use tokio::sync::{mpsc, broadcast};
use uuid::Uuid;
use alerion_datamodel as dm;
use parking_lot::Mutex as PlMutex;
use serde::Serialize;

use crate::servers::remote;
use crate::fs::{FsLogger, LocalDataHandle, LogFileInterface};
use crate::docker;

use super::ServerError;

pub struct ServerChannel {
    antenna: broadcast::Receiver<OutboundMessage>,
    passage: mpsc::WeakSender<InboundMessage>,
}

impl ServerChannel {
    pub fn split(self) -> (broadcast::Receiver<OutboundMessage>, mpsc::WeakSender<InboundMessage>) {
        (self.antenna, self.passage)
    }
}

#[derive(Clone, Debug)]
pub enum OutboundMessage {
    ServerOutput { output: Arc<String> },

    InstallOutput { output: Arc<String >},
}

impl OutboundMessage {
    pub fn server_output(output: Arc<String>) -> Self {
        Self::ServerOutput { output }
    }

    pub fn install_output(output: Arc<String>) -> Self {
        Self::InstallOutput{ output }
    }
}

#[derive(Clone, Debug)]
pub enum InboundMessage {
    SetState(dm::websocket::StateUpdate),
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
    broadcaster: broadcast::Sender<OutboundMessage>,
    receiver_sender: mpsc::Sender<InboundMessage>,
}

impl WebsocketBucket {
    #[allow(clippy::new_without_default)]
    pub fn new() -> (Self, mpsc::Receiver<InboundMessage>) {
        let (broadcaster, _) = broadcast::channel(64);
        let (receiver_sender, receiver) = mpsc::channel(64);

        let this = Self {
            broadcaster,
            receiver_sender,
        };

        (this, receiver)
    }

    /// "Adds" a receiver to this bucket. Just drop the returned `ServerChannel` to unsubscribe.
    pub fn add(&self) -> ServerChannel {
        ServerChannel {
            antenna: self.broadcaster.subscribe(),
            passage: self.receiver_sender.downgrade(),
        }
    }

    /// Broadcast a message to all receivers.
    pub fn broadcast(&self, msg: OutboundMessage) {
        // an error means there are no receivers ready, which is fine
        let _ = self.broadcaster.send(msg);
    }
}

#[derive(Debug)]
pub enum State {
    /// Empty server, with no egg set.
    Bare,
    Installing,
    Installed,
}

impl State {
    pub fn is_bare(&self) -> bool {
        matches!(self, State::Bare)
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub enum ProcState {
    #[serde(rename = "offline")]
    Offline,
    #[serde(rename = "starting")]
    Starting,
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "stopping")]
    Stopping,
}

impl ProcState {
    pub fn to_str(self) -> &'static str {
        match self {
            ProcState::Offline => "offline",
            ProcState::Starting => "starting",
            ProcState::Running => "running",
            ProcState::Stopping => "stopping",
        }
    }
}

// TODO: Remove allow(dead_code) when implemented
#[allow(dead_code)]
pub struct Server {
    pub start_time: Instant,
    pub websocket: WebsocketBucket,
    pub uuid: Uuid,
    pub(crate) state: PlMutex<State>,
    pub(crate) proc_state: PlMutex<ProcState>,
    remote: remote::ServerApi,
    docker: Arc<Docker>,
    localdata: LocalDataHandle,
    logger: FsLogger,
}

impl Server {
    /// Creates a bare, uninitiated server.
    pub async fn new(uuid: Uuid, remote: remote::Api, docker: Arc<Docker>, localdata: LocalDataHandle) -> Result<Arc<Self>, ServerError> {
        let (websocket, mut ws_receiver) = WebsocketBucket::new();

        let server = Arc::new(Server {
            start_time: Instant::now(),
            websocket,
            uuid,
            remote: remote.server_api(uuid),
            docker,
            state: PlMutex::new(State::Bare),
            proc_state: PlMutex::new(ProcState::Offline),
            logger: localdata.logger(uuid).await?,
            localdata,
        });

        tokio::spawn(async move {
            while let Some(_msg) = ws_receiver.recv().await {
            }
        });

        Ok(server)
    }

    pub fn start_installation(this: Arc<Server>) -> Result<(), ServerError> {
        // Server must not be active
        let mut lock = this.state.lock();
        if !lock.is_bare() {
            tracing::error!("tried to begin installation process, but server is already active");
            return Err(ServerError::Conflict);
        }

        *lock = State::Installing;

        drop(lock);
        tokio::spawn(installation_process(this));

        Ok(())
    }

    pub async fn install_logfile(&self) -> LogFileInterface {
        self.logger.open_install().await
    }

    pub async fn server_logfile(&self) -> LogFileInterface {
        self.logger.open_server().await
    }

    pub async fn set_installation_status(&self, success: bool) -> Result<(), ServerError> {
        self.remote.post_installation_status(success, false).await?;
        Ok(())
    }

    pub(crate) fn docker_api(&self) -> &Docker {
        &self.docker
    }

    pub(crate) fn localdata(&self) -> &LocalDataHandle {
        &self.localdata
    }

    pub(crate) fn get_proc_state(&self) -> ProcState {
        *self.proc_state.lock()
    }
}

async fn installation_process(server: Arc<Server>) -> Result<(), ServerError> {
    // Get remote server configuration
    let server_cfg = server.remote.get_server_configuration().await?;
    let install_cfg = server.remote.get_install_instructions().await?;
 
    docker::install::engage(&server, &server_cfg, install_cfg).await?;

    Ok(())
}

