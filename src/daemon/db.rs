//! Persistent metadata cache for the daemon.
//!
//! Stores the system status snapshot as a single atomically-replaced JSON
//! file so it survives daemon restarts. A single-key snapshot does not
//! justify an embedded transactional database; [`crate::core::safe_ops`]
//! provides the same crash-safety (no truncated files, owner-only mode).

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::protocol::StatusResult;

/// Current on-disk status snapshot format.
const STATUS_FORMAT_VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct PersistedStatus {
    format_version: u32,
    status: StatusResult,
}

pub(crate) struct PersistentCache {
    path: PathBuf,
}

impl PersistentCache {
    pub(crate) fn new(dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(dir)?;
        Ok(Self {
            path: dir.join("status-cache.json"),
        })
    }

    pub(crate) fn get_status(&self) -> Result<Option<StatusResult>> {
        let content = match std::fs::read_to_string(&self.path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).context(format!(
                    "Failed to read daemon status cache {}",
                    self.path.display()
                ));
            }
        };
        let persisted: PersistedStatus = serde_json::from_str(&content)
            .with_context(|| format!("Malformed daemon status cache: {}", self.path.display()))?;
        anyhow::ensure!(
            persisted.format_version == STATUS_FORMAT_VERSION,
            "Unsupported daemon status cache format version {} (expected {})",
            persisted.format_version,
            STATUS_FORMAT_VERSION
        );
        Ok(Some(persisted.status))
    }

    pub(crate) fn set_status(&self, status: &StatusResult) -> Result<()> {
        let persisted = PersistedStatus {
            format_version: STATUS_FORMAT_VERSION,
            status: status.clone(),
        };
        let content =
            serde_json::to_vec(&persisted).context("Failed to serialize daemon status cache")?;
        crate::core::safe_ops::atomic_write_file_sync(&self.path, content)
            .with_context(|| format!("Failed to write {}", self.path.display()))
    }
}
