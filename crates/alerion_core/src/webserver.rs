use std::io;
use std::net::SocketAddrV4;
use std::sync::Arc;

use actix_web::http::header;
use actix_web::{dev, guard, middleware, web, App, HttpServer};
use serde::{Deserialize, Serialize};
use utils::bearer_auth::BearerAuth;

use crate::config::AlerionConfig;
use crate::servers::ServerPool;

const ALLOWED_HEADERS: &str = "Accept, Accept-Encoding, Authorization, Cache-Control, Content-Type, Content-Length, Origin, X-Real-IP, X-CSRF-Token";
const ALLOWED_METHODS: &str = "GET, POST, PATCH, PUT, DELETE, OPTIONS";

fn default_headers(config: &AlerionConfig) -> middleware::DefaultHeaders {
    middleware::DefaultHeaders::new()
        .add((header::ACCESS_CONTROL_ALLOW_ORIGIN, config.remote.clone()))
        .add((header::ACCESS_CONTROL_MAX_AGE, 7200))
        .add((header::ACCESS_CONTROL_ALLOW_HEADERS, ALLOWED_HEADERS))
        .add((header::ACCESS_CONTROL_ALLOW_METHODS, ALLOWED_METHODS))
        .add((header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true"))
}

#[derive(Serialize, Deserialize)]
pub struct SystemOptions {
    architecture: &'static str,
    cpu_count: u32,
    kernel_version: &'static str,
    os: &'static str,
    version: &'static str,
}

pub struct Webserver {
    server_fut: dev::Server,
}

impl Webserver {
    /// Build the webserver. May block if a DNS lookup is required to resolve the host
    /// set in the configuration.
    pub fn make(config: AlerionConfig, server_pool: Arc<ServerPool>) -> io::Result<Self> {
        let moved_out_config = config.clone();

        let http_server = HttpServer::new(move || {
            let config = moved_out_config.clone();
            let token = &config.token;

            let base_system_options = SystemOptions {
                architecture: "amd64",
                cpu_count: 8,
                kernel_version: "5.14.0-362.8.1.el9_3.x86_64",
                os: "linux",
                version: "1.11.11",
            };

            App::new()
                .app_data(web::Data::new(base_system_options))
                .app_data(web::Data::new(config.clone()))
                .app_data(web::Data::new(Arc::clone(&server_pool)))
                .wrap(default_headers(&config))
                .wrap(utils::camel_case::CamelCaseHeaders)
                .wrap(middleware::Logger::new("%r"))
                .route("/", web::get().to(router::root))
                .service({
                    use router::api;

                    web::scope("/api")
                        .route(
                            "servers",
                            web::post()
                                .wrap(BearerAuth::new(token.clone()))
                                .to(api::servers_post),
                        )
                        .route(
                            "system",
                            web::get()
                                .wrap(BearerAuth::new(token.clone()))
                                .to(api::system_get),
                        )
                        .route(
                            "system",
                            web::route().guard(guard::Options()).to(api::system_options),
                        )
                        .route(
                            "update",
                            web::post()
                                .wrap(BearerAuth::new(token.clone()))
                                .to(api::update_post),
                        )
                        .service(
                            web::scope("/servers/{id}")
                                .route("ws", web::get().to(api::servers::ws)),
                        )
                })
        });

        let ip = config.api.host;
        let port = config.api.port;

        // only set a low amount of workers, because the webserver
        // isn't gonna handle a ton of requests and we want alerion
        // as a whole to use as little resources as possible, to
        // leave room for the actual servers.
        let server_fut = http_server
            .worker_max_blocking_threads(16)
            .workers(1)
            .bind(SocketAddrV4::new(ip, port))?
            .run();

        Ok(Webserver { server_fut })
    }

    pub async fn serve(self) -> io::Result<()> {
        self.server_fut.await
    }
}

pub mod router;
pub mod utils;
