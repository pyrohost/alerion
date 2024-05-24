use std::collections::HashMap;
use std::io;

use thiserror::Error;

const ALERION_VERSION_LABEL: &str = "host.pyro.alerion-version";

fn alerion_version_labels() -> HashMap<&'static str, &'static str> {
    HashMap::from([(ALERION_VERSION_LABEL, env!("CARGO_PKG_VERSION"))])
}

#[derive(Debug, Error)]
pub enum DockerError {
    #[error("Docker Engine API error: {0:?}")]
    Api(#[from] bollard::errors::Error),
    #[error("Docker Engine sent back an improper response, cannot continue")]
    BadResponse,
    #[error("{0}")]
    Io(#[from] io::Error)
}

pub type Result<T> = std::result::Result<T, DockerError>;

fn is_404(err: &bollard::errors::Error) -> bool {
    use bollard::errors::Error::DockerResponseServerError;
    matches!(err, DockerResponseServerError { status_code: 404, .. })
}

pub mod container;
pub mod install;
pub mod volume;
pub mod bind_mount;
