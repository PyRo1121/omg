use anyhow::{Context, Result};
use std::sync::mpsc;
use std::thread;

use super::alpm_ops::get_pkg_info_from_db;
use super::types::PackageInfo;
use crate::core::paths;

enum AlpmRequest {
    Info(String, mpsc::Sender<Result<Option<PackageInfo>>>),
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
                }
            }
        });

        Self { tx }
    }

    pub async fn get_info(&self, name: String) -> Result<Option<PackageInfo>> {
        let (tx, rx) = mpsc::channel();
        self.tx.send(AlpmRequest::Info(name, tx))?;

        tokio::task::spawn_blocking(move || rx.recv().context("ALPM worker disconnected")?).await?
    }
}
