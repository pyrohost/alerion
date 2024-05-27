use std::collections::HashMap;

use tokio::task::JoinHandle;
use tokio::fs;
use bollard::{models, Docker};
use uuid::Uuid;

use alerion_datamodel::remote::server::{GetServerByUuidResponse, GetServerInstallByUuidResponse};
use crate::servers::Server;
use crate::os::{User, UserImpl};
use crate::docker::{
    self,
    models::{volume, container, Inspectable, Inspected, Volume, VolumeName, BindMount, Container, ContainerName},
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
pub async fn process(
    server: &Server,
    server_cfg: &GetServerByUuidResponse, 
) -> docker::Result<JoinHandle<docker::Result<()>>> {
    let uuid = server.uuid;
    let api = server.docker_api();

    // 1. Installation volume
    //
    // This uses a named volume.
    let install_volume = {
        let name = VolumeName::new_install(uuid);

        match Volume::inspect(api, name.clone()).await? {
            Inspected::Some(vol) => {
                tracing::warn!("the installation volume was already created by alerion, but not deleted");
                tracing::warn!("creation time: {}", vol.created_at().unwrap_or("unknown"));
                tracing::warn!("this signals alerion might have crashed during the installation process");
                tracing::warn!("the volume will be force-deleted and the installation process will restart");

                volume::force_remove_by_name(api, &name.full_name()).await?;
            }

            Inspected::Invalid(resp) => {
                tracing::warn!("the installation volume was already created, but not by alerion");
                tracing::warn!("this might be an artifact from wings");
                tracing::warn!("the volume will be force-deleted and the installation process will start");

                tracing::debug!("Docker response body: {resp:#?}");

                volume::force_remove_by_name(api, &name.full_name()).await?;
            }

            Inspected::None => {
                tracing::debug!("installation volume not found: OK");
            }
        }

        tracing::info!("creating installation volume");

        let vol_fut = Volume::create(api, name);
        crate::ensure!(vol_fut.await, "failed to create server volume")
    };

    // 2. Create the server's bind mount.
    //
    // Make sure it's empty; at this point, its contents should be backed up/ready to be lost.
    // This uses a bind mount because we need the flexibility that those provide (support
    // being modified by the host.)
    let server_bind_mount = {
        tracing::info!("creating server bind mount");
        BindMount::new_clean(uuid).await?
    };

    // 3. Create the container for the installation process
    let install_container = {
        let name = ContainerName::new_install(uuid);

        match Container::inspect(api, name.clone()).await? {
            Inspected::Some(cont) => {
                tracing::warn!("the installation container already exists and was created by alerion");
                tracing::warn!("creation time: {}", cont.created_at().unwrap_or("unknown"));
                tracing::warn!("this could either mean alerion crashed, or the installation");
                tracing::warn!("the container will be deleted and the installation process will restart");

                cont.force_remove(api).await?;
            }

            Inspected::Invalid(resp) => {
                tracing::warn!("the installation container already exists, but wasn't created by alerion");
                tracing::warn!("this could be an artifact from wings");
                tracing::warn!("the container will be deleted and the installation process will start");

                tracing::debug!("Docker response body: {resp:#?}");

                container::force_remove_by_name_or_id(api, &name.full_name()).await?;
            }

            Inspected::None => {
                tracing::debug!("installation container not found: OK");
            }
        }

        let volumes = vec![
            install_volume.to_docker_mount("/mnt/install".to_owned()),
            server_bind_mount.to_docker_mount("/mnt/server".to_owned()),
        ];

        let host_cfg = models::HostConfig {
            mounts: Some(volumes),
            ..models::HostConfig::default()
        };

        tracing::info!("creating installation container");

        let cont_fut = Container::create(api, name, host_cfg);
        crate::ensure!(cont_fut.await, "failed to create installation container")
    };

    // put the installation script in the install volume
    let path = install_volume.mountpoint.join("install.sh");
    let script = br#"
    echo "hello, $SUBJECT!"
    whoami
    pwd
    touch tmpfile
    cd /mnt/server
    echo ok > serverfile.txt
    "#;
    if let Err(e) = fs::write(path, script).await {
        tracing::error!("failed to write installation script: {e:?}");
        return Err(e.into());
    };

    if let Err(e) = install_container.start(api).await {
        tracing::error!("container failed to start: {e:?}");
        return Err(e);
    }

    let monitor_api = api.clone();
    let handle = tokio::spawn(async move {
        let attach_fut = install_container.attach(&monitor_api);
        crate::ensure!(attach_fut.await, "failed to attach to container");

        Ok(())
    });

    Ok(handle)
}

fn docker_install_container_config(
    name: &ContainerName,
    cfg: &GetServerByUuidResponse,
    entrypoint: String,
    image: String,
    username: String,
    mounts: Vec<models::Mount>,
) -> bollard::container::Config<String> {
    let env = format_environment_for_docker(&cfg.settings.environment);
    let build = &cfg.settings.build;
    
    bollard::container::Config {
        hostname: Some(name.short_uid()),
        // only relevant if using NIS, not relevant here
        domainname: None,
        user: Some(username),
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
        // the installer script doesn't assume any starting working dir
        working_dir: None,
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

fn format_environment_for_docker(env: &HashMap<String, serde_json::Value>) -> Vec<String> {
    env.into_iter().map(|(k, v)| format!("{k}={v}")).collect()
}
