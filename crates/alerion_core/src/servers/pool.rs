use std::collections::HashMap;
use std::sync::Arc;

use bollard::Docker;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::fs::{LocalData, Config};
use crate::servers::{remote, ServerError, Server};
use crate::docker;

pub struct ServerPool {
    servers: RwLock<HashMap<Uuid, Arc<Server>>>,
    remote_api: remote::Api,
    docker: Arc<Docker>,
    localdata: LocalData,
}

impl ServerPool {
    pub async fn new(config: &Config, localdata: LocalData) -> Result<Arc<Self>, ServerError> {
        let remote_api = remote::Api::new(config)?;

        tracing::info!("initiating connection to Docker Engine");
        let docker = Docker::connect_with_defaults().map_err(docker::DockerError::Api)?;

        let pool = Arc::new(Self {
            servers: RwLock::new(HashMap::new()),
            remote_api,
            docker: Arc::new(docker),
            localdata,
        });


        for data in pool.remote_api.get_servers().await? {
            let pool_cpy = Arc::clone(&pool);
            tokio::spawn(async move {
                pool_cpy.create(data.uuid, true).await
            });
        }

        Ok(pool)
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
    pub async fn create(
        &self,
        uuid: Uuid,
        start_on_completion: bool,
    ) -> Result<Arc<Server>, ServerError> {
        let read = self.servers.read().await;

        if read.get(&uuid).is_none() {
            drop(read);

            let docker = Arc::clone(&self.docker);
            let server = Server::new(uuid, self.remote_api.clone(), docker, self.localdata.clone());

            let mut writer = self.servers.write().await;
            writer.insert(uuid, Arc::clone(&server));

            drop(writer);

            Server::start_installation(Arc::clone(&server))?;

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
