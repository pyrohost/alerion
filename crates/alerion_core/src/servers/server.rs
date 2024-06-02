use std::sync::Arc;
use std::time::Instant;

use bollard::Docker;
use tokio::sync::{mpsc, broadcast};
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use alerion_datamodel as dm;

use crate::servers::remote;
use crate::fs::{db, FsLogger, LocalDataPaths, Mounts};
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum State {
    /// Empty server, with no egg set.
    Bare,
    /// Installation process ongoing.
    Installing,
    /// Unstartable because the installation failed or something else
    /// brought the server to not be healthy.
    Unhealthy,
    /// Installed, but offline
    Offline,
    /// Starting
    Starting,
    /// Running
    Running,
    /// Stopping
    Stopping,
}

impl State {
    pub fn from_installation_success(success: bool) -> Self {
        if success {
            State::Offline
        } else {
            State::Unhealthy
        }
    }

    pub fn is_bare(&self) -> bool {
        matches!(self, State::Bare)
    }

    pub fn as_datamodel_status(&self) -> dm::websocket::ServerStatus {
        use dm::websocket::ServerStatus;

        match self {
            State::Starting => ServerStatus::Starting,
            State::Running => ServerStatus::Running,
            State::Stopping => ServerStatus::Stopping,
            _ => ServerStatus::Offline,
        }
    }
}

pub struct Fs {
    pub mounts: Mounts,
    pub logger: FsLogger,
    pub db: db::Handle,
}

// TODO: Remove allow(dead_code) when implemented
#[allow(dead_code)]
pub struct Server {
    pub start_time: Instant,
    pub websocket: WebsocketBucket,
    pub uuid: Uuid,
    pub(crate) fs: Fs,
    pub(crate) remote: remote::ServerApi,
    pub(crate) docker: Arc<Docker>,
    pub(crate) autostart: bool,
}

impl Server {
    /// Creates a server. This will spawn a task for it's installation
    /// process if needed.
    pub async fn new(
        uuid: Uuid,
        remote: remote::Api,
        docker: Arc<Docker>,
        localdata: &LocalDataPaths,
        autostart: bool,
    ) -> Result<Arc<Self>, ServerError> {
        let (websocket, mut ws_receiver) = WebsocketBucket::new();

        let server = Arc::new(Server {
            start_time: Instant::now(),
            websocket,
            uuid,
            remote: remote.server_api(uuid),
            docker,
            autostart,
            fs: Fs {
                mounts: localdata.mounts_of(uuid),
                db: localdata.db_of(uuid).await,
                logger: localdata.logger(uuid).await?,
            },
        });

        tokio::spawn(async move {
            while let Some(_msg) = ws_receiver.recv().await {
            }
        });

        // don't care about error
        let _ = Server::install_if_appropriate(&server).await;

        Ok(server)
    }

    /// Spawns a task for the installation process, if there's no state conflict.
    pub async fn install_if_appropriate(this: &Arc<Server>) -> Result<(), ServerError> {
        // Server must not be active
        if !this.get_state().is_bare() {
            return Err(ServerError::Conflict);
        }

        tokio::spawn(docker::install::engage(Arc::clone(this)));

        Ok(())
    }

    pub async fn mark_install_status(this: Arc<Server>, success: bool) {
        let server_online = Arc::clone(&this);
        let server_localdb = this;
        
        let fut_online_status = async move {
            match server_online.remote.post_installation_status(success, false).await {
                Ok(()) => tracing::debug!("notified remote API of installation status"),
                Err(e) => tracing::error!("couldn't notify the panel about the installation status: {e}"),
            }
        };

        let fut_db_update = async {
            // logging handled within DB
            let state = State::from_installation_success(success);
            server_localdb.fs.db.update(|s| {
                s.state = state;
            }).await;
        };

        tokio::join!(fut_online_status, fut_db_update);
    }

    pub fn get_state(&self) -> State {
        self.fs.db.get().state
    }
}

