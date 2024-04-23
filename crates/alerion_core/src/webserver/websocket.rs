use std::sync::Arc;

use futures::stream::{SplitSink, StreamExt};
use poem::web::websocket::{Message, WebSocketStream};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct RecvWebsocketEvent {
    event: RecvEventType,
    args: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SendWebsocketEvent {
    event: SendEventType,
    args: Option<Vec<String>>,
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

#[derive(Debug, Serialize, Deserialize)]
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

#[derive(Debug, Serialize, Deserialize)]
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
    stream: WebSocketStream,
    _recv: mpsc::Receiver<SendWebsocketEvent>,
    uuid: Uuid,
) {
    let (sink, mut stream) = stream.split();
    let sink = Arc::new(Mutex::new(sink));

    let direct_responder = Arc::clone(&sink);
    let inbound_handle = tokio::spawn(async move {
        while let Some(result) = stream.next().await {
            if let Ok(msg) = result {
                match msg {
                    Message::Text(text) => {
                        let data = serde_json::from_str::<RecvWebsocketEvent>(text.as_str());

                        match data {
                            Ok(json) => handle_incoming_message(json, &direct_responder, uuid),
                            Err(_e) => todo!(),
                        }
                    }

                    Message::Close(_maybe_reason) => {
                        return;
                    }

                    _ => {
                        // TODO
                        return;
                    }
                }
            }
        }
    });

    let outbound_handle = tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            //sink.send()
        }
    });

    tokio::select! {
        _ = inbound_handle => {},
        _ = outbound_handle => {}
    }
}

fn handle_incoming_message(
    msg: RecvWebsocketEvent,
    _sink: &Mutex<SplitSink<WebSocketStream, Message>>,
    _uuid: Uuid,
) {
    match msg.event {
        RecvEventType::Auth => {
            // handle auth..
        }

        _ => todo!(),
    }
}

pub mod auth;
