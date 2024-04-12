use std::sync::Arc;
use alerion_webserver::Webserver;
use alerion_config::ConfigFile;
use alerion_servers::InstallPool;
use futures::stream::{StreamExt, FuturesUnordered};

/// Alerion main entrypoint. Expects a tokio runtime to be setup.
pub async fn alerion_main() -> anyhow::Result<()> {
    // we need to:
    // - read config/start watch
    // - create webserver
    // - other stuff :33

    let config_file = ConfigFile::open_default().await?; 

    let install_pool = Arc::new(InstallPool::new());

    // there is a low likelyhood this will actually block, and if it does
    // it will block only once for a short amount of time, so it's no big deal.
    let webserver = Webserver::make(config_file.config(), Arc::clone(&install_pool))?;

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
