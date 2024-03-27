pub mod websocket;

use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;

async fn ws_api(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let resp = ws::start(websocket::WebSocket {}, &req, stream);
    resp
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| App::new().route("/ws/", web::get().to(ws_api)))
        .bind(("127.0.0.1", 8080))?
        .run()
        .await
}
