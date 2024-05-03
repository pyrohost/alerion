use std::{net::IpAddr, path::Path};
use std::io;

use serde::{Deserialize, Serialize};
use tokio::fs;
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AlerionApiSsl {
    pub enabled: bool,
    pub cert: String,
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AlerionApi {
    pub host: IpAddr,
    pub port: u16,
    pub ssl: AlerionApiSsl,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AlerionAuthentication {
    pub token: String,
    pub token_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AlerionConfig {
    pub debug: bool,
    pub uuid: String,
    pub api: AlerionApi,
    pub auth: AlerionAuthentication,
    pub remote: String,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[cfg(windows)]
const CONFIG_PATH: &str = "%PROGRAMFILES%/Alerion/";
#[cfg(unix)]
const CONFIG_PATH: &str = "/etc/alerion/";

impl AlerionConfig {
    pub async fn load() -> Result<Self, ConfigError> {
        let path = Path::new(CONFIG_PATH).join("config.json");

        tracing::info!("creating configuration directory");

        if let Err(e) = fs::create_dir(&CONFIG_PATH).await {
            tracing::error!("failed to create configuration directory {CONFIG_PATH}: {e}");
            return Err(e.into());
        }

        tracing::info!("loading configuration from {}", path.display());

        let config = match fs::read_to_string(CONFIG_PATH).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("failed to load configuration file: {e}");
                return Err(e.into());
            }
        };

        let config: AlerionConfig = serde_json::from_str(&config)?;
        Ok(config)
    }

    #[cfg(feature = "wings_compat")]
    pub fn import_wings(&self) -> anyhow::Result<Self> {
        if !cfg!(target_os = "linux") {
            return Err(anyhow!("Wings is not supported on this platform"));
        }

        let config = std::fs::read_to_string(wings_compat::WINGS_CONFIG_PATH).map_err(|e| {
            anyhow!(
                "Could not read Wings config from {}: {}",
                wings_compat::WINGS_CONFIG_PATH,
                e
            )
        })?;

        let config: wings_compat::Config = serde_yaml::from_str(&config)?;

        tracing::debug!("Imported Wings config: {:?}", config);

        Ok(config.into())
    }
}

#[cfg(feature = "wings_compat")]
mod wings_compat;
