#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    alerion_core::alerion_main().await?;

    Ok(())
}
