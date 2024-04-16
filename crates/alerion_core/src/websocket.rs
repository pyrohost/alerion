use actix_web::{HttpRequest, HttpResponse};
use actix_web::web;
use actix_web_actors::ws;
use uuid::Uuid;
use relay::ServerConnection;
use crate::config::AlerionConfig;


#[derive(Debug, Default)]
struct Permissions {
    pub connect: bool,
}

impl Permissions {
    pub fn from_strings(strings: &[impl AsRef<str>]) -> Self {
        let mut this = Permissions::default();

        for s in strings {
            match s.as_ref() {
                "*" => {
                    this.connect = true;
                }
                "websocket.connect" => { this.connect = true; }
                _what => {
                    // unknown permission..
                }
            }
        }

        this
    }
}

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

pub mod relay;
pub mod auth;
pub mod conn;
pub mod message;
