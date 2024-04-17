use std::collections::HashMap;
use std::net::Ipv4Addr;
use std::mem;

use reqwest::StatusCode;
use reqwest::header::{self, HeaderMap};
use uuid::Uuid;
use serde::{Serialize, Deserialize};
use serde::de::IgnoredAny;
use serde_json::Value;
use smallvec::SmallVec;
use thiserror::Error;

use crate::config::AlerionConfig;

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
    pub ip: Ipv4Addr,
    pub port: u16,
}

#[derive(Debug, Deserialize)]
pub struct AllocationConfig {
    pub force_outgoing_ip: bool,
    pub default: Allocation,
    pub mappings: HashMap<Ipv4Addr, SmallVec<[u16; 2]>>,
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
pub struct ServerMeta {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Deserialize)]
pub struct ServerSettings {
    pub uuid: Uuid,
    pub meta: IgnoredAny,
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
pub struct RemoteServersMeta {
    pub current_page: usize,
    pub from: usize,
    pub last_page: usize,
    pub links: IgnoredAny,
    pub path: IgnoredAny,
    pub per_page: usize,
    pub to: usize,
    pub total: usize,
}

#[derive(Debug, Deserialize)]
pub struct RemoteServersResponse {
    pub data: Vec<ServerData>,
    pub links: IgnoredAny,
    pub meta: RemoteServersMeta,
}

#[derive(Debug, Deserialize)]
pub struct RemoteSingleServerResponse {
    pub settings: ServerData,
    pub process_configuration: ProcessConfig,
}

#[derive(Debug, Deserialize)]
pub struct RemoteServerInstallationResponse {
    pub container_image: String,
    pub entrypoint: String,
    pub script: String,
}

#[derive(Debug, Serialize)]
struct UpdateServerInstallStatusRequest {
    pub successful: bool,
    pub reinstall: bool,
}

#[derive(Debug, Error)]
pub enum ResponseError {
    #[error("failed to process request or response: {0}")]
    Protocol(#[from] reqwest::Error),
    #[error("server with uuid {0} was not found")]
    NotFound(Uuid),
    #[error("failed to parse response: {0}")]
    InvalidJson(serde_json::Error),
    #[error("failed to authenticate")]
    Unauthorized,
    #[error("unknown error")]
    Unknown,
}

/// A wrapper around the simple pyrodactyl remote API
pub struct RemoteClient {
    remote: String,
    http: reqwest::Client,
}

impl RemoteClient {
    pub fn new(config: &AlerionConfig) -> Self {
        let token_id = &config.token_id;
        let token = &config.token;

        let mut headers = HeaderMap::new();
        headers.insert(
            header::AUTHORIZATION,
            format!("Bearer {token_id}.{token}").parse().unwrap(),
        );
        headers.insert(
            header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(
            header::ACCEPT,
            "application/json".parse().unwrap(),
        );

        Self {
            remote: config.remote.clone(),
            http: reqwest::Client::builder()
                .user_agent("alerion/0.1.0")
                .default_headers(headers)
                .build()
                .unwrap()
        }
    }

    pub async fn post_installation_status(
        &self,
        uuid: Uuid,
        successful: bool,
        reinstall: bool,
    ) -> Result<(), ResponseError> {
        let req = UpdateServerInstallStatusRequest {
            successful,
            reinstall,
        };

        let resp = self.http
            .post(format!("{}/api/remote/servers/{}/install", self.remote, uuid.as_hyphenated()))
            .body(serde_json::to_string(&req).unwrap())
            .send()
            .await?;

        if resp.status() == StatusCode::NOT_FOUND {
            Err(ResponseError::NotFound(uuid))
        } else {
            Ok(())
        }
    }

    pub async fn get_install_instructions(
        &self,
        uuid: Uuid,
    ) -> Result<RemoteServerInstallationResponse, ResponseError> {
        let resp = self.http
            .get(format!("{}/api/remote/servers/{}/install", self.remote, uuid.as_hyphenated()))
            .send()
            .await?;

        match resp.status() {
            StatusCode::NOT_FOUND => Err(ResponseError::NotFound(uuid)),
            StatusCode::UNAUTHORIZED => Err(ResponseError::Unauthorized),
            StatusCode::OK => {
                let bytes = resp.bytes().await?;

                serde_json::from_slice::<RemoteServerInstallationResponse>(&bytes)
                    .map_err(|json_e| ResponseError::InvalidJson(json_e))
            }

            _ => Err(ResponseError::Unknown),
        }
    }

    pub async fn get_server_configuration(&self, uuid: Uuid) -> Result<RemoteSingleServerResponse, ResponseError> {
        let resp = self.http
            .get(format!("{}/api/remote/servers/{}", self.remote, uuid.as_hyphenated()))
            .send()
            .await?;

        match resp.status() {
            StatusCode::NOT_FOUND => Err(ResponseError::NotFound(uuid)),
            StatusCode::UNAUTHORIZED => Err(ResponseError::Unauthorized),
            StatusCode::OK => {
                let bytes = resp.bytes().await?;

                serde_json::from_slice::<RemoteSingleServerResponse>(&bytes)
                    .map_err(|json_e| ResponseError::InvalidJson(json_e))
            }

            _ => Err(ResponseError::Unknown),
        }
    }

    pub async fn get_servers(&self) -> Result<Vec<ServerData>, ResponseError> {
        let mut servers: Option<Vec<ServerData>> = None;
        let mut page = 1;

        loop {
            let resp = self.http
                .get(format!("{}/api/remote/servers?page={}&per_page=10", self.remote, page))
                .send()
                .await?;

            let parsed = match resp.status() {
                StatusCode::UNAUTHORIZED => Err(ResponseError::Unauthorized),
                StatusCode::OK => {
                    let bytes = resp.bytes().await?;

                    serde_json::from_slice::<RemoteServersResponse>(&bytes)
                        .map_err(|json_e| ResponseError::InvalidJson(json_e))
                }

                _ => Err(ResponseError::Unknown),
            };

            let mut parsed = parsed?;
            let server_data = mem::take(&mut parsed.data);

            servers = Some(match servers {
                None => server_data,
                Some(mut s) => {
                    s.extend(server_data);
                    s
                }
            });

            if parsed.meta.current_page == parsed.meta.last_page {
                return Ok(servers.unwrap());
            }

            page += 1;
        }
    }
}
