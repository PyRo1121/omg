use std::sync::mpsc;
use std::thread;

use anyhow::{Context, Result};
use tokio::sync::oneshot;

use super::alpm_ops::get_pkg_info_from_db;
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
            let alpm = match alpm::Alpm::new(root, db_path) {
                Ok(a) => a,
                Err(e) => {
                    tracing::error!("Failed to initialize ALPM worker: {e}");
                    return;
                }
            };

            let repos = crate::core::pacman_conf::get_configured_repos().unwrap_or_else(|e| {
                tracing::warn!("Failed to parse pacman.conf: {e}. Using default repos.");
                vec![
                    "core".to_string(),
                    "extra".to_string(),
                    "multilib".to_string(),
                ]
            });

            let mut registered = 0;
            for db_name in &repos {
                match alpm.register_syncdb(db_name.as_str(), alpm::SigLevel::USE_DEFAULT) {
                    Ok(_) => registered += 1,
                    Err(e) => tracing::debug!("Failed to register repo '{db_name}': {e}"),
                }
            }

            if registered == 0 {
                tracing::warn!("No sync databases registered in ALPM worker");
            }

            tracing::info!("ALPM hot worker ready ({registered} repos)");

            while let Ok(req) = rx.recv() {
                match req {
                    AlpmRequest::Info(name, reply) => {
                        let res = get_pkg_info_from_db(&alpm, &name);
                        let _ = reply.send(res);
                    }
                    AlpmRequest::ListUpdates(reply) => {
                        let res = Ok(list_updates_from_handle(&alpm));
                        let _ = reply.send(res);
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

        rx.await.context("ALPM worker disconnected")?
    }

    pub async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(AlpmRequest::ListUpdates(tx))?;

        rx.await.context("ALPM worker disconnected")?
    }
}

/// List updates using an already-initialized ALPM handle (zero startup cost)
fn list_updates_from_handle(alpm: &alpm::Alpm) -> Vec<UpdateInfo> {
    let localdb = alpm.localdb();
    let syncdbs = alpm.syncdbs();
    let local_pkg_count = localdb.pkgs().len();

    // Build HashMap of sync packages: name -> (version_str, repo_name)
    let mut sync_map: ahash::AHashMap<&str, (&str, &str)> =
        ahash::AHashMap::with_capacity(local_pkg_count);

    for db in syncdbs {
        let repo_name = db.name();
        for pkg in db.pkgs() {
            sync_map
                .entry(pkg.name())
                .or_insert_with(|| (pkg.version().as_str(), repo_name));
        }
    }

    let mut updates = Vec::with_capacity(local_pkg_count / 20);

    for pkg in localdb.pkgs() {
        let name = pkg.name();
        let local_ver_str = pkg.version().as_str();

        if let Some(&(sync_ver_str, repo)) = sync_map.get(name)
            && alpm::vercmp(sync_ver_str, local_ver_str) == std::cmp::Ordering::Greater
        {
            updates.push(UpdateInfo {
                name: name.to_string(),
                old_version: local_ver_str.to_string(),
                new_version: sync_ver_str.to_string(),
                repo: repo.to_string(),
            });
        }
    }

    updates
}
