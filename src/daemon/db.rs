//! Persistent metadata cache using redb (pure Rust)
//!
//! Stores system status metrics to survive daemon restarts.

use super::protocol::StatusResult;
use anyhow::{Context, Result};
use redb::{Database, ReadableDatabase, TableDefinition};
use std::path::Path;

const STATUS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("status");

pub struct PersistentCache {
    db: Database,
}

impl PersistentCache {
    pub fn new(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;
        let db_path = path.join("cache.redb");

        let db = Database::create(&db_path).context("Failed to open redb database")?;

        Ok(Self { db })
    }

    pub fn get_status(&self) -> Result<Option<StatusResult>> {
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(STATUS_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(anyhow::anyhow!("Database error: {e}")),
        };

        match table.get("current")? {
            Some(guard) => {
                let status: StatusResult = serde_json::from_slice(guard.value())?;
                Ok(Some(status))
            }
            None => Ok(None),
        }
    }

    pub fn set_status(&self, status: &StatusResult) -> Result<()> {
        let data = serde_json::to_vec(status)?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(STATUS_TABLE)?;
            table.insert("current", data.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }
}
