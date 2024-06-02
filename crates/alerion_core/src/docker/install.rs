use std::path::Path;
use std::io;
use std::sync::Arc;

use bollard::container::LogOutput;
use bollard::errors::Error as BollardError;
use futures::{Stream, StreamExt};
use tokio::fs;
use bollard::models;

use alerion_datamodel::remote::server::{GetServerByUuidResponse, GetServerInstallByUuidResponse};
use crate::docker::{util, BindMountName};
use crate::servers::{self, OutboundMessage, Server, State};
use crate::docker::{self, BindMount, Container, ContainerName};

const INSTALLER_SCRIPT_NAME: &str = "installer.sh";
// please ew

/// Initiates the installation of a server.  
///
/// Please, ensure check was made to ensure the installation was not properly
/// completeled beforehand, as this will undo and delete any previous installation
/// attempt's progress.  
///
/// If this is a reinstall, delete the involved containers and volumes beforehand
/// to avoid warnings being emitted.  
#[tracing::instrument(skip_all, name = "installation")]
pub async fn engage(server: Arc<Server>) -> servers::Result<bool> {
    let server_cfg = server.remote.get_server_configuration().await?;
    let install_cfg = server.remote.get_install_instructions().await?;

    server.fs.db
        .update(|m| m.state = State::Installing).await;

    let success = installation_core(Arc::clone(&server), server_cfg, install_cfg).await;
    Server::mark_install_status(server, success).await;

    Ok(success)
}

async fn installation_core(server: Arc<Server>, server_cfg: GetServerByUuidResponse, install_cfg: GetServerInstallByUuidResponse) -> bool { 
    let mounts = &server.fs.mounts; 
    let uuid = server.uuid;
    let api = &server.docker;

    let install_mount = {
        let name = BindMountName::new_installer(uuid);

        match BindMount::new_clean(mounts, name).await {
            Ok(b) => {
                tracing::debug!("recreated installation bind mount");
                b
            }

            Err(e) => {
                tracing::error!("failed to create installation bind mount: {e}");
                return false;
            }
        }
    };

    // create the server's bind mount.
    //
    // Make sure it's empty; at this point, its contents should be backed up/ready to be lost.
    // This uses a bind mount because we need the flexibility that those provide (support
    // being modified by the host.)
    let server_mount = {
        let name = BindMountName::new_server(uuid);

        match BindMount::new_clean(mounts, name).await {
            Ok(b) => {
                tracing::debug!("recreated server bind mount");
                b
            }

            Err(e) => {
                tracing::error!("failed to create server bind mount: {e}");
                return false;
            }
        }
    };

    // create the container for the installation process
    let GetServerInstallByUuidResponse {
        container_image,
        entrypoint,
        script,
    } = install_cfg;

    let install_container = {
        let name = ContainerName::new_install(uuid);

        let volumes = vec![
            install_mount.to_docker_mount("/mnt/install".to_owned()),
            server_mount.to_docker_mount("/mnt/server".to_owned()),
        ];

        let config = docker_install_container_config(
            &name,
            &server_cfg,
            entrypoint,
            container_image,
            volumes,
        );

        match Container::force_create(api, name, config).await {
            Ok(c) => {
                tracing::debug!("installation container created");
                c
            }

            Err(e) => {
                tracing::error!("failed to create installation container: {e}");
                return false;
            }
        }
    };

    // put the installation script in the install volume
    let script_normalized = normalize_script(&script);
    if let Err(e) = write_installer_script(install_mount.path(), &script_normalized).await {
        tracing::error!("failed to write installation script to container mountpoint: {e}");
        return false;
    }

    if let Err(e) = install_container.start(api).await {
        tracing::error!("container failed to start: {e:?}");
        return false;
    }

    // spawn a monitoring task
    let result = install_container.attach(api, false).await; 

    let (_input, mut output) = match result {
        Ok(tuple) => tuple,
        Err(e) => {
            tracing::error!("failed to attach to container: {e}");
            return false;
        }
    };

    monitor(&server, &mut output).await;

    let mut success = true;

    match install_container.inspect_existing(api).await {
        Ok(resp) => {
            if let Some(code) = resp.state.and_then(|s| s.exit_code) {
                if code == 0 {
                    tracing::info!("server installed successfully");
                } else {
                    tracing::error!("failed to install server (exit={code})");
                    success = false;
                }
            } else {
                tracing::error!("cannot get exit code of the installer");
                tracing::error!("assuming success");
            }
        }

        Err(e) => {
            tracing::error!("failed to inspect docker container after installation: {e}");
            tracing::error!("assuming failure");
            success = false;
        }
    }

    match install_container.force_remove(api).await {
        Ok(()) => tracing::debug!("deleted installation container"),
        Err(e) => tracing::error!("failed to delete installation container: {e}"),
    }

    success
}


