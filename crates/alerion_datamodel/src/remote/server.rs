use std::collections::HashMap;

use serde::{Serialize, Deserialize, de::IgnoredAny};
use serde_json::Value;
use smallvec::SmallVec;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SearchReplaceMatcher {
    #[serde(rename = "match")]
    pub match_item: String,
    pub replace_with: String,
}

#[derive(Debug, Deserialize)]
pub struct FileParser {
    pub parser: String,
    pub file: String,
    pub replace: Vec<SearchReplaceMatcher>,
}

#[derive(Debug, Deserialize)]
pub enum StopSignalType {
    #[serde(rename = "command")]
    Command,
}

#[derive(Debug, Deserialize)]
pub struct StopConfig {
    #[serde(rename = "type")]
    pub kind: StopSignalType,
    pub value: String,
}

#[derive(Debug, Deserialize)]
pub struct StartupConfig {
    pub done: Vec<String>,
    pub user_interaction: Vec<Value>,
    pub strip_ansi: bool,
}

#[derive(Debug, Deserialize)]
pub struct ProcessConfig {
    pub startup: StartupConfig,
    pub stop: StopConfig,
    pub configs: SmallVec<[FileParser; 1]>,
}

#[derive(Debug, Deserialize)]
pub struct Egg {
    pub id: Uuid,
    // todo: figure out what is inside this array
    pub file_denylist: Vec<Value>,
}

#[derive(Debug, Deserialize)]
pub struct Mount {
    pub source: String,
    pub target: String,
    pub read_only: bool,
}

#[derive(Debug, Deserialize)]
pub struct Allocation {
    pub ip: String,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct AllocationConfig {
    pub force_outgoing_ip: bool,
    pub default: Allocation,
    pub mappings: HashMap<String, SmallVec<[u16; 2]>>,
}

#[derive(Debug, Deserialize)]
pub struct ContainerConfig {
    pub image: String,
    pub oom_disabled: bool,
    pub requires_rebuild: bool,
}

#[derive(Debug, Deserialize)]
pub struct BuildConfig {
    pub memory_limit: isize,
    pub swap: isize,
    pub io_weight: u32,
    pub cpu_limit: u32,
    pub threads: Option<String>,
    pub disk_space: usize,
    pub oom_disabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct ServerMetadata {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct ServerSettings {
    pub uuid: Uuid,
    pub meta: ServerMetadata,
    pub suspended: bool,
    pub environment: HashMap<String, Value>,
    pub invocation: String,
    pub skip_egg_scripts: bool,
    pub build: BuildConfig,
    pub container: ContainerConfig,
    pub allocations: AllocationConfig,
    pub mounts: Vec<Mount>,
    pub egg: Egg,
}

#[derive(Debug, Deserialize)]
pub struct ServerData {
    pub uuid: Uuid,
    pub settings: ServerSettings,
    pub process_configuration: ProcessConfig,
}

#[derive(Debug, Deserialize)]
pub struct GetServersResponseMetadata {
    pub current_page: usize,
    pub from: usize,
    pub last_page: usize,
    pub links: IgnoredAny,
    pub path: IgnoredAny,
    pub per_page: usize,
    pub to: usize,
    pub total: usize,
}

/// Response to `GET /api/remote/servers`.
#[derive(Debug, Deserialize)]
pub struct GetServersResponse {
    pub data: Vec<ServerData>,
    pub links: IgnoredAny,
    pub meta: GetServersResponseMetadata,
}

/// Response to `GET /api/remote/servers/{uuid}`.
#[derive(Debug, Deserialize)]
pub struct GetServerByUuidResponse {
    pub settings: ServerSettings,
    pub process_configuration: ProcessConfig,
}

/// Response to `GET /api/remote/servers/{uuid}/install`
#[derive(Debug, Deserialize)]
pub struct GetServerInstallByUuidResponse {
    pub container_image: String,
    pub entrypoint: String,
    pub script: String,
}

/// Request to `POST /api/remote/servers/{uuid}/install`
#[derive(Debug, Serialize)]
pub struct PostServerInstallByUuidRequest {
    pub successful: bool,
    pub reinstall: bool,
}
