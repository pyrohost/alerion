use tokio::task::JoinHandle;
use tokio::fs;
use bollard::{models, Docker};
use uuid::Uuid;

use crate::servers::docker;
use super::models::{volume, container, Volume, VolumeName, FoundVolume, BindMount, Container, ContainerName, FoundContainer};


/// Initiates the installation of a server.  
///
/// Please, ensure check was made to ensure the installation was not properly
/// completeled beforehand, as this will undo and delete any previous installation
/// attempt's progress.  
///
/// If this is a reinstall, delete the involved containers and volumes beforehand
/// to avoid warnings being emitted.  
///
/// 1. Create necessary volumes.
/// 2. Create the installation container.
/// 3. Start the container
/// 4. Watch the container
pub async fn process(
    api: &Docker,
    uuid: Uuid,
) -> docker::Result<JoinHandle<docker::Result<()>>> {
    let install_volume = {
        let name = VolumeName::new_install(uuid);

        match Volume::get(api, name.clone()).await? {
            FoundVolume::Some(vol) => {
                tracing::warn!(
                    "the installation volume was already created by alerion, but not deleted"
                );
                tracing::warn!("creation time: {}", vol.created_at().unwrap_or("unknown"));
                tracing::warn!(
                    "this signals alerion might have crashed during the installation process"
                );
                tracing::warn!(
                    "the volume will be force-deleted and the installation process will restart"
                );

                volume::force_remove_by_name(api, &name.full_name()).await?;
            }

            FoundVolume::Foreign(resp) => {
                tracing::warn!("the installation volume was already created, but not by alerion");
                tracing::warn!("this might be an artifact from wings");
                tracing::warn!(
                    "the volume will be force-deleted and the installation process will start"
                );

                tracing::debug!("Docker response body: {resp:#?}");

                volume::force_remove_by_name(api, &name.full_name()).await?;
            }

            FoundVolume::None => {
                tracing::debug!("installation volume not found: OK");
            }
        }

        tracing::info!("creating installation volume");

        let vol_fut = Volume::create(api, name);
        crate::ensure!(vol_fut.await, "failed to create server volume")
    };

    let server_bind_mount = {
        tracing::info!("creating server bind mount");
        let mount = BindMount::new(uuid).await?;

        mount
    };

    let install_container = {
        let name = ContainerName::new_install(uuid);

        match Container::get(api, name.clone()).await? {
            FoundContainer::Some(cont) => {
                tracing::warn!(
                    "the installation container already exists and was created by alerion"
                );
                tracing::warn!("creation time: {}", cont.created_at().unwrap_or("unknown"));
                tracing::warn!("this could either mean alerion crashed, or the installation");
                tracing::warn!("process is not supposed to run right now and this is a bug");
                tracing::warn!(
                    "the container will be deleted and the installation process will restart"
                );

                cont.force_remove(api).await?;
            }

            FoundContainer::Foreign(resp) => {
                tracing::warn!(
                    "the installation container already exists, but wasn't created by alerion"
                );
                tracing::warn!("this could be an artifact from wings");
                tracing::warn!(
                    "the container will be deleted and the installation process will start"
                );

                tracing::debug!("Docker response body: {resp:#?}");

                container::force_remove_by_name_or_id(api, &name.full_name()).await?;
            }

            FoundContainer::None => {
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
