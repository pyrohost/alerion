use std::env::consts::{ARCH, OS};
use std::io;
use std::sync::Arc;

use alerion_datamodel::webserver::CreateServerRequest;
use poem::listener::TcpListener;
use poem::middleware::{Tracing, Cors};
use poem::web::websocket::WebSocket;
use poem::web::{Data, Json, Path};
use poem::{endpoint, get, handler, post, EndpointExt, IntoResponse, Route, Server};
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
async fn initialize_websocket(
    Path(uuid): Path<Uuid>,
    Data(server_pool): Data<&Arc<ServerPool>>,
    ws: WebSocket,
) -> impl IntoResponse {
    if let Some(server) = server_pool.get_server(uuid).await {
        let recv = server.add_websocket_connection().await;

        ws.on_upgrade(move |mut socket| websocket::websocket_handler(socket, recv, uuid))
            .into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[handler]
async fn create_server(Json(options): Json<CreateServerRequest>, Data(server_pool): Data<&Arc<ServerPool>>) -> impl IntoResponse {
    let _server = match server_pool.get_server(options.uuid).await {
        Some(s) => s,
        None => {
            let server_fut = server_pool.register_server(options.uuid, options.start_on_completion);
            let Ok(server) = server_fut.await else {
                return StatusCode::INTERNAL_SERVER_ERROR.into_response();
            };

            server
        }
    };

    ().into_response()
}

pub async fn serve(config: &AlerionConfig, server_pool: Arc<ServerPool>) -> io::Result<()> {
    // TODO: restrict origins
    let cors = Cors::new().allow_credentials(true);

    let system_endpoint = get(get_system_info)
        .options(endpoint::make_sync(|_| StatusCode::NO_CONTENT))
        .with(BearerAuthMiddleware::new(config.auth.token.clone()));

    let ws_endpoint = get(initialize_websocket);

    let install_endpoint = post(create_server)
        .with(BearerAuthMiddleware::new(config.auth.token.clone()));

    let api = Route::new()
        .nest(
            "api",
            Route::new()
                .at("system", system_endpoint)
                .at("servers", install_endpoint)
                .at("servers/:uuid/ws", ws_endpoint),
        )
        .with(cors)
        .with(Tracing)
        .data(server_pool);

    Server::new(TcpListener::bind((config.api.host, config.api.port)))
        .run(api)
        .await
}

pub mod middleware;
pub mod websocket;
