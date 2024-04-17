use std::net::{IpAddr, Ipv4Addr};

use serde::{Deserialize, Serialize};

const WINGS_CONFIG_PATH: &str = "/etc/pterodactyl/config.yml";

mod wings {
    use serde::{Deserialize, Serialize};
    use serde_json::Value;

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Config {
        pub debug: bool,
        pub app_name: String,
        pub uuid: String,
        pub token_id: String,
        pub token: String,
        pub api: Api,
        pub system: System,
        pub docker: Docker,
        pub throttles: Throttles,
        pub remote: String,
        pub remote_query: RemoteQuery,
        pub allowed_mounts: Vec<Value>,
        pub allowed_origins: Vec<Value>,
        pub allow_cors_private_network: bool,
        pub ignore_panel_config_updates: bool,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Api {
        pub host: String,
        pub port: i64,
        pub ssl: Ssl,
        pub disable_remote_download: bool,
        pub upload_limit: i64,
        pub trusted_proxies: Vec<Value>,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Ssl {
        pub enabled: bool,
        pub cert: String,
        pub key: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct System {
        pub root_directory: String,
        pub log_directory: String,
        pub data: String,
        pub archive_directory: String,
        pub backup_directory: String,
        pub tmp_directory: String,
        pub username: String,
        pub timezone: String,
        pub user: User,
        pub disk_check_interval: i64,
        pub activity_send_interval: i64,
        pub activity_send_count: i64,
        pub check_permissions_on_boot: bool,
        pub enable_log_rotate: bool,
        pub websocket_log_count: i64,
        pub sftp: Sftp,
        pub crash_detection: CrashDetection,
        pub backups: Backups,
        pub transfers: Transfers,
        pub openat_mode: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct User {
        pub rootless: Rootless,
        pub uid: i64,
        pub gid: i64,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Rootless {
        pub enabled: bool,
        pub container_uid: i64,
        pub container_gid: i64,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Sftp {
        pub bind_address: String,
        pub bind_port: i64,
        pub read_only: bool,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct CrashDetection {
        pub enabled: bool,
        pub detect_clean_exit_as_crash: bool,
        pub timeout: i64,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Backups {
        pub write_limit: i64,
        pub compression_level: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Transfers {
        pub download_limit: i64,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Docker {
        pub network: Network,
        pub domainname: String,
        pub registries: Registries,
        pub tmpfs_size: i64,
        pub container_pid_limit: i64,
        pub installer_limits: InstallerLimits,
        pub overhead: Overhead,
        pub use_performant_inspect: bool,
        pub userns_mode: String,
        pub log_config: LogConfig,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Network {
        pub interface: String,
        pub dns: Vec<String>,
        pub name: String,
        pub ispn: bool,
        pub driver: String,
        pub network_mode: String,
        pub is_internal: bool,
        pub enable_icc: bool,
        pub network_mtu: i64,
        pub interfaces: Interfaces,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Interfaces {
        pub v4: V4,
        pub v6: V6,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct V4 {
        pub subnet: String,
        pub gateway: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct V6 {
        pub subnet: String,
        pub gateway: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Registries {}

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct InstallerLimits {
        pub memory: i64,
        pub cpu: i64,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Overhead {
        #[serde(rename = "override")]
        pub override_field: bool,
        pub default_multiplier: f64,
        pub multipliers: Multipliers,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Multipliers {}

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct LogConfig {
        #[serde(rename = "type")]
        pub type_field: String,
        pub config: LogFileConfig,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct LogFileConfig {
        pub compress: String,
        #[serde(rename = "max-file")]
        pub max_file: String,
        #[serde(rename = "max-size")]
        pub max_size: String,
        pub mode: String,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct Throttles {
        pub enabled: bool,
        pub lines: i64,
        pub line_reset_interval: i64,
    }

    #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
    pub struct RemoteQuery {
        pub timeout: i64,
        pub boot_servers_per_page: i64,
    }
}

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

impl From<wings::Config> for AlerionConfig {
    fn from(root: wings::Config) -> Self {
        let api = AlerionApi {
            host: root
                .api
                .host
                .parse()
                .unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            port: root.api.port as u16,
            ssl: AlerionApiSsl {
                enabled: root.api.ssl.enabled,
                cert: root.api.ssl.cert,
                key: root.api.ssl.key,
            },
        };

        let auth = AlerionAuthentication {
            token: root.token,
            token_id: root.token_id,
        };

        AlerionConfig {
            remote: root.remote,
            debug: root.debug,
            uuid: root.uuid,
            api,
            auth,
        }
    }
}

impl AlerionConfig {
    pub fn load(project_dirs: &directories::ProjectDirs) -> anyhow::Result<Self> {
        let config_path = project_dirs.config_dir().join("config.json");
        let config = std::fs::read_to_string(&config_path).map_err(|e| {
            anyhow::anyhow!(
                "Could not read Alerion config from {}: {}",
                config_path.display(),
                e
            )
        })?;

        let config: AlerionConfig = serde_json::from_str(&config)?;
        Ok(config)
    }

    pub fn save(&self, project_dirs: &directories::ProjectDirs) -> anyhow::Result<()> {
        let config_path = project_dirs.config_dir().join("config.json");
        let config = serde_json::to_string_pretty(self)?;

        std::fs::write(&config_path, config).map_err(|e| {
            anyhow::anyhow!(
                "Could not write Alerion config to {}: {}",
                config_path.display(),
                e
            )
        })?;

        Ok(())
    }

    pub fn import_wings(&self) -> anyhow::Result<Self> {
        if !cfg!(target_os = "linux") {
            return Err(anyhow::anyhow!("Wings is not supported on this platform"));
        }

        let config = std::fs::read_to_string(WINGS_CONFIG_PATH).map_err(|e| {
            anyhow::anyhow!(
                "Could not read Wings config from {}: {}",
                WINGS_CONFIG_PATH,
                e
            )
        })?;

        let config: wings::Config = serde_yaml::from_str(&config)?;
        Ok(config.into())
    }
}
