use std::net::IpAddr;
use std::io;
use std::env;
use std::fs;
use std::path::PathBuf;

use serde::{Serialize, Deserialize};
use thiserror::Error;

use crate::os::{ConfigPath, ConfigPathImpl};

#[cfg(feature = "wings_compat")]
use super::wings_compat;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("failed to parse configuration file: {0}")]
    Json(#[from] serde_json::Error),
    #[error("could not read environment variable '{var}': {err}")]
    Env {
        err: env::VarError,
        var: &'static str,
    },
}

impl From<(env::VarError, &'static str)> for ConfigError {
    fn from((err, var): (env::VarError, &'static str)) -> Self {
        Self::Env { err, var }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ApiSsl {
    pub enabled: bool,
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Api {
    pub host: IpAddr,
    pub port: u16,
    pub ssl: ApiSsl,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Authentication {
    pub token: String,
    pub token_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub debug: bool,
    pub data_dir: PathBuf,
    pub uuid: String,
    pub api: Api,
    pub auth: Authentication,
    pub remote: String,
}

impl Config {
    pub fn load() -> Result<Self, ConfigError> {
        let folder = ConfigPath::parent()?;

        fs::create_dir_all(&folder)?;

        let config_path = folder.join(ConfigPath::node());

        tracing::info!("loading configuration at '{}'", config_path.display());
        let read = fs::read_to_string(config_path)?;

        let config: Config = serde_json::from_str(&read)?;
        Ok(config)
    }

    #[cfg(feature = "wings_compat")]
    pub fn import_wings(&self) -> Self {
        todo!()
    }
}
