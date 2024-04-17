use std::sync::Arc;

use config::AlerionConfig;
use futures::stream::{FuturesUnordered, StreamExt};
use servers::ServerPool;
use webserver::Webserver;

/// Alerion main entrypoint. Expects a tokio runtime to be setup.
pub async fn alerion_main() -> anyhow::Result<()> {
    logging::splash();
    logging::setup();

    log::info!("Starting Alerion");
    let config = match AlerionConfig::load() {
        Ok(config) => config,
        Err(e) => {
            log::error!("Failed to load config: {}", e);
            return Err(e);
        }
    };

    let server_pool = Arc::new(ServerPool::builder(&config).build());

    //server_pool.create_server("0e4059ca-d79b-46a5-8ec4-95bd0736d150".try_into().unwrap()).await;

    // there is a low likelyhood this will actually block, and if it does
    // it will block only once for a short amount of time, so it's no big deal.
    let webserver = Webserver::make(config, Arc::clone(&server_pool))?;

    let webserver_handle = tokio::spawn(async move {
        let _result = webserver.serve().await;
        // handle recovery
    });

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

pub mod config;
pub mod logging;
pub mod servers;
pub mod webserver;
pub mod websocket;
