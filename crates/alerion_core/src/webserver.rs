use std::env::consts::{ARCH, OS};
use std::io;
use std::sync::Arc;

use alerion_datamodel::webserver::CreateServerRequest;
use poem::listener::TcpListener;
use poem::middleware::Cors;
use poem::web::websocket::WebSocket;
use poem::web::{Data, Json, Path};
use poem::{endpoint, get, handler, post, EndpointExt, IntoResponse, Route, Server};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use sysinfo::System;
use uuid::Uuid;

use self::middleware::bearer_auth::BearerAuthMiddleware;
use crate::fs::Config;
use crate::servers::pool::ServerPool;

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
    Data(config): Data<&Config>,
    ws: WebSocket,
) -> impl IntoResponse {
    if let Some(server) = server_pool.get(uuid).await {
        let auth = websocket::auth::Auth::from_config(config);

        let resp = ws.on_upgrade(move |mut socket| {
            tracing::info!("upgraded websocket");
            websocket::websocket_handler(server, socket, uuid, auth)
        });

        resp.into_response()
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

#[handler]
async fn create_server(
    Json(options): Json<CreateServerRequest>,
    Data(server_pool): Data<&Arc<ServerPool>>,
) -> impl IntoResponse {
    let _server = match server_pool.get(options.uuid).await {
        Some(s) => s,
        None => {
            let server_fut = server_pool.create(options.uuid, options.start_on_completion);

            match server_fut.await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("error occured when creating a server: {e}");
                    return StatusCode::INTERNAL_SERVER_ERROR.into_response();
                }
            }
        }
    };

    ().into_response()
}

pub async fn serve(config: Config, server_pool: Arc<ServerPool>) -> io::Result<()> {
    // TODO: restrict origins
    let cors = Cors::new().allow_credentials(true);

    let system_endpoint = get(get_system_info)
        .options(endpoint::make_sync(|_| StatusCode::NO_CONTENT))
        .with(BearerAuthMiddleware::new(config.auth.token.clone()));

    let ws_endpoint = get(initialize_websocket);

    let install_endpoint =
        post(create_server).with(BearerAuthMiddleware::new(config.auth.token.clone()));

    let bound = (config.api.host, config.api.port);

    let api = Route::new()
        .nest(
            "api",
            Route::new()
                .at("system", system_endpoint)
                .at("servers", install_endpoint)
                .at("servers/:uuid/ws", ws_endpoint),
        )
        .with(cors)
        .with(middleware::tracing::Tracing)
        .data(config)
        .data(server_pool);

    Server::new(TcpListener::bind(bound))
        .run(api)
        .await
}

pub mod middleware;
pub mod websocket;
