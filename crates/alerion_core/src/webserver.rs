#![deny(dead_code)]
use std::collections::HashSet;
use std::env::consts::{ARCH, OS};
use std::io;

use bitflags::bitflags;
use futures::{SinkExt, StreamExt};
use jsonwebtoken::{Algorithm, DecodingKey, Validation};
use poem::listener::TcpListener;
use poem::middleware::Cors;
use poem::web::websocket::{Message, WebSocket};
use poem::web::{Json, Path};
use poem::{endpoint, get, handler, EndpointExt, IntoResponse, Route, Server};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sysinfo::System;
use uuid::Uuid;

use self::middleware::bearer_auth::BearerAuthMiddleware;
use crate::config::AlerionConfig;

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
struct Claims {
    iss: String,
    aud: Vec<String>,
    jti: String,
    iat: usize,
    nbf: usize,
    exp: usize,
    server_uuid: Uuid,
    permissions: Vec<String>,
    user_uuid: Uuid,
    user_id: usize,
    unique_id: String,
}

bitflags! {
    #[derive(Debug, Clone, Copy)]
    pub struct Permissions: u32 {
        const CONNECT = 1;
        const START = 1 << 1;
        const STOP = 1 << 2;
        const RESTART = 1 << 3;
        const CONSOLE = 1 << 4;
        const BACKUP_READ = 1 << 5;
        const ADMIN_ERRORS = 1 << 6;
        const ADMIN_INSTALL = 1 << 7;
        const ADMIN_TRANSFER = 1 << 8;
    }
}

impl Permissions {
    pub fn from_strings(strings: &[impl AsRef<str>]) -> Self {
        let mut this = Permissions::empty();

        for s in strings {
            match s.as_ref() {
                "*" => {
                    this.insert(Permissions::CONNECT);
                    this.insert(Permissions::START);
                    this.insert(Permissions::STOP);
                    this.insert(Permissions::RESTART);
                    this.insert(Permissions::CONSOLE);
                    this.insert(Permissions::BACKUP_READ);
                }
                "websocket.connect" => {
                    this.insert(Permissions::CONNECT);
                }
                "control.start" => {
                    this.insert(Permissions::START);
                }
                "control.stop" => {
                    this.insert(Permissions::STOP);
                }
                "control.restart" => {
                    this.insert(Permissions::RESTART);
                }
                "control.console" => {
                    this.insert(Permissions::CONSOLE);
                }
                "backup.read" => {
                    this.insert(Permissions::BACKUP_READ);
                }
                "admin.websocket.errors" => {
                    this.insert(Permissions::ADMIN_ERRORS);
                }
                "admin.websocket.install" => {
                    this.insert(Permissions::ADMIN_INSTALL);
                }
                "admin.websocket.transfer" => {
                    this.insert(Permissions::ADMIN_TRANSFER);
                }
                _ => {}
            }
        }

        this
    }
}

pub struct Auth {
    validation: Validation,
    key: DecodingKey,
}

impl Auth {
    pub fn from_config(cfg: &AlerionConfig) -> Self {
        let mut validation = Validation::new(Algorithm::HS256);

        let spec_claims = ["exp", "nbf", "aud", "iss"].map(ToOwned::to_owned);

        validation.required_spec_claims = HashSet::from(spec_claims);
        validation.leeway = 10;
        validation.reject_tokens_expiring_in_less_than = 0;
        validation.validate_exp = false;
        validation.validate_nbf = false;
        validation.validate_aud = false;
        validation.aud = None;
        validation.iss = Some(HashSet::from([cfg.remote.clone()]));
        validation.sub = None;

        let key = DecodingKey::from_secret(cfg.auth.token.as_ref());

        Self { validation, key }
    }

    pub fn validate(&self, auth: &str, server_uuid: &Uuid) -> Option<Permissions> {
        jsonwebtoken::decode::<Claims>(auth, &self.key, &self.validation)
            .ok()
            .filter(|result| &result.claims.server_uuid == server_uuid)
            .map(|result| Permissions::from_strings(&result.claims.permissions))
    }
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
struct WebsocketEvent {
    event: ServerEventType,
    args: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ClientWebSocketEvent {
    event: ClientEventType,
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
pub enum ClientEventType {
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
pub enum ServerEventType {
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
async fn process_system_query() -> impl IntoResponse {
    let Some(kernel_version) = System::kernel_version() else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };

    Json(SystemResponseV1 {
        architecture: ARCH.to_owned(),
        cpu_count: num_cpus::get(),
        kernel_version,
        os: OS.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
    })
    .into_response()
}

#[handler]
async fn initialize_websocket(Path(_uuid): Path<Uuid>, ws: WebSocket) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| async move {
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let data = serde_json::from_str::<ClientWebSocketEvent>(text.as_str());

            let response = match data {
                Ok(json) => Message::Text(match json.event {
                    ClientEventType::Auth => "auth success".to_owned(),
                    _ => "not implemented".to_owned(),
                }),
                Err(e) => Message::Text(format!("error: {e}")),
            };

            let _ = socket.send(response).await;
        }
    })
}

#[handler]
async fn return_auth_details(Path(_identifier): Path<String>) -> impl IntoResponse {
    todo!()
}

pub async fn serve(config: AlerionConfig) -> io::Result<()> {
    let system_endpoint = get(process_system_query)
        .options(endpoint::make_sync(|_| StatusCode::NO_CONTENT))
        .with(Cors::new().allow_credentials(true))
        .with(BearerAuthMiddleware::new(config.auth.token));

    let api = Route::new().nest(
        "api",
        Route::new().at("system", system_endpoint).nest(
            "servers",
            Route::new().at(":uuid/ws", get(initialize_websocket)),
        ),
    );

    Server::new(TcpListener::bind((config.api.host, config.api.port)))
        .run(api)
        .await
}

pub mod middleware;
