use std::sync::Arc;
use webserver::Webserver;
use config::ConfigFile;
use servers::ServerPool;
use futures::stream::{StreamExt, FuturesUnordered};

/// Alerion main entrypoint. Expects a tokio runtime to be setup.
pub async fn alerion_main() -> anyhow::Result<()> {
    // we need to:
    // - read config/start watch
    // - create webserver
    // - other stuff :33

    let config_file = ConfigFile::open_default().await?; 

    let server_pool = Arc::new(ServerPool::new());

    server_pool.create_server("0e4059ca-d79b-46a5-8ec4-95bd0736d150".try_into().unwrap()).await;

    // there is a low likelyhood this will actually block, and if it does
    // it will block only once for a short amount of time, so it's no big deal.
    let webserver = Webserver::make(config_file.config(), Arc::clone(&server_pool))?;

    let webserver_handle = tokio::spawn(async move {
        let _result = webserver.serve().await;
        // handle recovery
    });

    let mut handles = FuturesUnordered::new();
    handles.push(webserver_handle);

    loop {
        match handles.next().await {
            None => break,
            Some(_result) => {},
        }
    }

    Ok(())
}

pub mod config;
pub mod servers;
pub mod webserver;
pub mod websocket;

