#![deny(clippy::unwrap_used)]
#![allow(dead_code)]

use tracing_subscriber::filter;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    std::env::set_var("RUST_LOG", "info,alerion_core=debug");

    tracing_subscriber::fmt()
        .compact()
        .without_time()
        .with_env_filter(filter::EnvFilter::from_default_env())
        .init();

    alerion_core::alerion_main().await?;

    Ok(())
}
