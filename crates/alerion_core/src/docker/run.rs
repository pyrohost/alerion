use std::sync::Arc;

use alerion_datamodel::remote::server::GetServerByUuidResponse;
use bollard::container::LogOutput;
use bollard::errors::Error as BollardError;
use futures::{Stream, StreamExt};

use crate::servers::{self, Server, State, OutboundMessage};
use crate::docker::{alerion_version_labels, util, BindMountName, BindMount, Container, ContainerName};

const CONTAINER_USER: &str = "container";
const CONTAINER_HOME: &str = "/home/container";

pub async fn engage(server: Arc<Server>) -> servers::Result<()> {
    let mounts = &server.fs.mounts;
    let api = &server.docker;

    let server_cfg = server.remote.get_server_configuration().await?;

    let server_mount = match BindMount::get(mounts, BindMountName::new_server(server.uuid)).await {
        Ok(b) => {
            tracing::debug!("got server's bind mount");
            b
        }

        Err(e) => {
            tracing::error!("couldn't find server's bind mount: {e}");
            return Err(e.into());
        }
    };

    let container = {
        let name = ContainerName::new_server(server.uuid);
        let mnt = server_mount.to_docker_mount(CONTAINER_HOME.to_owned());

        let config = docker_configuration(&name, &server_cfg, mnt);

        match Container::force_create(api, name, config).await {
            Ok(c) => {
                tracing::debug!("created server container");
                c
            },

            Err(e) => {
                tracing::error!("couldn't create server container: {e}");
                return Err(e.into());
            }
        }
    };

    if let Err(e) = container.start(api).await {
        tracing::error!("failed to start server container: {e}");
        return Err(e.into());
    }

    let _ = server.fs.db.update(|s| s.state = State::Starting).await;

    let (_input, mut output) = match container.attach(api, false).await {
        Ok(tuple) => tuple,
        Err(e) => {
            tracing::error!("failed to attach to server container: {e}");
            return Err(e.into());
        }
    };

    monitor(server, &mut output).await; 

    Ok(())
}

async fn monitor(
    server: Arc<Server>,
    stream: &mut (dyn Stream<Item = Result<LogOutput, BollardError>> + Unpin + Send),
) {
    let mut logfile = server.fs.logger.open_server().await;

    while let Some(result) = stream.next().await {
         match result {
            Ok(output) => {
                let bytes = output.into_bytes();
                let sanitized = Arc::new(util::sanitize_output(&bytes));

                let msg = OutboundMessage::server_output(Arc::clone(&sanitized));
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

fn docker_configuration(
    name: &ContainerName,
    cfg: &GetServerByUuidResponse,
    server_mount: bollard::models::Mount,
) -> bollard::container::Config<String> {
    let env = util::format_environment_for_docker(&cfg.settings.environment);
    let image = cfg.settings.container.image.clone();

    bollard::container::Config {
        hostname: Some(name.short_uid()),
        domainname: None,
        user: Some(CONTAINER_USER.to_owned()),
        attach_stdin: Some(true),
        attach_stdout: Some(true),
        attach_stderr: Some(true),
        exposed_ports: None,
        tty: Some(false),
        open_stdin: Some(true),
        stdin_once: Some(false),
        env: Some(env),
        // set by image
        cmd: None,
        healthcheck: None,
        args_escaped: Some(true),
        image: Some(image),
        volumes: None,
        // set by image
        working_dir: None,
        entrypoint: Some(vec!["touch".to_owned(), "hi".to_owned()]),
        network_disabled: Some(false),
        mac_address: None,
        on_build: None,
        labels: Some(alerion_version_labels()),
        stop_signal: None,
        stop_timeout: Some(600),
        shell: None,
        host_config: Some({
            const MIB_TO_BYTES: i64 = 1000 * 1000;

            let build = &cfg.settings.build;
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

            bollard::models::HostConfig {
                memory: Some(mem_hard_limit),
                cgroup_parent: None,
                blkio_weight: Some(build.io_weight),
                cpuset_cpus: build.threads.clone(),
                memory_swap: Some(build.swap + build.memory_limit),
                memory_swappiness: None,
                memory_reservation: Some(generous_mem),
                cpu_period,
                cpu_shares,
                cpu_quota,
                oom_kill_disable: Some(build.oom_disabled),
                mounts: Some(vec![server_mount]),
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
