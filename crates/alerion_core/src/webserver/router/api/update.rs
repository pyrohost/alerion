use std::net::Ipv4Addr;

use actix_web::{web, Responder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SslConfig {
    pub enabled: bool,
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiConfig {
    // should probably just be 0.0.0.0
    pub host: Ipv4Addr,
    pub port: u16,
    pub ssl: SslConfig,
    pub upload_limit: u32,
}

#[derive(Debug, Deserialize)]
pub struct SftpConfig {
    pub bind_port: u16,
}

#[derive(Debug, Deserialize)]
pub struct SystemConfig {
    pub data: String,
    pub sftp: SftpConfig,
}

// https://github.com/pyrohost/panel/blob/278fa6681c653dd3fcc82f086000c771e73e2030/app/Models/Node.php#L137
#[derive(Debug, Deserialize)]
pub struct ConfigUpdate {
    pub debug: bool,
    pub uuid: Uuid,
    // todo: string w/ length?
    pub token_id: String,
    // todo: string w/ length?
    pub token: String,
    pub api: ApiConfig,
    pub system: SystemConfig,
    pub allowed_mounts: Vec<String>,
    // todo: uri?
    pub remote: String,
}

#[derive(Debug, Serialize)]
pub struct ConfigUpdateResponse {
    applied: bool,
}

pub async fn update_post(_payload: web::Json<ConfigUpdate>) -> impl Responder {
    web::Json(ConfigUpdateResponse { applied: false })
}
