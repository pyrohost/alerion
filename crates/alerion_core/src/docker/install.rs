use std::collections::HashMap;
use std::path::Path;
use std::io;
use std::borrow::Cow;
use std::sync::Arc;

use bollard::container::LogOutput;
use bollard::errors::Error as BollardError;
use futures::{Stream, StreamExt};
use tokio::fs;
use bollard::models;

use alerion_datamodel::remote::server::{GetServerByUuidResponse, GetServerInstallByUuidResponse};
use crate::docker::models::bind_mount::BindMountName;
use crate::servers::server::{Server, State, OutboundMessage};
use crate::docker::{
    self,
    models::{BindMount, Container, ContainerName},
};

const INSTALLER_SCRIPT_NAME: &str = "installer.sh";
// please ew
const MIB_TO_BYTES: i64 = 1000 * 1000;

/// Initiates the installation of a server.  
///
/// Please, ensure check was made to ensure the installation was not properly
/// completeled beforehand, as this will undo and delete any previous installation
/// attempt's progress.  
///
/// If this is a reinstall, delete the involved containers and volumes beforehand
/// to avoid warnings being emitted.  
pub async fn engage(
    server: &Arc<Server>,
    server_cfg: &GetServerByUuidResponse, 
    install_cfg: GetServerInstallByUuidResponse,
) -> docker::Result<()> {
    let uuid = server.uuid;
    let api = server.docker_api();
    let localdata = server.localdata();

    let mounts = localdata.mounts();

    // 1. Installation mount
    let install_mount = {
        tracing::debug!("creating installer bind mount");
        let name = BindMountName::new_installer(uuid);
        BindMount::new_clean(&mounts, name).await?
    };

    // 2. Create the server's bind mount.
    //
    // Make sure it's empty; at this point, its contents should be backed up/ready to be lost.
    // This uses a bind mount because we need the flexibility that those provide (support
    // being modified by the host.)
    let server_mount = {
        tracing::debug!("creating server bind mount");
        let name = BindMountName::new_server(uuid);
        BindMount::new_clean(&mounts, name).await?
    };


    // 3. Create the container for the installation process
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
            server_cfg,
            entrypoint,
            container_image,
            volumes,
        );

        tracing::debug!("{config:#?}");

        Container::recreate(api, name, config).await?
    };

    // put the installation script in the install volume
    let script_normalized = normalize_script(&script);
    if let Err(e) = installer_script(install_mount.path(), &script_normalized).await {
        tracing::error!("failed to write installation script to container mountpoint: {e}");
        return Err(e.into());
    }

    if let Err(e) = install_container.start(api).await {
        tracing::error!("container failed to start: {e:?}");
        return Err(e);
    }

    // spawn a monitoring task
    {
        let server = Arc::clone(server);
        let api = api.clone();
        tokio::spawn(async move {
            let result = install_container.attach(&api, false).await; 

            let (_input, mut output) = match result {
                Ok(tuple) => tuple,
                Err(e) => {
                    tracing::error!("failed to attach to container: {e}");
                    return;
                }
            };

            monitor(&server, &mut output).await;

            let mut success = true;

            match install_container.inspect_existing(&api).await {
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
                    tracing::error!("assuming success");
                }
            }

            *server.state.lock() = State::Installed;

            match server.set_installation_status(success).await {
                Ok(()) => tracing::debug!("notified remote API of installation status"),
                Err(e) => tracing::error!("couldn't notify the panel about the installation status: {e}"),
            }
        });
    }

    tracing::info!("server installation process successfully engaged");

    Ok(())
}

#[tracing::instrument(skip_all, name = "monitor_installer")]
async fn monitor(
    server: &Arc<Server>,
    stream: &mut (dyn Stream<Item = Result<LogOutput, BollardError>> + Unpin + Send),
) {
    let mut logfile = server.install_logfile().await;

    while let Some(result) = stream.next().await {
         match result {
            Ok(output) => {
                let bytes = output.into_bytes();
                let sanitized = Arc::new(sanitize_output(&bytes));

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

/// Sanitizes the given bytes to remove bad control characters
fn sanitize_output(bytes: &[u8]) -> String {
    // would be better if it didn't strip colors and stuff but oh well

    // strip controls except whitespaces
    String::from_utf8_lossy(bytes)
        .as_ref()
        .chars()
        // REPLACEMENT_CHARACTER.is_whitespace() == false
        .filter(|c| !c.is_control() || c.is_whitespace())
        .collect::<String>()
}

async fn installer_script(mount_path: &Path, contents: &str) -> io::Result<()> {
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
    let env = format_environment_for_docker(&cfg.settings.environment);
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
            // why is this even signed
            let mem_hard_limit = i64::max(0, build.memory_limit) * MIB_TO_BYTES;
            // set hard limit to 20% more than whatever the actual limit is
            let generous_mem = mem_hard_limit + mem_hard_limit / 5;

            let cpu_period;
            let cpu_quota;
            let cpu_shares;

            // wings does this, not giving cpu_period/shares if a cpu limit isn't set
            // bcuz of java bugs, odd https://github.com/pterodactyl/panel/issues/3988
            if build.cpu_limit > 0 {
                cpu_period = Some(100_000); // docker default
                cpu_shares = Some(1024); // docker default
                cpu_quota = Some((build.cpu_limit * 1000) as i64); // cpu_limit is in percentage
            } else {
                cpu_period = None;
                cpu_shares = None;
                cpu_quota = None;
            }

            // TODO: check for /tmp, tmpfs mount?

            bollard::models::HostConfig {
                memory: Some(mem_hard_limit),
                cgroup_parent: None,
                blkio_weight: Some(build.io_weight),
                cpuset_cpus: build.threads.clone(),
                memory_swap: Some(build.swap + build.memory_limit),
                // TODO: Experiment with swappiness. Definitely ensure a high value though.
                memory_swappiness: Some(70),
                memory_reservation: Some(generous_mem),
                cpu_period,
                cpu_shares,
                cpu_quota,
                oom_kill_disable: Some(build.oom_disabled),
                mounts: Some(mounts),
                userns_mode: None,
                // TODO: Config option
                pids_limit: Some(256),
                // We should use rootless docker
                cap_drop: None,
                ..Default::default()
            }
        }),
        ..Default::default()
    }
}

fn normalize_script(script: &str) -> String {
    // replace ALL carriage returns, not just those prefixed with newlines
    script.replace('\r', "\n")
}

fn format_environment_for_docker(env: &HashMap<String, serde_json::Value>) -> Vec<String> {
    env.iter().map(|(k, v)| {
        // now we hope the env variables won't create security issues!
        // TODO
        let value = v.as_str()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(format!("{v}")));

        format!("{k}={value}")
    }).collect()
}
