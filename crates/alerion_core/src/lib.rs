#![deny(clippy::unwrap_used)]

use std::sync::Arc;

use configuration::AlerionConfig;
use futures::stream::{FuturesUnordered, StreamExt};

use crate::servers::pool::ServerPool;

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

    servers::docker::environment::setup()?;

    let config = AlerionConfig::load().await?;

    let server_pool = ServerPool::new(&config).await?;
    let server_pool = Arc::new(server_pool);

    let webserver_handle = tokio::spawn(async move {
        let cfg = config.clone();
        let result = webserver::serve(&cfg, Arc::clone(&server_pool)).await;

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

#[macro_export]
macro_rules! ensure {
    ($value:expr, $fmt:expr $(, $arg:expr)* $(,)?) => {
        match $value {
            ::std::result::Result::Ok(ok_value) => ok_value,
            ::std::result::Result::Err(error_value) => {
                ::tracing::error!("{}: {error_value}", format!($fmt $(, $arg)*));
                return ::std::result::Result::Err(std::convert::Into::into(error_value));
            }
        }

    }
}

pub mod configuration;
pub mod servers;
pub mod webserver;
pub mod websocket;
pub mod os;
