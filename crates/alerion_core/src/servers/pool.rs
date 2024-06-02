use std::collections::HashMap;
use std::sync::Arc;

use bollard::Docker;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::fs::{LocalDataPaths, Config};
use crate::servers::{remote, ServerError, Server};
use crate::docker;

pub struct ServerPool {
    servers: RwLock<HashMap<Uuid, Arc<Server>>>,
    remote_api: remote::Api,
    docker: Arc<Docker>,
    localdata: LocalDataPaths,
}

impl ServerPool {
    pub async fn new(config: &Config, localdata: LocalDataPaths) -> Result<Arc<Self>, ServerError> {
        let remote_api = remote::Api::new(config)?;

        tracing::info!("initiating connection to Docker Engine");
        let docker = Docker::connect_with_defaults().map_err(docker::DockerError::Api)?;

        Ok(Arc::new(Self {
            servers: RwLock::new(HashMap::new()),
            remote_api,
            docker: Arc::new(docker),
            localdata,
        }))
    }

    /// Retrieves all preexisting servers from the panel and starts their installation process in
    /// a separate task, if needed.
    #[tracing::instrument(skip_all)]
    pub async fn try_fetch_existing(&self) {
        match self.remote_api.get_servers().await {
            Ok(servers) => {
                for data in servers {
                    if let Err(e) = self.create(data.uuid, true).await {
                        tracing::error!("creating server '{}' from the remote list of preexisting servers failed: {e}", data.uuid.as_hyphenated());
                    }
                }
            },
            Err(e) => {
                tracing::error!("couldn't fetch existing servers from remote api: {e}");
            }
        }
    }

    /// Adds a server to the pool and begins its installation process in a separate task,
    /// if needed.
    #[tracing::instrument(name = "create_server", skip(self))]
    pub async fn create(
        &self,
        uuid: Uuid,
        start_on_completion: bool,
    ) -> Result<Arc<Server>, ServerError> {
        let read = self.servers.read().await;

        if read.get(&uuid).is_none() {
            drop(read);

            let docker = Arc::clone(&self.docker);
            // this will start the installation if needed
            let server = Server::new(uuid, self.remote_api.clone(), docker, &self.localdata, start_on_completion).await?;

            let mut writer = self.servers.write().await;
            writer.insert(uuid, Arc::clone(&server));

            drop(writer);

            Ok(server)
        } else {
            tracing::error!("server {uuid} already exists");
            Err(ServerError::Conflict)
        }
    }

    pub async fn get(&self, uuid: Uuid) -> Option<Arc<Server>> {
        self.servers.read().await.get(&uuid).cloned()
    }
}
