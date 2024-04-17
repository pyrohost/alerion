use std::sync::Arc;

use actix_web::{web, HttpResponse, HttpRequest, Responder};
use alerion_datamodel::webserver::CreateServerRequest;
use alerion_datamodel::webserver::update::{ConfigUpdateRequest, ConfigUpdateResponse};
use uuid::Uuid;

use crate::servers::ServerPool;
use crate::webserver::SystemOptions;
use crate::config::AlerionConfig;

pub async fn servers_post(
    opts: web::Json<CreateServerRequest>,
    server_pool: web::Data<Arc<ServerPool>>,
) -> impl Responder {
    match server_pool.create_server(opts.uuid).await {
        Ok(_) => HttpResponse::Accepted(),
        Err(_) => HttpResponse::InternalServerError(),
    }
}

pub async fn system_options() -> impl Responder {
    HttpResponse::NoContent()
}

pub async fn system_get(system_options: web::Data<SystemOptions>) -> impl Responder {
    web::Json(system_options)
}

pub async fn update_post(_payload: web::Json<ConfigUpdateRequest>) -> impl Responder {
    web::Json(ConfigUpdateResponse { applied: false })
}

pub async fn ws(
    req: HttpRequest,
    payload: web::Payload,
    server_uuid: web::Path<Uuid>,
    config: web::Data<AlerionConfig>,
    server_pool: web::Data<Arc<ServerPool>>,
) -> actix_web::Result<HttpResponse> {
    let uuid = server_uuid.into_inner();
    let config = config.into_inner();

    if let Some(server) = server_pool.get_server(uuid).await {
        // if the server doesn't exist well we'll see
        let fut = server.setup_new_websocket(|conn| {
            crate::websocket::start_websocket(uuid, &config, conn, &req, payload)
        });

        fut.await
    } else {
        Ok(HttpResponse::NotImplemented().into())
    }
}

