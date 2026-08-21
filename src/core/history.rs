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
    pub source: String,
}

impl PackageChange {
    /// Whether the system package manager can restore this change.
    /// Official repository names vary (`core`, `extra`, `apt`, `pacman`), so
    /// only sources that require a separate installation path are excluded.
    #[must_use]
    pub fn is_official_source(&self) -> bool {
        !self.source.eq_ignore_ascii_case("aur") && !self.source.eq_ignore_ascii_case("local")
    }
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
        Self::new_in(crate::core::paths::data_dir().join("history.json"))
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

    /// Persist the outcome of a package mutation without hiding either the
    /// operation error or a history persistence failure.
    pub fn finish_operation(
        &self,
        transaction_type: TransactionType,
        changes: Vec<PackageChange>,
        operation_result: Result<()>,
    ) -> Result<()> {
        let history_result = self
            .add_transaction(transaction_type, changes, operation_result.is_ok())
            .context("Failed to persist package operation history");

        match (operation_result, history_result) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(operation_error), Ok(())) => Err(operation_error),
            (Ok(()), Err(history_error)) => Err(history_error
                .context("Package operation succeeded but its history could not be persisted")),
            (Err(operation_error), Err(history_error)) => Err(anyhow::anyhow!(
                "Package operation failed: {operation_error}; history persistence also failed: {history_error}"
            )),
        }
    }

    pub fn add_transaction(
        &self,
        transaction_type: TransactionType,
        changes: Vec<PackageChange>,
        success: bool,
    ) -> Result<()> {
        let lock_path = self.log_path.with_extension("lock");
        let lock = fs::OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .with_context(|| format!("Failed to open history lock: {}", lock_path.display()))?;
        lock.lock()
            .with_context(|| format!("Failed to lock package history: {}", lock_path.display()))?;

        let result = self.add_transaction_locked(transaction_type, changes, success);
        if let Err(error) = lock.unlock() {
            return match result {
                Ok(()) => Err(error).context("Failed to unlock package history"),
                Err(operation_error) => Err(operation_error.context(format!(
                    "Package history update also failed to unlock its lock: {error}"
                ))),
            };
        }
        result
    }

    fn add_transaction_locked(
        &self,
        transaction_type: TransactionType,
        changes: Vec<PackageChange>,
        success: bool,
    ) -> Result<()> {
        let mut history = self.load()?;
        history.push(Transaction {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Timestamp::now(),
            transaction_type,
            changes,
            success,
        });

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
    fn official_repository_names_are_restorable() {
        for source in ["official", "core", "extra", "apt", "pacman"] {
            let change = PackageChange {
                name: "example".to_string(),
                old_version: Some("1.0".to_string()),
                new_version: Some("2.0".to_string()),
                source: source.to_string(),
            };
            assert!(change.is_official_source(), "source {source}");
        }
        for source in ["aur", "AUR", "local"] {
            let change = PackageChange {
                name: "example".to_string(),
                old_version: Some("1.0".to_string()),
                new_version: Some("2.0".to_string()),
                source: source.to_string(),
            };
            assert!(!change.is_official_source(), "source {source}");
        }
    }

    #[test]
    fn finish_operation_persists_failed_mutations_and_returns_the_operation_error() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let manager = HistoryManager::new_in(directory.path().join("history.json"))?;

        let error = manager
            .finish_operation(
                TransactionType::Install,
                vec![PackageChange {
                    name: "example".to_string(),
                    old_version: None,
                    new_version: Some("1.0".to_string()),
                    source: "official".to_string(),
                }],
                Err(anyhow::anyhow!("backend failed")),
            )
            .expect_err("operation failure must be returned");

        assert!(error.to_string().contains("backend failed"));
        let history = manager.load()?;
        assert_eq!(history.len(), 1);
        assert!(!history[0].success);
        Ok(())
    }

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
    fn concurrent_transactions_are_not_lost() -> Result<()> {
        use std::sync::{Arc, Barrier};

        const WRITERS: usize = 8;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("history.json");
        let barrier = Arc::new(Barrier::new(WRITERS));
        let mut writers = Vec::new();
        for index in 0..WRITERS {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            writers.push(std::thread::spawn(move || -> Result<()> {
                let manager = HistoryManager::new_in(path)?;
                barrier.wait();
                manager.add_transaction(
                    TransactionType::Install,
                    vec![PackageChange {
                        name: format!("package-{index}"),
                        old_version: None,
                        new_version: Some("1.0.0".to_string()),
                        source: "test".to_string(),
                    }],
                    true,
                )
            }));
        }
        for writer in writers {
            writer.join().expect("history writer panicked")?;
        }

        let history = HistoryManager::new_in(path)?.load()?;
        assert_eq!(history.len(), WRITERS);
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
