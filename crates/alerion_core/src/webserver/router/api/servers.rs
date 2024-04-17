use std::sync::Arc;

use actix_web::{web, HttpRequest, HttpResponse};
use uuid::Uuid;

use crate::config::AlerionConfig;
use crate::servers::ServerPool;

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
