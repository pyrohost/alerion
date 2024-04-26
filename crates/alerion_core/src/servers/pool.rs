use std::{collections::HashMap, sync::Arc};

use bollard::Docker;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::AlerionConfig;
use crate::servers::server::{ServerInfo, Server};
use crate::servers::remote;
use super::ServerError;

pub struct ServerPool {
    servers: RwLock<HashMap<Uuid, Arc<Server>>>,
    remote_api: Arc<remote::RemoteClient>,
    docker: Arc<Docker>,
}

impl ServerPool {
    #[tracing::instrument(skip(config))]
    pub async fn new(config: &AlerionConfig) -> Result<Self, ServerError> {
        tracing::info!("Initializing managed servers...");

        let remote_api = remote::RemoteClient::new(config)?;

        tracing::info!("Initiating connection to Docker Engine");
        let docker = Docker::connect_with_defaults()?;

        Ok(Self {
            servers: RwLock::new(HashMap::new()),
            remote_api: Arc::new(remote_api),
            docker: Arc::new(docker),
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn fetch_existing_servers(&self) -> Result<(), ServerError> {
        tracing::info!("Fetching existing servers on this node");

        let servers = self.remote_api.get_servers().await?;

        for s in servers {
            tracing::info!("Adding server {}...", s.uuid);

            let uuid = s.uuid;
            let info = ServerInfo::from_remote_info(s.settings);
            let server = Server::new(
                uuid,
                info,
                Arc::clone(&self.remote_api),
                Arc::clone(&self.docker),
            )
            .await?;

            self.servers.write().await.insert(uuid, server);
        }

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    pub async fn register_server(&self, uuid: Uuid, start: bool) -> Result<Arc<Server>, ServerError> {
        tracing::info!("Adding server {uuid}...");

        let remote_api = Arc::clone(&self.remote_api);
        let docker = Arc::clone(&self.docker);

        tracing::debug!("Fetching server configuration from remote");
        let config = remote_api.get_server_configuration(uuid).await?;
        let server_info = ServerInfo::from_remote_info(config.settings);

        let server = Server::new(uuid, server_info, remote_api, docker).await?;
        self.servers.write().await.insert(uuid, Arc::clone(&server));

        Ok(server)
    }

    pub async fn get_server(&self, uuid: Uuid) -> Option<Arc<Server>> {
        self.servers.read().await.get(&uuid).cloned()
    }
}

