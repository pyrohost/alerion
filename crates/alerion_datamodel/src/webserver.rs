use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
pub struct SystemOptions {
    pub architecture: &'static str,
    pub cpu_count: u32,
    pub kernel_version: &'static str,
    pub os: &'static str,
    pub version: &'static str,
}

#[derive(Serialize, Deserialize)]
pub struct CreateServerRequest {
    pub uuid: Uuid,
    pub start_on_completion: bool,
}

pub mod update;
