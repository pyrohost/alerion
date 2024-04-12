use std::sync::Arc;
use actix_web::{HttpRequest, HttpResponse};
use actix_web::web;
use uuid::Uuid;
use alerion_config::AlerionConfig;
use alerion_websocket::WebsocketConnection;
use alerion_servers::InstallPool;

pub async fn ws(
    req: HttpRequest,
    payload: web::Payload,
    server_id: web::Path<Uuid>,
    config: web::Data<AlerionConfig>,
    install_pool: web::Data<Arc<InstallPool>>,
) -> actix_web::Result<HttpResponse> {
    let uuid = server_id.into_inner();

    let conn = WebsocketConnection::new(uuid.clone(), &config.into_inner());
    let (addr, resp) = conn.start(&req, payload)?;

    install_pool.into_inner().push(uuid, addr);

    Ok(resp)
}
