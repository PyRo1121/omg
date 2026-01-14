use anyhow::{Context, Result};
use std::sync::mpsc;
use std::thread;

use super::alpm_ops::{get_pkg_info_from_db, PackageInfo};

/// Request type for the ALPM worker
enum AlpmRequest {
    Info(String, mpsc::Sender<Result<Option<PackageInfo>>>),
}

/// A handle to a dedicated thread that keeps libalpm 'hot'
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
            // Initialize ALPM once and keep it alive in this thread
            let alpm = match alpm::Alpm::new("/", "/var/lib/pacman") {
                Ok(a) => a,
                Err(e) => {
                    tracing::error!("Failed to initialize ALPM worker: {}", e);
                    return;
                }
            };

            // Register sync DBs
            for db_name in ["core", "extra", "multilib"] {
                let _ = alpm.register_syncdb(db_name, alpm::SigLevel::USE_DEFAULT);
            }

            tracing::info!("ALPM hot worker ready");

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

        // Convert sync rx to async wait (spawn_blocking)
        tokio::task::spawn_blocking(move || rx.recv().context("ALPM worker disconnected")?).await?
    }
}
