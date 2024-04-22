use std::env::consts::{ARCH, OS};
use std::net::SocketAddr;

use futures::{SinkExt, StreamExt};
use poem::listener::TcpListener;
use poem::web::websocket::{Message, WebSocket};
use poem::web::{Path, Query};
use poem::{get, handler, Body, IntoResponse, Response, Route, Server};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sysinfo::System;
use uuid::Uuid;

//const ALLOWED_HEADERS: &str = "Accept, Accept-Encoding, Authorization, Cache-Control, Content-Type, Content-Length, Origin, X-Real-IP, X-CSRF-Token";
//const ALLOWED_METHODS: &str = "GET, POST, PATCH, PUT, DELETE, OPTIONS";

//fn default_headers(config: &AlerionConfig) -> middleware::DefaultHeaders {
//middleware::DefaultHeaders::new()
//.add((header::ACCESS_CONTROL_ALLOW_ORIGIN, config.remote.clone()))
//.add((header::ACCESS_CONTROL_MAX_AGE, 7200))
//.add((header::ACCESS_CONTROL_ALLOW_HEADERS, ALLOWED_HEADERS))
//.add((header::ACCESS_CONTROL_ALLOW_METHODS, ALLOWED_METHODS))
//.add((header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true"))
//}

#[derive(Debug, Serialize, Deserialize)]
struct SystemQuery {
    v: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct SystemResponseV1 {
    architecture: String,
    cpu_count: usize,
    kernel_version: String,
    os: String,
    version: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SystemResponseV2 {
    version: String,
    docker: DockerInfo,
    system: SystemInfo,
}

#[derive(Debug, Serialize, Deserialize)]
struct DockerInfo {
    version: String,
    cgroups: CGroupsInfo,
    containers: ContainersInfo,
    storage: StorageInfo,
    runc: RunCInfo,
}

#[derive(Debug, Serialize, Deserialize)]
struct CGroupsInfo {
    driver: String,
    version: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContainersInfo {
    total: u32,
    running: u32,
    paused: u32,
    stopped: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct StorageInfo {
    driver: String,
    filesystem: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct RunCInfo {
    version: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SystemInfo {
    architecture: String,
    cpu_threads: usize,
    memory_bytes: usize,
    kernel_version: String,
    os: String,
    os_type: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct WebSocketEvent {
    event: String,
    args: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum EventType {
    // Send
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

    // Recieve
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

#[handler]
fn process_system_query(Query(params): Query<SystemQuery>) -> impl IntoResponse {
    match params.v.as_deref() {
        Some("2") => Response::builder()
            .status(StatusCode::NOT_IMPLEMENTED)
            .finish(),
        Some(_) => Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .body("Invalid version"),
        None => {
            let Some(kernel_version) = System::kernel_version() else {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body("The system information could not be fetched.");
            };

            let Ok(response) = Body::from_json(SystemResponseV1 {
                architecture: ARCH.to_owned(),
                cpu_count: num_cpus::get(),
                kernel_version,
                os: OS.to_owned(),
                version: env!("CARGO_PKG_VERSION").to_owned(),
            }) else {
                return Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .finish();
            };

            Response::builder().body(response)
        }
    }
}

#[handler]
async fn initialize_websocket(Path(uuid): Path<Uuid>, ws: WebSocket) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| async move {
        loop {
            if let Some(Ok(Message::Text(text))) = socket.next().await {
                let _ = socket.send(Message::Text(format!("{uuid}"))).await;
                let _ = socket
                    .send(Message::Text(
                        match serde_json::from_str::<WebSocketEvent>(text.as_str()) {
                            Ok(json) => format!("{json:?}"),
                            Err(e) => format!("error: {e}"),
                        },
                    ))
                    .await;
            }
        }
    })
}

pub async fn serve(address: impl Into<SocketAddr>) {
    let api = Route::new().nest(
        "/api",
        Route::new()
            .at("system", get(process_system_query))
            .at("servers/:uuid/ws", get(initialize_websocket)),
    );

    let _ = Server::new(TcpListener::bind(address.into()))
        .run(api)
        .await;
}
