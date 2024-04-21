use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::mpsc::Sender;

use crate::websocket::conn::{ConnectionAddr, PanelMessage, ServerMessage};

pub struct ServerConnection {
    auth_tracker: Arc<AuthTracker>,
    sender: Sender<(u32, PanelMessage)>,
    id: u32,
}

impl ServerConnection {
    pub fn new(auth_tracker: Arc<AuthTracker>, sender: Sender<(u32, PanelMessage)>, id: u32) -> Self {
        ServerConnection {
            auth_tracker,
            sender,
            id,
        }
    }

    pub fn set_authenticated(&self) {
        self.auth_tracker.set_auth(true);
    }

    pub fn is_authenticated(&self) -> bool {
        self.auth_tracker.get_auth()
    }

    pub fn send_if_authenticated(&self, msg: PanelMessage) {
        if self.auth_tracker.get_auth() {
            let _ = self.sender.try_send((self.id, msg));
        }
    }

    pub fn force_send(&self, msg: PanelMessage) {
        let _ = self.sender.try_send((self.id, msg));
    }

    pub fn auth_tracker(&self) -> Arc<AuthTracker> {
        Arc::clone(&self.auth_tracker)
    }
}

pub struct ClientConnection {
    auth_tracker: Arc<AuthTracker>,
    ws_sender: ConnectionAddr,
}

impl ClientConnection {
    pub fn new(auth_tracker: Arc<AuthTracker>, ws_sender: ConnectionAddr) -> Self {
        Self {
            auth_tracker,
            ws_sender,
        }
    }

    /// Uses a closure because many messages might be expensive to compute.
    pub fn send_if_authenticated<F>(&self, msg: F)
    where
        F: FnOnce() -> ServerMessage
    {
        if self.auth_tracker.get_auth() {
            let m = msg();
            self.ws_sender.do_send(m);
        }
    }

    /// Terminate the connection on the server's side.  
    ///
    /// There could be a condition where the server tries to terminate the connection,
    /// sets the auth bool to false and tells the websocket to kill itself. Before the
    /// websocket connection is actually terminated, the client to re-authenticate and
    /// send more messages to the server. This should be a non-issue: who cares if
    /// the client manages to send a few more frames; the connection will eventually
    /// terminate.
    ///
    /// This would easily be fixable with another atomic check, but I'd rather avoid
    /// seemingly unnecessary atomic loads.
    pub fn terminate(&self) {
        self.expire_auth();
        self.ws_sender.do_send(ServerMessage::Kill);
    }

    pub fn expire_auth(&self) {
        self.auth_tracker.set_auth(false);
    }

    pub fn is_authenticated(&self) -> bool {
        self.auth_tracker.get_auth()
    }
}

/// A middleman between a websocket connection and a server, which keeps track of
/// auth state and the status of the websocket connection.
pub struct AuthTracker {
    started_at: AtomicU64,
    authenticated: AtomicBool,
}

impl AuthTracker {
    pub fn new(server_time: u64) -> Self {
        Self {
            started_at: AtomicU64::new(server_time),
            authenticated: AtomicBool::new(false),
        }
    }

    pub fn set_auth(&self, value: bool) {
        self.authenticated.store(value, Ordering::SeqCst);
    }

    pub fn get_auth(&self) -> bool {
        self.authenticated.load(Ordering::SeqCst)
    }
}
