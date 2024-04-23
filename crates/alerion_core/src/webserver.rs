use std::sync::Arc;
use std::env::consts::{ARCH, OS};
use std::io;

use poem::listener::TcpListener;
use poem::middleware::Cors;
use poem::web::websocket::WebSocket;
use poem::web::{Json, Path};
use poem::{endpoint, get, handler, EndpointExt, IntoResponse, Route, Server};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sysinfo::System;
use uuid::Uuid;

use self::middleware::bearer_auth::BearerAuthMiddleware;
use crate::config::AlerionConfig;
use crate::servers::ServerPool;

#[derive(Debug, Serialize, Deserialize)]
struct SystemResponseV1 {
    architecture: String,
    cpu_count: usize,
    kernel_version: String,
    os: String,
    version: String,
}

#[handler]
async fn get_system_info() -> impl IntoResponse {
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
async fn initialize_websocket(Path(uuid): Path<Uuid>, ws: WebSocket) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| {
        websocket::websocket_handler(socket, uuid)
    })
}

pub async fn serve(config: &AlerionConfig) -> io::Result<()> {
    // TODO: restrict origins
    let cors = Cors::new().allow_credentials(true);

    let system_endpoint = get(get_system_info)
        .options(endpoint::make_sync(|_| StatusCode::NO_CONTENT))
        .with(BearerAuthMiddleware::new(config.auth.token.clone()));

    let ws_endpoint = get(initialize_websocket);

    let api = Route::new()
        .nest_no_strip(
            "api",
            Route::new()
                .at("system", system_endpoint)
                .at("servers/:uuid/ws", ws_endpoint),
        )
        .with(cors);

    Server::new(TcpListener::bind((config.api.host, config.api.port)))
        .run(api)
        .await
}

pub mod middleware;
pub mod websocket;
