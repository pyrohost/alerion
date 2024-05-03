use std::{collections::HashMap, sync::Arc};

use bollard::Docker;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::configuration::AlerionConfig;
use crate::servers::server::{ServerInfo, Server};
use crate::servers::{docker, remote, ServerError};

pub struct ServerPool {
    servers: RwLock<HashMap<Uuid, Arc<Server>>>,
    remote_api: Arc<remote::RemoteClient>,
    docker: Arc<Docker>,
}

impl ServerPool {
    #[tracing::instrument(skip(config))]
    pub async fn new(config: &AlerionConfig) -> Result<Self, ServerError> {
        let remote_api = remote::RemoteClient::new(config)?;

        tracing::info!("initiating connection to Docker Engine");
        let docker = Docker::connect_with_defaults().map_err(docker::DockerError::Api)?;

        Ok(Self {
            servers: RwLock::new(HashMap::new()),
            remote_api: Arc::new(remote_api),
            docker: Arc::new(docker),
        })
    }

    #[tracing::instrument(skip(self))]
    pub async fn fetch_existing(&self) -> Result<(), ServerError> {
        tracing::info!("fetching existing servers on this node");

        let servers = self.remote_api.get_servers().await?;

        for s in servers {
            tracing::info!("recovering server {}", s.uuid);

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

    #[tracing::instrument(name = "create_server", skip(self))]
    pub async fn create(&self, uuid: Uuid, start_on_completion: bool) -> Result<Arc<Server>, ServerError> {
        let remote_api = Arc::clone(&self.remote_api);
        let docker = Arc::clone(&self.docker);

        tracing::debug!("fetching server configuration from remote");
        let config_fut = remote_api.get_server_configuration(uuid);
        let config = crate::ensure!(config_fut.await, "failed to retrieve server configuration from remote source");

        let server_info = ServerInfo::from_remote_info(config.settings);

        let server = Server::new(uuid, server_info, remote_api, docker).await?;
        self.servers.write().await.insert(uuid, Arc::clone(&server));

        Ok(server)
    }

    pub async fn get(&self, uuid: Uuid) -> Option<Arc<Server>> {
        self.servers.read().await.get(&uuid).cloned()
    }
}

