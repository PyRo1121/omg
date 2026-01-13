//! Persistent metadata cache using LMDB (heed)
//!
//! Stores system status metrics to survive daemon restarts.

use super::protocol::StatusResult;
use anyhow::{Context, Result};
use heed::types::*;
use heed::{Database, Env, EnvOpenOptions};
use std::path::Path;

pub struct PersistentCache {
    env: Env,
    status_db: Database<Str, SerdeJson<StatusResult>>,
}

impl PersistentCache {
    pub fn new(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(10 * 1024 * 1024) // 10MB
                .max_dbs(1)
                .open(path)
                .context("Failed to open LMDB environment")?
        };

        let mut wtxn = env.write_txn()?;
        let status_db = env.create_database(&mut wtxn, Some("status"))?;
        wtxn.commit()?;

        Ok(PersistentCache { env, status_db })
    }

    pub fn get_status(&self) -> Result<Option<StatusResult>> {
        let rtxn = self.env.read_txn()?;
        Ok(self.status_db.get(&rtxn, "current")?)
    }

    pub fn set_status(&self, status: StatusResult) -> Result<()> {
        let mut wtxn = self.env.write_txn()?;
        self.status_db.put(&mut wtxn, "current", &status)?;
        wtxn.commit()?;
        Ok(())
    }
}
