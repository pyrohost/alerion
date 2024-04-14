use actix_web::{HttpResponse, Responder};

pub async fn root() -> impl Responder {
    HttpResponse::Ok().body("alerion 0.1.0")
}

pub mod api;
