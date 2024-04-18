#![deny(clippy::unwrap_used)]
#![allow(dead_code)]

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    alerion_core::alerion_main().await?;

    Ok(())
}
