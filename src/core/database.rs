//! LMDB database wrapper for high-performance caching

use anyhow::Result;
use heed::{Database as HeedDatabase, Env, EnvOpenOptions};
use std::path::Path;

/// Main database wrapper using LMDB via heed
pub struct Database {
    env: Env,
}

impl Database {
    /// Open or create a database at the given path
    ///
    /// Uses 4GB mmap size as per architecture spec for optimal performance
    /// across all hardware configurations.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        std::fs::create_dir_all(&path)?;

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(4 * 1024 * 1024 * 1024) // 4GB mmap
                .max_readers(126)
                .max_dbs(16)
                .open(path)?
        };

        Ok(Self { env })
    }

    /// Get the completion database
    pub fn get_completion_db(&self) -> Result<HeedDatabase<heed::types::Str, heed::types::Str>> {
        let mut wtxn = self.env.write_txn()?;
        let db = self
            .env
            .create_database(&mut wtxn, Some("completion_cache"))?;
        wtxn.commit()?;
        Ok(db)
    }

    /// Get the underlying heed environment
    #[must_use]
    pub const fn env(&self) -> &Env {
        &self.env
    }

    /// Get the default database path
    pub fn default_path() -> Result<std::path::PathBuf> {
        let data_dir = directories::ProjectDirs::from("com", "omg", "omg")
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;

        Ok(data_dir.data_dir().join("db"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_database_open() {
        let temp_dir = TempDir::new().unwrap();
        let db = Database::open(temp_dir.path().join("test.db"));
        assert!(db.is_ok());
    }
}
