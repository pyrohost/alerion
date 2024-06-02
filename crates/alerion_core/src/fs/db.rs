use std::collections::hash_map::{HashMap, Entry};
use std::io;
use std::sync::Arc;
use std::path::Path;

use tokio::fs;
use tokio::io::{SeekFrom, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::{Mutex, mpsc};
use parking_lot::{RwLock, RwLockReadGuard};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use thiserror::Error;

use crate::servers::server;

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(Debug, Error)]
pub enum DbError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerData {
    pub state: server::State,
}

impl Default for ServerData {
    fn default() -> Self {
        Self {
            state: server::State::Bare,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Model {
    pub servers: HashMap<Uuid, ServerData>,
}

#[derive(Debug)]
pub struct Root {
    servers: Mutex<HashMap<Uuid, Handle>>,
    sender: mpsc::Sender<(Uuid, ServerData)>,
}

impl Root {
    pub fn init(path: &Path) -> Result<Self> {
        let model = std::fs::read(path)
            .map(|contents| serde_json::from_slice::<Model>(&contents))
            .unwrap_or_else(|_| Ok(Model::default()))?;

        let fd = std::fs::File::options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        let (sender, recv) = mpsc::channel(16);

        let servers = model.servers
            .iter()
            .map(|(&uuid, data)| {
                let handle = Handle::new(uuid, data.clone(), sender.clone());
                (uuid, handle)
            })
            .collect();

        tokio::spawn(Root::io_task(model, recv, fs::File::from_std(fd)));

        Ok(Root { servers: Mutex::new(servers), sender })
    }

    pub async fn server(&self, uuid: Uuid) -> Handle {
        let mut guard  = self.servers.lock().await;
        match guard.entry(uuid) {
            Entry::Occupied(occu) => occu.get().clone(),
            Entry::Vacant(vacant) => {
                let sender = self.sender.clone();
                let data = ServerData::default();
                let _ = sender.send((uuid, data.clone())).await;

                let handle = Handle::new(uuid, data, sender);
                vacant.insert(handle.clone());
                handle
            },
        }
    }

    async fn io_task(
        mut model: Model,
        mut recv: mpsc::Receiver<(Uuid, ServerData)>, 
        mut fd: fs::File,
    ) {
        let mut buffer = Vec::with_capacity(32);
        while let Some((uuid, new_data)) = recv.recv().await {
            tracing::debug!("updating DB record for '{uuid}'");
            model.servers.insert(uuid, new_data);
            
            if let Err(e) = serde_json::to_writer(&mut buffer, &model) {
                tracing::error!("failed to serialize DB data into json: {e}");
                continue;
            }

            // errors are logged in the function body
            let _ = rewrite_flushed(&mut fd, &buffer).await;
            buffer.clear();
        }

        tracing::info!("DB I/O channel closed");
    }
}

#[derive(Debug, Clone)]
pub struct Handle {
    uuid: Uuid,
    data: Arc<RwLock<ServerData>>,
    chan: mpsc::Sender<(Uuid, ServerData)>,
}

impl Handle {
    pub fn new(uuid: Uuid, data: ServerData, chan: mpsc::Sender<(Uuid, ServerData)>) -> Self {
        Handle {
            uuid,
            data: Arc::new(RwLock::new(data)),
            chan,
        }
    }

    pub fn get(&self) -> RwLockReadGuard<ServerData> {
        self.data.read()
    }

    /// Update the internal and filesystem data. Very unlikely that
    /// this will poll `Pending`.
    pub async fn update<F>(&self, f: F)
    where
        F: Fn(&mut ServerData),
    {
        // the lock will not be busy at all so no need for
        // optimisation here.
        let cpy = {
            let mut guard = self.data.write();
            f(&mut guard);
            guard.clone()
        };

        let _ = self.chan.send((self.uuid, cpy)).await;
    }
}

async fn rewrite_flushed(fd: &mut fs::File, bytes: &[u8]) -> io::Result<()> {
    if let Err(e) = fd.set_len(0).await {
        tracing::error!("failed to truncate database file: {e}");
        return Err(e);
    }

    if let Err(e) = fd.seek(SeekFrom::Start(0)).await {
        tracing::error!("failed to update file buffer cursor: {e}");
    }
    
    if let Err(e) = fd.write_all(bytes).await {
        tracing::error!("failed to write to local database: {e}");
        return Err(e);
    }

    if let Err(e) = fd.flush().await {
        tracing::error!("failed to flush data into local database: {e}");
        return Err(e);
    }

    Ok(())
}
