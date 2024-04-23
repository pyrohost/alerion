use std::mem;

use alerion_datamodel::remote::server::{
    GetServerByUuidResponse, GetServerInstallByUuidResponse, GetServersResponse, PostServerInstallByUuidRequest, ServerData
};
use reqwest::header::{self, HeaderMap};
use reqwest::StatusCode;
use thiserror::Error;
use uuid::Uuid;

use crate::config::AlerionConfig;

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
    #[error("unknown error (status: {0})")]
    Unknown(StatusCode),
}

/// A wrapper around the simple pyrodactyl remote API
pub struct RemoteClient {
    remote: String,
    http: reqwest::Client,
}

impl RemoteClient {
    pub fn new(config: &AlerionConfig) -> Result<Self, ResponseError> {
        let token_id = &config.auth.token_id;
        let token = &config.auth.token;

        let mut headers = HeaderMap::new();

        let authorization = format!("Bearer {token_id}.{token}")
            .parse()
            .expect("valid header value");

        headers.insert(header::AUTHORIZATION, authorization);

        let accept = "application/vnd.pterodactyl.v1+json"
            .parse()
            .expect("valid header value");

        headers.insert(header::ACCEPT, accept);

        Ok(Self {
            remote: config.remote.clone(),
            http: reqwest::Client::builder()
                .user_agent("alerion/0.1.0")
                .default_headers(headers)
                .build()?,
        })
    }

    pub async fn post_installation_status(
        &self,
        uuid: Uuid,
        successful: bool,
        reinstall: bool,
    ) -> Result<(), ResponseError> {
        let req = PostServerInstallByUuidRequest {
            successful,
            reinstall,
        };

        let url = format!(
            "{}/api/remote/servers/{}/install",
            self.remote,
            uuid.as_hyphenated()
        );

        tracing::debug!("remote: POST {url}");

        let resp = self
            .http
            .post(url)
            .body(serde_json::to_string(&req).expect("JSON serialization should not fail"))
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
    ) -> Result<GetServerInstallByUuidResponse, ResponseError> {
        let url = format!(
            "{}/api/remote/servers/{}/install",
            self.remote,
            uuid.as_hyphenated()
        );

        tracing::debug!("remote: GET {url}");

        let resp = self.http.get(url).send().await?;

        match resp.status() {
            StatusCode::NOT_FOUND => Err(ResponseError::NotFound(uuid)),
            StatusCode::UNAUTHORIZED => Err(ResponseError::Unauthorized),
            StatusCode::OK => {
                let bytes = resp.bytes().await?;

                serde_json::from_slice::<GetServerInstallByUuidResponse>(&bytes)
                    .map_err(ResponseError::InvalidJson)
            }

            _ => Err(ResponseError::Unknown(resp.status())),
        }
    }

    pub async fn get_server_configuration(
        &self,
        uuid: Uuid,
    ) -> Result<GetServerByUuidResponse, ResponseError> {
        let url = format!(
            "{}/api/remote/servers/{}",
            self.remote,
            uuid.as_hyphenated()
        );

        tracing::debug!("remote: GET {url}");

        let resp = self.http.get(url).send().await?;

        match resp.status() {
            StatusCode::NOT_FOUND => Err(ResponseError::NotFound(uuid)),
            StatusCode::UNAUTHORIZED => Err(ResponseError::Unauthorized),
            StatusCode::OK => {
                let bytes = resp.bytes().await?;

                serde_json::from_slice::<GetServerByUuidResponse>(&bytes)
                    .map_err(ResponseError::InvalidJson)
            }

            _ => Err(ResponseError::Unknown(resp.status())),
        }
    }

    pub async fn get_servers(&self) -> Result<Vec<ServerData>, ResponseError> {
        let mut servers: Option<Vec<ServerData>> = None;
        let mut page = 1;

        loop {
            let url = format!(
                "{}/api/remote/servers?page={}&per_page=2",
                self.remote, page
            );

            tracing::debug!("remote: GET {url}");

            let resp = self.http.get(url).send().await?;

            let parsed = match resp.status() {
                StatusCode::UNAUTHORIZED => Err(ResponseError::Unauthorized),
                StatusCode::OK => {
                    let bytes = resp.bytes().await?;

                    serde_json::from_slice::<GetServersResponse>(&bytes)
                        .map_err(ResponseError::InvalidJson)
                }

                _ => {
                    let status = resp.status();
                    //log::debug!("{}", resp.text().await.unwrap());
                    Err(ResponseError::Unknown(status))
                }
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
                return Ok(unsafe { servers.unwrap_unchecked() });
            }

            page += 1;
        }
    }
}
