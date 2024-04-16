use actix_web::{web, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use relay::ServerConnection;
use uuid::Uuid;

use crate::config::AlerionConfig;

pub fn start_websocket(
    server_uuid: Uuid,
    config: &AlerionConfig,
    conn: ServerConnection,
    req: &HttpRequest,
    payload: web::Payload,
) -> actix_web::Result<(conn::ConnectionAddr, HttpResponse)> {
    let conn = conn::WebsocketConnectionImpl::new(server_uuid, conn, config);
    ws::WsResponseBuilder::new(conn, req, payload).start_with_addr()
}

pub mod auth;
pub mod conn;
pub mod relay;
