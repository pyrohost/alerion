use anyhow::Context;
use directories::ProjectDirs;

#[tracing::instrument]
pub async fn setup_directories() -> anyhow::Result<ProjectDirs> {
    let project_dirs = ProjectDirs::from("host", "pyro", "alerion")
        .context("couldn't determine a home directory for your operating system")?;

    tokio::fs::create_dir_all(project_dirs.config_dir()).await?;
    tokio::fs::create_dir_all(project_dirs.data_dir()).await?;
    tokio::fs::create_dir_all(project_dirs.cache_dir()).await?;
    
    tracing::info!("Directories created");

    Ok(project_dirs)
}
