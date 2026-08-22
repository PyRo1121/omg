//! Persistent metadata cache using redb (pure Rust)
//!
//! Stores the system status snapshot so it survives daemon restarts.

use anyhow::{Context, Result};
use redb::{Database, ReadableDatabase, TableDefinition};
use std::path::Path;

use super::protocol::StatusResult;

const STATUS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("status");

pub(crate) struct PersistentCache {
    db: Database,
}

impl PersistentCache {
    pub(crate) fn new(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path)?;
        let db_path = path.join("cache.redb");

        let db = Database::create(&db_path).with_context(|| {
            format!(
                "Failed to open redb database at {}. \
                 This usually means another daemon instance is already running. \
                 Try: killall omgd && rm -f {}",
                db_path.display(),
                db_path.display()
            )
        })?;

        Ok(Self { db })
    }

    pub(crate) fn get_status(&self) -> Result<Option<StatusResult>> {
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(STATUS_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(anyhow::Error::new(e).context("failed to open status table")),
        };

        match table.get("current")? {
            Some(guard) => {
                // Zero-copy access with validation
                let bytes = guard.value();
                let archived =
                    rkyv::access::<rkyv::Archived<StatusResult>, rkyv::rancor::Error>(bytes)
                        .context("cached status failed validation")?;

                let status: StatusResult =
                    rkyv::deserialize::<StatusResult, rkyv::rancor::Error>(archived)
                        .context("failed to deserialize cached status")?;
                Ok(Some(status))
            }
            None => Ok(None),
        }
    }

    pub(crate) fn set_status(&self, status: &StatusResult) -> Result<()> {
        let data = rkyv::to_bytes::<rkyv::rancor::Error>(status)
            .context("failed to serialize status for persistence")?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(STATUS_TABLE)?;
            table.insert("current", data.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }
}
