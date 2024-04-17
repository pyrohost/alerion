use std::sync::Arc;

use actix_web::{web, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::servers::ServerPool;
use crate::webserver::SystemOptions;

#[derive(Serialize, Deserialize)]
pub struct CreateServer {
    uuid: Uuid,
    start_on_completion: bool,
}

pub async fn servers_post(
    opts: web::Json<CreateServer>,
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

pub mod servers;
pub mod update;

pub use update::update_post;
