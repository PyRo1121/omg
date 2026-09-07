use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result};
use tokio::sync::oneshot;

use super::alpm_ops::{
    collect_updates, configure_package_filters, configure_signature_policy, get_pkg_info_from_db,
    register_configured_syncdbs,
};
use super::pacman_db::AlpmCatalogEpoch;
use super::types::{PackageInfo, UpdateInfo};
use crate::core::paths;

struct LoadedAlpm {
    handle: alpm::Alpm,
    epoch: AlpmCatalogEpoch,
}

enum AlpmRequest {
    Info(String, oneshot::Sender<Result<Option<PackageInfo>>>),
    ListUpdates(oneshot::Sender<Result<Vec<UpdateInfo>>>),
}

pub struct AlpmWorker {
    tx: mpsc::Sender<AlpmRequest>,
}

fn initialize_alpm_worker() -> Result<alpm::Alpm> {
    let root = paths::pacman_root_result()?.to_string_lossy().into_owned();
    let db_path = paths::pacman_db_dir_result()?
        .to_string_lossy()
        .into_owned();
    let mut alpm = alpm::Alpm::new(root, db_path).context("Failed to initialize ALPM worker")?;
    let config = crate::core::pacman_conf::PacmanConfig::parse(paths::pacman_conf_path())
        .context("Failed to load pacman.conf for ALPM worker")?;
    configure_signature_policy(&alpm, &config)?;
    register_configured_syncdbs(&alpm, &config)?;
    configure_package_filters(&mut alpm, &config)?;
    Ok(alpm)
}

fn load_alpm_worker() -> Result<LoadedAlpm> {
    let handle = initialize_alpm_worker()?;
    let epoch = AlpmCatalogEpoch::observe()
        .context("Failed to observe ALPM catalog epoch after ALPM init")?;
    Ok(LoadedAlpm { handle, epoch })
}

fn reincarnate_if_disk_newer(loaded: &mut LoadedAlpm) -> Result<()> {
    let disk = AlpmCatalogEpoch::observe().context("Failed to observe ALPM catalog epoch")?;
    if disk.disk_is_newer_than(loaded.epoch) {
        *loaded = load_alpm_worker()?;
    }
    Ok(())
}

impl AlpmWorker {
    pub fn new() -> Result<Self> {
        let (tx, rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);

        thread::spawn(move || {
            let mut loaded = match load_alpm_worker() {
                Ok(loaded) => loaded,
                Err(error) => {
                    let message = format!("{error:#}");
                    tracing::error!("Failed to initialize ALPM worker: {message}");
                    let _ = ready_tx.send(Err(message));
                    return;
                }
            };

            tracing::info!(
                "ALPM hot worker ready ({} repos)",
                loaded.handle.syncdbs().len()
            );
            if ready_tx.send(Ok(())).is_err() {
                return;
            }

            while let Ok(req) = rx.recv() {
                match req {
                    AlpmRequest::Info(name, reply) => {
                        let res = match reincarnate_if_disk_newer(&mut loaded) {
                            Ok(()) => get_pkg_info_from_db(&loaded.handle, &name),
                            Err(error) => Err(error),
                        };
                        let _ = reply.send(res);
                    }
                    AlpmRequest::ListUpdates(reply) => {
                        let res = match reincarnate_if_disk_newer(&mut loaded) {
                            Ok(()) => Ok(collect_updates(&loaded.handle)),
                            Err(error) => Err(error),
                        };
                        let _ = reply.send(res);
                    }
                }
            }
            tracing::debug!("ALPM worker shutting down");
        });

        ready_rx
            .recv()
            .context("ALPM worker exited during initialization")?
            .map_err(anyhow::Error::msg)?;
        Ok(Self { tx })
    }

    pub async fn get_info(&self, name: String) -> Result<Option<PackageInfo>> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(AlpmRequest::Info(name, tx))?;

        rx.await
            .context("ALPM worker disconnected (it may have failed to initialize)")?
    }

    pub async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(AlpmRequest::ListUpdates(tx))?;

        rx.await
            .context("ALPM worker disconnected (it may have failed to initialize)")?
    }
}

#[cfg(test)]
mod tests {
    use super::AlpmWorker;

    #[test]
    #[serial_test::serial]
    fn initialization_errors_are_returned_to_the_caller() {
        let directory = tempfile::tempdir().expect("temporary ALPM paths");
        let database = directory.path().join("db");
        std::fs::create_dir(&database).expect("database directory");
        let config = directory.path().join("pacman.conf");
        std::fs::write(
            &config,
            "[options]\nSigLevel = PackageSometimes\n\n[core]\nServer = https://example.invalid\n",
        )
        .expect("pacman config");

        temp_env::with_vars(
            [
                ("OMG_PACMAN_DB_DIR", Some(database.as_os_str())),
                ("OMG_PACMAN_CONF", Some(config.as_os_str())),
            ],
            || {
                let Err(error) = AlpmWorker::new() else {
                    panic!("invalid worker policy must fail");
                };
                assert!(error.to_string().contains("PackageSometimes"), "{error:#}");
            },
        );
    }
}
