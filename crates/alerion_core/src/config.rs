use std::net::IpAddr;

use anyhow::anyhow;
use serde::{Deserialize, Serialize};

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

impl AlerionConfig {
    pub fn load(project_dirs: &directories::ProjectDirs) -> anyhow::Result<Self> {
        tracing::info!(
            "Loading Alerion config from {}",
            project_dirs.config_dir().display()
        );
        let config_path = project_dirs.config_dir().join("config.json");
        let config = std::fs::read_to_string(&config_path).map_err(|e| {
            anyhow!(
                "Could not read Alerion config from {}: {}",
                config_path.display(),
                e
            )
        })?;

        let config: AlerionConfig = serde_json::from_str(&config)?;
        tracing::debug!("Loaded Alerion config: {:?}", config);
        Ok(config)
    }

    pub fn save(&self, project_dirs: &directories::ProjectDirs) -> anyhow::Result<()> {
        let config_path = project_dirs.config_dir().join("config.json");
        let config = serde_json::to_string_pretty(self)?;

        std::fs::write(&config_path, config).map_err(|e| {
            anyhow!(
                "Could not write Alerion config to {}: {}",
                config_path.display(),
                e
            )
        })?;

        tracing::info!("Saved Alerion config to {}", config_path.display());

        Ok(())
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
