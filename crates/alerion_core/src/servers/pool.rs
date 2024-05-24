use std::{collections::HashMap, sync::Arc};

use bollard::Docker;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::configuration::AlerionConfig;
use crate::servers::server::Server;
use crate::servers::{docker, remote, ServerError};

pub struct ServerPool {
    servers: RwLock<HashMap<Uuid, Arc<Server>>>,
    remote_api: remote::Api,
    docker: Arc<Docker>,
}

impl ServerPool {
    pub async fn new(config: &AlerionConfig) -> Result<Self, ServerError> {
        let remote_api = remote::Api::new(config)?;

        tracing::info!("initiating connection to Docker Engine");
        let docker = Docker::connect_with_defaults().map_err(docker::DockerError::Api)?;

        Ok(Self {
            servers: RwLock::new(HashMap::new()),
            remote_api,
            docker: Arc::new(docker),
        })
    }

    #[tracing::instrument(name = "fetch_existing_servers", skip(self))]
    pub async fn fetch_existing(&self) -> Result<(), ServerError> {
        tracing::info!("fetching existing servers on this node");

        let servers = self.remote_api.get_servers().await?;

        for s in servers {
            tracing::info!("recovering server {}", s.uuid);

            let _uuid = s.uuid;

            tracing::error!("!!!!!!!!!TODO: RECOVERING SERVERS");
        }

        Ok(())
    }

    #[tracing::instrument(name = "create_server", skip(self))]
    pub async fn create(&self, uuid: Uuid, start_on_completion: bool) -> Result<Arc<Server>, ServerError> {
        let docker = Arc::clone(&self.docker);

        let server = Server::new(uuid, self.remote_api.clone(), docker);
        let server = Arc::new(server);

        tracing::error!("!!!!!!!!!TODO: ACTUALLY ADDING SERVERS AND STUFF");

        Ok(server)
    }

    pub async fn get(&self, uuid: Uuid) -> Option<Arc<Server>> {
        self.servers.read().await.get(&uuid).cloned()
    }
}

