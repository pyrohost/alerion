#![deny(clippy::unwrap_used)]

pub mod config;
pub mod filesystem;
pub mod logging;
pub mod servers;
pub mod webserver;
pub mod websocket;

use config::AlerionConfig;
use futures::stream::{FuturesUnordered, StreamExt};

use crate::filesystem::setup_directories;

/// Alerion main entrypoint. Expects a tokio runtime to be setup.
pub async fn alerion_main() -> anyhow::Result<()> {
    logging::splash();
    //logging::setup();

    tracing::info!("Starting Alerion");

    let project_dirs = setup_directories().await?;
    let config = AlerionConfig::load(&project_dirs)?;

    //let server_pool = Arc::new(ServerPool::builder(&config)?.fetch_servers().await?.build());

    //server_pool.create_server("0e4059ca-d79b-46a5-8ec4-95bd0736d150".try_into().unwrap()).await;

    // there is a low likelyhood this will actually block, and if it does
    // it will block only once for a short amount of time, so it's no big deal.
    let webserver = webserver::serve(config);

    let webserver_handle = tokio::spawn(webserver);

    let mut handles = FuturesUnordered::new();
    handles.push(webserver_handle);

    loop {
        match handles.next().await {
            None => break,
            Some(_result) => {}
        }
    }

    Ok(())
}
