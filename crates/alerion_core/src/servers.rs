use std::io;

use thiserror::Error;
use poem::IntoResponse;
use poem::http::StatusCode;

use crate::docker::DockerError;

pub type Result<T> = std::result::Result<T, ServerError>;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("docker error: {0}")]
    Docker(#[from] DockerError),
    #[error("malformed response from the Docker API")]
    MalformedResponse,
    #[error("panel remote API error: {0}")]
    RemoteApi(#[from] remote::ResponseError),
    #[error("{0}")]
    Io(#[from] io::Error),
    #[error("action cannot be performed at this moment")]
    Conflict,
}

impl IntoResponse for ServerError {
    fn into_response(self) -> poem::Response {
        match self {
            ServerError::Docker(_) | ServerError::MalformedResponse | ServerError::Io(_) => {
                StatusCode::INTERNAL_SERVER_ERROR.into()
            }

            ServerError::RemoteApi(_) => {
                StatusCode::BAD_GATEWAY.into()
            }

            ServerError::Conflict => {
                StatusCode::CONFLICT.into()
            }
        }
    }
}

pub use server::{OutboundMessage, InboundMessage, Server, State};

pub mod pool;
pub mod remote;
pub mod server;
