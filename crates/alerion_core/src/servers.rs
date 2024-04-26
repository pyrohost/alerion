use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("docker error: {0}")]
    Docker(#[from] bollard::errors::Error),
    #[error("panel remote API error: {0}")]
    RemoteApi(#[from] remote::ResponseError),
}

pub mod remote;
pub mod pool;
pub mod server;
