//! Persistent metadata cache using redb (pure Rust)
//!
//! Stores system status metrics and package index to survive daemon restarts.
//! Index preloading enables <10ms cold start times.

use super::protocol::{DetailedPackageInfo, StatusResult};
use anyhow::{Context, Result};
use redb::{Database, ReadableDatabase, TableDefinition};
use std::path::Path;

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

const STATUS_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("status");
const INDEX_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("package_index");
const INDEX_META_TABLE: TableDefinition<&str, &[u8]> = TableDefinition::new("index_meta");

/// Serialized package index for fast loading
#[derive(Archive, RkyvSerialize, RkyvDeserialize, serde::Serialize, serde::Deserialize)]
pub struct SerializedIndex {
    pub packages: Vec<DetailedPackageInfo>,
    pub timestamp: u64,
}

/// Index metadata for cache invalidation
#[derive(serde::Serialize, serde::Deserialize)]
pub struct IndexMeta {
    pub package_count: usize,
    pub timestamp: u64,
    pub db_mtime: u64,
}

pub struct PersistentCache {
    db: Database,
}

impl PersistentCache {
    pub fn new(path: &Path) -> Result<Self> {
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

    pub fn get_status(&self) -> Result<Option<StatusResult>> {
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(STATUS_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(anyhow::anyhow!("Database error: {e}")),
        };

        match table.get("current")? {
            Some(guard) => {
                // Zero-copy access with validation
                let bytes = guard.value();
                let archived =
                    rkyv::access::<rkyv::Archived<StatusResult>, rkyv::rancor::Error>(bytes)
                        .map_err(|e| anyhow::anyhow!("Validation error: {e}"))?;

                let status: StatusResult =
                    rkyv::deserialize::<StatusResult, rkyv::rancor::Error>(archived)
                        .map_err(|e| anyhow::anyhow!("Deserialization error: {e}"))?;
                Ok(Some(status))
            }
            None => Ok(None),
        }
    }

    pub fn set_status(&self, status: &StatusResult) -> Result<()> {
        let data = rkyv::to_bytes::<rkyv::rancor::Error>(status)
            .map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;
        let write_txn = self.db.begin_write()?;
        {
            let mut table = write_txn.open_table(STATUS_TABLE)?;
            table.insert("current", data.as_slice())?;
        }
        write_txn.commit()?;
        Ok(())
    }

    /// Get cached package index metadata
    pub fn get_index_meta(&self) -> Result<Option<IndexMeta>> {
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(INDEX_META_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(anyhow::anyhow!("Database error: {e}")),
        };

        match table.get("meta")? {
            Some(guard) => {
                let meta: IndexMeta = bitcode::deserialize(guard.value())?;
                Ok(Some(meta))
            }
            None => Ok(None),
        }
    }

    /// Load cached package index (for instant startup)
    pub fn load_index(&self) -> Result<Option<SerializedIndex>> {
        let read_txn = self.db.begin_read()?;
        let table = match read_txn.open_table(INDEX_TABLE) {
            Ok(t) => t,
            Err(redb::TableError::TableDoesNotExist(_)) => return Ok(None),
            Err(e) => return Err(anyhow::anyhow!("Database error: {e}")),
        };

        match table.get("packages")? {
            Some(guard) => {
                let start = std::time::Instant::now();
                let bytes = guard.value();

                // Zero-copy access with validation
                let archived =
                    rkyv::access::<rkyv::Archived<SerializedIndex>, rkyv::rancor::Error>(bytes)
                        .map_err(|e| anyhow::anyhow!("Validation error: {e}"))?;

                let index: SerializedIndex =
                    rkyv::deserialize::<SerializedIndex, rkyv::rancor::Error>(archived)
                        .map_err(|e| anyhow::anyhow!("Deserialization error: {e}"))?;

                tracing::debug!(
                    "Loaded {} packages from cache in {:?}",
                    index.packages.len(),
                    start.elapsed()
                );
                Ok(Some(index))
            }
            None => Ok(None),
        }
    }

    /// Save package index to cache (for fast reload)
    pub fn save_index(&self, packages: &[DetailedPackageInfo], db_mtime: u64) -> Result<()> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_else(|e| {
                tracing::warn!("System clock error, using fallback timestamp: {e}");
                u64::MAX // Force cache miss on clock errors
            });

        let index = SerializedIndex {
            packages: packages.to_vec(),
            timestamp,
        };

        let index_data = rkyv::to_bytes::<rkyv::rancor::Error>(&index)
            .map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;

        let meta = IndexMeta {
            package_count: packages.len(),
            timestamp,
            db_mtime,
        };
        let meta_data = bitcode::serialize(&meta)?;

        let write_txn = self.db.begin_write()?;
        {
            let mut index_table = write_txn.open_table(INDEX_TABLE)?;
            index_table.insert("packages", index_data.as_slice())?;

            let mut meta_table = write_txn.open_table(INDEX_META_TABLE)?;
            meta_table.insert("meta", meta_data.as_slice())?;
        }
        write_txn.commit()?;

        tracing::info!(
            "Saved {} packages to index cache ({} bytes)",
            packages.len(),
            index_data.len()
        );
        Ok(())
    }

    /// Check if cached index is still valid
    pub fn is_index_valid(&self, current_db_mtime: u64) -> bool {
        match self.get_index_meta() {
            Ok(Some(meta)) => meta.db_mtime == current_db_mtime,
            _ => false,
        }
    }
}
