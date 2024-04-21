#![deny(clippy::unwrap_used)]
#![allow(dead_code)]

use tracing_subscriber::filter;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(filter::EnvFilter::from_default_env())
        .init();

    alerion_core::alerion_main().await?;

    Ok(())
}
