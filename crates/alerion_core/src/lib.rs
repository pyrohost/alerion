#![deny(clippy::unwrap_used)]

use config::AlerionConfig;
use futures::stream::{FuturesUnordered, StreamExt};

use crate::filesystem::setup_directories;

pub fn splash() {
    println!(
        "

 █████  ██      ███████ ██████  ██  ██████  ███    ██ 
██   ██ ██      ██      ██   ██ ██ ██    ██ ████   ██ 
███████ ██      █████   ██████  ██ ██    ██ ██ ██  ██ 
██   ██ ██      ██      ██   ██ ██ ██    ██ ██  ██ ██ 
██   ██ ███████ ███████ ██   ██ ██  ██████  ██   ████

Copyright (c) 2024 Pyro Host Inc. All Right Reserved.

Pyro Alerion is licensed under the Pyro Source Available
License (PSAL). Your use of this software is governed by
the terms of the PSAL. If you don't agree to the terms of
the PSAL, you are not permitted to use this software. 

License: https://github.com/pyrohost/legal/blob/main/licenses/PSAL.md
Source code: https://github.com/pyrohost/alerion");
}

/// Alerion main entrypoint. Expects a tokio runtime to be setup.
pub async fn alerion_main() -> anyhow::Result<()> {
    splash();

    tracing::info!("Starting Alerion");

    let project_dirs = setup_directories().await?;
    let config = AlerionConfig::load(&project_dirs)?;

    //let server_pool = Arc::new(ServerPool::builder(&config)?.fetch_servers().await?.build());

    //server_pool.create_server("0e4059ca-d79b-46a5-8ec4-95bd0736d150".try_into().unwrap()).await;

    let webserver_handle = tokio::spawn(async move {
        let cfg = config.clone();
        let result = webserver::serve(cfg).await;

        match result {
            Ok(()) => tracing::info!("webserver exited gracefully"),
            Err(e) => tracing::error!("webserver exited with an error: {e}"),
        }
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
pub mod filesystem;
pub mod servers;
pub mod webserver;
pub mod websocket;

