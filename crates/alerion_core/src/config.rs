use std::net::Ipv4Addr;
use std::path::{Path, PathBuf};
use std::io;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use tokio::fs;
use thiserror::Error;

#[cfg(target_os = "linux")]
const DEFAULT_LOCATION: &str = "/etc/alerion/";
#[cfg(target_os = "windows")]
const DEFAULT_LOCATION: &str = "%ProgramData%\\Alerion\\";

const CONFIG_FILE_NAME: &str = "config.yml";

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("couldn't read configuration file at '{path}' ({io_e})")]
    Io {
        path: PathBuf,
        io_e: Box<io::Error>,
    },
    #[error("couldn't parse configuration file: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiConfig {
    pub host: Ipv4Addr,
    pub port: u16,
    pub upload_limit: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlerionConfig {
    pub debug: bool,
    pub uuid: Uuid,
    pub token_id: String,
    pub token: String,
    pub api: ApiConfig,
    pub remote: String,
}

pub struct ConfigFile {
    path: PathBuf,
    last_fetched_data: AlerionConfig,
}

impl ConfigFile {
    pub async fn open(config_file: PathBuf) -> Result<Self> {
        let contents = fs::read_to_string(&config_file)
            .await
            .map_err(|io_e| ConfigError::Io {
                path: config_file.clone(),
                io_e: Box::new(io_e),
            })?;
        let config = serde_yaml::from_str(&contents)?;

        Ok(ConfigFile {
            path: config_file,
            last_fetched_data: config,
        })
    }

    pub async fn open_default() -> Result<Self> {
        let path = Path::new(DEFAULT_LOCATION).join(CONFIG_FILE_NAME);
        Self::open(path).await
    }

    pub fn config(&self) -> AlerionConfig {
        self.last_fetched_data.clone()
    }
}