async fn monitor(
    server: &Arc<Server>,
    stream: &mut (dyn Stream<Item = Result<LogOutput, BollardError>> + Unpin + Send),
) {
    let mut logfile = server.fs.logger.open_install().await;

    while let Some(result) = stream.next().await {
         match result {
            Ok(output) => {
                let bytes = output.into_bytes();
                let sanitized = Arc::new(util::sanitize_output(&bytes));

                let msg = OutboundMessage::install_output(Arc::clone(&sanitized));
                server.websocket.broadcast(msg);

                logfile.write(&sanitized).await;
            },

            Err(e) => {
                tracing::error!("failed to read output stream: {e}");
            }
        }
    }

    logfile.flush().await;
}

async fn write_installer_script(mount_path: &Path, contents: &str) -> io::Result<()> {
    let path = mount_path.join(INSTALLER_SCRIPT_NAME);
    tracing::debug!("writing installer at '{}'", path.display());

    fs::write(path, contents).await?;

    Ok(())
}

fn docker_install_container_config(
    name: &ContainerName,
    cfg: &GetServerByUuidResponse,
    entrypoint: String,
    image: String,
    mounts: Vec<models::Mount>,
) -> bollard::container::Config<String> {
    let env = util::format_environment_for_docker(&cfg.settings.environment);
    let build = &cfg.settings.build;
    
    bollard::container::Config {
        hostname: Some(name.short_uid()),
        // only relevant if using NIS, not relevant here
        domainname: None,
        user: Some("0:0".to_owned()),
        attach_stdin: Some(true),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        exposed_ports: None,
        tty: Some(false),
        open_stdin: Some(true),
        stdin_once: Some(true),
        env: Some(env),
        cmd: Some(vec![entrypoint, INSTALLER_SCRIPT_NAME.to_owned()]),
        // should inherit? or disable.. idk
        healthcheck: None,
        args_escaped: Some(true),
        image: Some(image),
        volumes: None,
        working_dir: Some("/mnt/install".to_owned()),
        // no need for an entrypoint, using `Cmd` instead
        entrypoint: None,
        network_disabled: Some(false),
        // deprecated!
        mac_address: None,
        on_build: None,
        labels: Some(docker::alerion_version_labels()),
        // leave the default at SIGTERM
        stop_signal: None,
        // no need for a very high stop timeout
        stop_timeout: Some(5),
        // irrelevant
        shell: None,
        // - should we use cgroups?
        host_config: Some({
            const ONE_GB: i64 = 1024 * 1024 * 1024;
            bollard::models::HostConfig {
                memory: Some(4 * ONE_GB),
                blkio_weight: Some(build.io_weight),
                memory_swap: Some(4 * ONE_GB),
                memory_reservation: Some(4 * ONE_GB),
                mounts: Some(mounts),
                ..Default::default()
            }
        }),
        ..Default::default()
    }
}

fn normalize_script(script: &str) -> String {
    // replace ALL carriage returns, not just those prefixed with newlines
    // ash doesn't like carriage returns
    script.replace('\r', "\n")
}

