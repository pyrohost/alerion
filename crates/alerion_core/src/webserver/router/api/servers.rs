use std::sync::Arc;
use actix_web::{HttpRequest, HttpResponse};
use actix_web::web;
use uuid::Uuid;
use crate::config::AlerionConfig;
use crate::servers::ServerPool;
use crate::websocket::relay::ClientConnection;

pub async fn ws(
    req: HttpRequest,
    payload: web::Payload,
    server_uuid: web::Path<Uuid>,
    config: web::Data<AlerionConfig>,
    server_pool: web::Data<Arc<ServerPool>>,
) -> actix_web::Result<HttpResponse> {
    let uuid = server_uuid.into_inner();
    let config = config.into_inner();

    if let Some(server) = server_pool.get(uuid).await {
        // if the server doesn't exist well we'll see
        let (conn, auth_tracker) = server.new_connection_with_auth_tracker();
        let (addr, resp) = crate::websocket::start_websocket(uuid, &config, conn, &req, payload)?;

        server.add_websocket(ClientConnection::new(auth_tracker, addr)).await;

        Ok(resp)
    } else {
        Ok(HttpResponse::NotImplemented().into())
    }


}
