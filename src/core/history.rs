use anyhow::{Context, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Maximum number of transactions to retain in history
const MAX_HISTORY_TRANSACTIONS: usize = 1000;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum TransactionType {
    Install,
    Remove,
    Update,
    Sync,
}

impl std::fmt::Display for TransactionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Install => write!(f, "Install"),
            Self::Remove => write!(f, "Remove"),
            Self::Update => write!(f, "Update"),
            Self::Sync => write!(f, "Sync"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageChange {
    pub name: String,
    pub old_version: Option<String>,
    pub new_version: Option<String>,
    pub source: String, // "official" or "aur"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transaction {
    pub id: String,
    pub timestamp: Timestamp,
    pub transaction_type: TransactionType,
    pub changes: Vec<PackageChange>,
    pub success: bool,
}

pub struct HistoryManager {
    log_path: PathBuf,
}

impl HistoryManager {
    pub fn new() -> Result<Self> {
        let log_dir = dirs::data_dir()
            .context("Unable to determine the user data directory for package history")?
            .join("omg");
        Self::new_in(log_dir.join("history.json"))
    }

    pub fn new_in(log_path: impl AsRef<Path>) -> Result<Self> {
        let log_path = log_path.as_ref().to_path_buf();
        let parent = log_path
            .parent()
            .context("Package history path must have a parent directory")?;
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create package history directory: {}",
                parent.display()
            )
        })?;
        Ok(Self { log_path })
    }

    pub fn load(&self) -> Result<Vec<Transaction>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.log_path)
            .with_context(|| format!("Failed to read history file: {}", self.log_path.display()))?;

        serde_json::from_str(&content)
            .with_context(|| format!("Malformed history file: {}", self.log_path.display()))
    }

    pub fn save(&self, history: &[Transaction]) -> Result<()> {
        let parent = self
            .log_path
            .parent()
            .context("Package history path must have a parent directory")?;
        let mut temporary = tempfile::NamedTempFile::new_in(parent)
            .context("Failed to create temporary history file")?;
        serde_json::to_writer_pretty(temporary.as_file_mut(), history)
            .context("Failed to serialize history")?;
        temporary
            .as_file_mut()
            .write_all(b"\n")
            .context("Failed to finalize history file")?;
        temporary
            .as_file_mut()
            .sync_all()
            .context("Failed to sync history file")?;
        temporary
            .persist(&self.log_path)
            .map_err(|error| error.error)
            .with_context(|| {
                format!(
                    "Failed to replace history file: {}",
                    self.log_path.display()
                )
            })?;
        Ok(())
    }

    pub fn add_transaction(
        &self,
        transaction_type: TransactionType,
        changes: Vec<PackageChange>,
        success: bool,
    ) -> Result<()> {
        let mut history = self.load()?;

        let transaction = Transaction {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Timestamp::now(),
            transaction_type,
            changes,
            success,
        };

        history.push(transaction);

        // Keep only last N transactions to prevent file bloat
        let excess = history.len().saturating_sub(MAX_HISTORY_TRANSACTIONS);
        if excess > 0 {
            history.drain(0..excess);
        }

        self.save(&history)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_history_is_rejected_without_data_loss() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("history.json");
        fs::write(&path, "not valid JSON")?;
        let manager = HistoryManager::new_in(&path)?;

        let error = manager
            .load()
            .expect_err("malformed persisted history must be rejected");

        assert!(error.to_string().contains("Malformed history file"));
        assert_eq!(fs::read_to_string(path)?, "not valid JSON");
        Ok(())
    }

    #[test]
    fn save_replaces_history_atomically_and_loads_it() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("history.json");
        let manager = HistoryManager::new_in(&path)?;
        let transaction = Transaction {
            id: "transaction-1".to_string(),
            timestamp: Timestamp::now(),
            transaction_type: TransactionType::Install,
            changes: vec![PackageChange {
                name: "example".to_string(),
                old_version: None,
                new_version: Some("1.0.0".to_string()),
                source: "official".to_string(),
            }],
            success: true,
        };

        manager.save(std::slice::from_ref(&transaction))?;
        let loaded = manager.load()?;

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, transaction.id);
        assert_eq!(loaded[0].changes[0].name, "example");
        assert!(loaded[0].success);
        Ok(())
    }
}
