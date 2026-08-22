use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result};
use tokio::sync::oneshot;

use super::alpm_ops::{collect_updates, configure_package_filters, get_pkg_info_from_db};
use super::types::{PackageInfo, UpdateInfo};
use crate::core::paths;

enum AlpmRequest {
    Info(String, oneshot::Sender<Result<Option<PackageInfo>>>),
    ListUpdates(oneshot::Sender<Result<Vec<UpdateInfo>>>),
}

pub struct AlpmWorker {
    tx: mpsc::Sender<AlpmRequest>,
}

impl Default for AlpmWorker {
    fn default() -> Self {
        Self::new()
    }
}

impl AlpmWorker {
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            let root = paths::pacman_root().to_string_lossy().into_owned();
            let db_path = paths::pacman_db_dir().to_string_lossy().into_owned();
            let mut alpm = match alpm::Alpm::new(root, db_path) {
                Ok(a) => a,
                Err(e) => {
                    tracing::error!("Failed to initialize ALPM worker: {e}");
                    return;
                }
            };

            let repos = match crate::core::pacman_conf::get_configured_repos() {
                Ok(repos) => repos,
                Err(error) => {
                    tracing::error!("Failed to load repositories from pacman.conf: {error}");
                    return;
                }
            };

            let mut registered = 0;
            for db_name in &repos {
                match alpm.register_syncdb(db_name.as_str(), alpm::SigLevel::USE_DEFAULT) {
                    Ok(_) => registered += 1,
                    Err(e) => tracing::debug!("Failed to register repo '{db_name}': {e}"),
                }
            }

            if registered == 0 {
                tracing::warn!("No sync databases registered in ALPM worker");
            } else {
                // Apply pacman.conf ignore filters so worker answers agree
                // with the CLI path (`alpm_ops::get_update_list`).
                match crate::core::pacman_conf::PacmanConfig::parse(paths::pacman_conf_path()) {
                    Ok(config) => {
                        if let Err(e) = configure_package_filters(&mut alpm, &config) {
                            tracing::warn!("Failed to configure update filters: {e}");
                        }
                    }
                    Err(error) => {
                        tracing::warn!("Failed to load pacman.conf filters: {error}");
                    }
                }
            }

            tracing::info!("ALPM hot worker ready ({registered} repos)");

            while let Ok(req) = rx.recv() {
                match req {
                    AlpmRequest::Info(name, reply) => {
                        let res = get_pkg_info_from_db(&alpm, &name);
                        let _ = reply.send(res);
                    }
                    AlpmRequest::ListUpdates(reply) => {
                        let _ = reply.send(Ok(collect_updates(&alpm)));
                    }
                }
            }
            tracing::debug!("ALPM worker shutting down");
        });

        Self { tx }
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
