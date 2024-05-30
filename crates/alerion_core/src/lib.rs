#![deny(clippy::unwrap_used)]

use std::sync::Arc;

use fs::config::Config;
use futures::stream::{FuturesUnordered, StreamExt};

use crate::servers::pool::ServerPool;
use crate::fs::LocalDataHandle;

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
Source code: https://github.com/pyrohost/alerion
"
    );
}

/// Alerion main entrypoint. Expects a tokio runtime to be setup.
pub async fn alerion_main() -> anyhow::Result<()> {
    splash();

    tracing::info!("starting alerion");

    let config = Config::load()?;
    let localdata = LocalDataHandle::new(config.data_dir.clone()).await?;

    let server_pool = ServerPool::new(&config, localdata.clone()).await?;

    let webserver_handle = tokio::spawn(async move {
        let cfg = config.clone();
        let result = webserver::serve(cfg, Arc::clone(&server_pool)).await;

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

pub mod fs;
pub mod os;
pub mod servers;
pub mod webserver;
pub mod docker;
