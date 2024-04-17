use std::net::IpAddr;

use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SslConfig {
    pub enabled: bool,
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiConfig {
    pub host: IpAddr,
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

#[derive(Debug, Deserialize)]
pub struct ConfigUpdateRequest {
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
    pub applied: bool,
}

