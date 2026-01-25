//! Pure Rust embedded database wrapper using redb

use anyhow::Result;
use redb::{Database as RedbDatabase, ReadableDatabase, TableDefinition};
use std::path::Path;
use std::sync::Arc;

use crate::core::paths;

/// Table definition for completion cache
const COMPLETION_TABLE: TableDefinition<&str, &str> = TableDefinition::new("completion_cache");

/// Main database wrapper using redb (pure Rust)
pub struct Database {
    db: Arc<RedbDatabase>,
}

impl Database {
    /// Open or create a database at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let db = RedbDatabase::create(path)?;
        Ok(Self { db: Arc::new(db) })
    }

    /// Get a value from the completion database
    pub fn get_completion(&self, key: &str) -> Result<Option<String>> {
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(COMPLETION_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(anyhow::anyhow!("Database error: {e}")),
        };
        Ok(table.get(key)?.map(|guard| guard.value().to_string()))
    }

    /// Set a value in the completion database
    pub fn set_completion(&self, key: &str, value: &str) -> Result<()> {
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(COMPLETION_TABLE)?;
            table.insert(key, value)?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get the underlying redb database
    #[must_use]
    pub fn inner(&self) -> &RedbDatabase {
        &self.db
    }

    /// Get the default database path
    pub fn default_path() -> Result<std::path::PathBuf> {
        Ok(paths::data_dir().join("omg.redb"))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_open() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("test.redb"));
        assert!(db.is_ok());
    }
}
