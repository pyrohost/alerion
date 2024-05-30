use std::collections::HashMap;
use std::io;
use std::sync::Arc;
use std::path::PathBuf;

use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, DbError>;

#[derive(Debug, Error)]
pub enum DbError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum InstallState {
    Ongoing,
    Success,
    Failure,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServerData {
    pub installation: InstallState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Model {
    pub servers: HashMap<Uuid, ServerData>,
}

/// Handle to the internal JSON database.  
///
/// Handles filesystem operations and JSON serialization.
/// It uses a global lock to avoid reading partially
/// written data.
///
/// The handle uses `Arc`s internally and can be cheaply
/// cloned; don't hold the handle in an `Arc`.
#[derive(Debug, Clone)]
pub struct Handle {
    // contains a size estimate of the backing file
    lock: Arc<Mutex<usize>>,
    // an aid for detecting deadlocks
    path: Arc<PathBuf>,
}

impl Handle {
    pub fn new(path: PathBuf) -> Self {
        Self {
            lock: Arc::new(Mutex::new(32)),
            path: Arc::new(path),
        }
    }

    pub async fn reader(&self) -> Result<Model> {
        let mut guard = self.lock.lock().await;
        let mut contents = String::with_capacity(*guard);

        let mut fd = fs::File::open(self.path.as_path()).await?;
        let size = fd.read_to_string(&mut contents).await?;
        *guard = size;
        drop(guard);

        let model = serde_json::from_str(&contents)?;
        Ok(model)
    }

    pub async fn write<F>(&self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Model)
    {
        let mut guard = self.lock.lock().await;

        let mut contents = String::with_capacity(*guard);

        let mut fd = fs::File::options()
            .write(true)
            .read(true)
            .truncate(true)
            .open(self.path.as_path())
            .await?;

        let size = fd.read_to_string(&mut contents).await?;
        *guard = size;

        let mut model = serde_json::from_str(&contents)?;
        f(&mut model);

        // probably faster to reuse the buffer here than directly
        // writing to the file/through a `BufWriter`.
        contents.clear();
        let mut contents = contents.into_bytes();
        serde_json::to_writer(&mut contents, &model)?;
        // truncate is true!
        fd.write_all(&contents).await?;
        
        Ok(())
    }
}
