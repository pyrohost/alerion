use std::fmt;
use std::sync::Arc;

use futures::stream::{FuturesUnordered, SplitSink, StreamExt};
use futures::sink::{Sink, SinkExt};
use futures::future::ready;
use poem::web::websocket::{Message, WebSocketStream};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use smallvec::{smallvec, SmallVec};
use uuid::Uuid;

use crate::servers::server::{Server, ProcState, InboundMessage, OutboundMessage};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecvWebsocketEvent {
    event: RecvEventType,
    args: Option<SmallVec<[serde_json::Value; 1]>>,
}

impl RecvWebsocketEvent {
    pub fn into_arg_as_string(self) -> Option<String> {
        self.args
            .and_then(|mut sv| sv.pop())
            .and_then(|v| match v {
                serde_json::Value::String(s) => Some(s),
                _ => None,
            })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SendWebsocketEvent<'a> {
    event: SendEventType,
    args: Option<SmallVec<[&'a str; 1]>>,
}

impl<'a> SendWebsocketEvent<'a> {
    pub fn console_output(output: &'a str) -> Self {
        Self {
            event: SendEventType::ConsoleOutput,
            args: Some(smallvec![output]),
        }
    }

    pub fn install_output(output: &'a str) -> Self {
        Self {
            event: SendEventType::InstallOutput,
            args: Some(smallvec![output]),
        }
    }

    pub fn auth_success() -> Self {
        Self {
            event: SendEventType::AuthSuccess,
            args: None,
        }
    }

    pub fn status(status: ProcState) -> Self {
        Self {
            event: SendEventType::Status,
            args: Some(smallvec![status.to_str()])
        }
    }

    /// Serializes the event and produces an error message if serialization fails.
    pub fn serialize(&self) -> Option<String> {
        let out = serde_json::to_string(self);

        if let Err(ref e) = out {
            tracing::error!("failed to serialize websocket event: {e}");
        }

        out.ok()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthDetails {
    data: AuthDetailsInner,
}

#[derive(Debug, Serialize, Deserialize)]
struct AuthDetailsInner {
    token: String,
    socket: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RecvEventType {
    #[serde(rename = "auth")]
    Auth,
    #[serde(rename = "set state")]
    SetState,
    #[serde(rename = "send command")]
    SendCommand,
    #[serde(rename = "send logs")]
    SendLogs,
    #[serde(rename = "send stats")]
    SendStats,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum SendEventType {
    #[serde(rename = "auth success")]
    AuthSuccess,
    #[serde(rename = "backup complete")]
    BackupComplete,
    #[serde(rename = "backup restore completed")]
    BackupRestoreCompleted,
    #[serde(rename = "console output")]
    ConsoleOutput,
    #[serde(rename = "daemon error")]
    DaemonError,
    #[serde(rename = "daemon message")]
    DaemonMessage,
    #[serde(rename = "install completed")]
    InstallCompleted,
    #[serde(rename = "install output")]
    InstallOutput,
    #[serde(rename = "install started")]
    InstallStarted,
    #[serde(rename = "jwt error")]
    JwtError,
    #[serde(rename = "stats")]
    Stats,
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "token expired")]
    TokenExpired,
    #[serde(rename = "token expiring")]
    TokenExpiring,
    #[serde(rename = "transfer logs")]
    TransferLogs,
    #[serde(rename = "transfer status")]
    TransferStatus,
}

pub async fn websocket_handler(
    server: Arc<Server>,
    stream: WebSocketStream,
    _uuid: Uuid,
    auth: auth::Auth,
) {
    let (mut sink, mut stream) = stream.split();
    let chan = server.websocket.add();
    let (mut antenna, passage) = chan.split();

    /// Tries sending the given event into the sink, handling all errors.
    async fn try_send<S>(sink: &mut S, event: SendWebsocketEvent<'_>)
    where
        S: Sink<Message> + Unpin,
        <S as Sink<Message>>::Error: fmt::Display,
    {
        if let Some(t) = event.serialize() {
            if let Err(e) = sink.send(Message::Text(t)).await {
                tracing::debug!("failed to send through websocket connection: {e}");
            }
        }
    }

    async fn batch_send<S>(sink: &mut S, events: &[SendWebsocketEvent<'_>])
    where
        S: Sink<Message> + Unpin,
        <S as Sink<Message>>::Error: fmt::Display,
    {
        let mut futs = events.iter()
            // silently ignore events that can't serialize (there is no way this would happen
            // anyways. right.
            .filter_map(SendWebsocketEvent::serialize)
            .map(Message::Text)
            .map(Ok)
            .map(ready)
            .collect::<FuturesUnordered<_>>();

        if let Err(e) = sink.send_all(&mut futs).await {
            tracing::error!("failed to send through websocket connection: {e}");
        }
    }

    async fn handle_incoming(
        server: &Server,
        msg: RecvWebsocketEvent,
        state: &mut Option<CurrentAuth>,
        sink: &mut SplitSink<WebSocketStream, Message>,
        _passage: &mpsc::Sender<InboundMessage>,
        auth: &auth::Auth,
    ) {
        match msg.event {
            RecvEventType::Auth => {
                tracing::debug!("received AUTH event");

                if let Some(jwt) = msg.into_arg_as_string() {
                    match auth.validate(&jwt) {
                        Ok(perms) if perms.contains(auth::Permissions::CONNECT) => {
                            tracing::debug!("authenticated");
                            *state = Some(CurrentAuth::new(perms));

                            let proc_state = server.get_proc_state();

                            batch_send(sink, &[
                                SendWebsocketEvent::auth_success(),
                                SendWebsocketEvent::status(proc_state),
                            ]).await;
                        }
                        Ok(_) => {
                            tracing::debug!("authentication OK, but no CONNECT permissions");
                        }
                        Err(e) => {
                            tracing::debug!("authentication failed: {e}");
                        }
                    }
                }
            }

            RecvEventType::SendLogs => {
                tracing::debug!("received SEND LOGS event");

                if let Some(ref mut state) = state.as_mut() {
                    if state.perms.contains(auth::Permissions::CONSOLE) {
                        state.wants_logs = true;
                    }
                }
            }

            RecvEventType::SendStats => {
                tracing::debug!("received SEND STATS event");

                if let Some(ref mut state) = state.as_mut() {
                    state.wants_stats = true;
                }
            }

            _ => todo!(),
        }
    }

    struct CurrentAuth {
        perms: auth::Permissions,
        wants_logs: bool,
        wants_stats: bool,
        wants_install_logs: bool,
    }

    impl CurrentAuth {
        pub fn new(perms: auth::Permissions) -> Self {
            Self {
                perms,
                wants_logs: false,
                wants_stats: false,
                wants_install_logs: false,
            }
        }
    }

    let mut state = None::<CurrentAuth>;

    loop {
        tokio::select! {
            biased;

            Ok(msg) = antenna.recv() => {
                match msg {
                    OutboundMessage::ServerOutput { output } => {
                        if state.as_ref().is_some_and(|s| s.wants_logs) {
                            try_send(&mut sink, SendWebsocketEvent::console_output(output.as_str())).await;
                        }
                    }

                    OutboundMessage::InstallOutput { output } => {
                        if state.as_ref().is_some_and(|s| s.wants_install_logs) {
                            try_send(&mut sink, SendWebsocketEvent::install_output(output.as_str())).await;
                        }
                    }
                }
            },

            item = stream.next() => {
                let Some(result) = item else { break; };
                let Some(strong_passage) = passage.upgrade() else {
                    break;
                };

                if let Ok(msg) = result {
                    match msg {
                        Message::Text(text) => {
                            let Ok(data) = serde_json::from_str::<RecvWebsocketEvent>(text.as_str()) else {
                                tracing::debug!("error: bad websocket input");
                                continue;
                            };

                            handle_incoming(&server, data, &mut state, &mut sink, &strong_passage, &auth).await;
                        }

                        _ => todo!(),
                    }
                }
            }
        }
    }
}

pub mod auth;
