use std::collections::HashMap;
use std::io;
use std::hash::Hash;

use thiserror::Error;

const ALERION_VERSION_LABEL: &str = "host.pyro.alerion-version";

fn alerion_version_labels<T>() -> HashMap<T, T>
where
    T: From<&'static str> + PartialEq + Eq + Hash,
{
    HashMap::from([
        (ALERION_VERSION_LABEL.into(), env!("CARGO_PKG_VERSION").into())
    ])
}

#[derive(Debug, Error)]
pub enum DockerError {
    #[error("Docker Engine API error: {0:?}")]
    Api(#[from] bollard::errors::Error),
    #[error("Docker Engine sent back an improper response, cannot continue")]
    BadResponse,
    #[error("{0}")]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, DockerError>;

fn is_404(err: &bollard::errors::Error) -> bool {
    use bollard::errors::Error::DockerResponseServerError;
    matches!(
        err,
        DockerResponseServerError {
            status_code: 404,
            ..
        }
    )
}

pub use models::{
    container::{ContainerName, Container},
    bind_mount::{BindMount, BindMountName},
};

pub mod models;
pub mod install;
pub mod run;
mod util;
