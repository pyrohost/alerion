use std::net::SocketAddr;

use warp::Filter;

const ALLOWED_HEADERS: &str = "Accept, Accept-Encoding, Authorization, Cache-Control, Content-Type, Content-Length, Origin, X-Real-IP, X-CSRF-Token";
const ALLOWED_METHODS: &str = "GET, POST, PATCH, PUT, DELETE, OPTIONS";

//fn default_headers(config: &AlerionConfig) -> middleware::DefaultHeaders {
//middleware::DefaultHeaders::new()
//.add((header::ACCESS_CONTROL_ALLOW_ORIGIN, config.remote.clone()))
//.add((header::ACCESS_CONTROL_MAX_AGE, 7200))
//.add((header::ACCESS_CONTROL_ALLOW_HEADERS, ALLOWED_HEADERS))
//.add((header::ACCESS_CONTROL_ALLOW_METHODS, ALLOWED_METHODS))
//.add((header::ACCESS_CONTROL_ALLOW_CREDENTIALS, "true"))
//}

pub async fn serve(address: impl Into<SocketAddr>) {
    warp::serve(warp::path!("test" / String).map(|name| format!("Hello {}!", name)))
        .run(address)
        .await
}
