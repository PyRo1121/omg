//! Package transaction history
//!
//! Records package install/remove/update/sync outcomes to a single JSON
//! file under the data directory. Writes are atomic (temp file + rename)
//! and serialized across processes through a sibling `.lock` file so
//! concurrent omg invocations cannot drop each other's transactions.

use anyhow::{Context, Result};
use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Once-per-process gate shared by `warn_corrupt_history_once` and its tests.
static CORRUPT_HISTORY_WARNED: AtomicBool = AtomicBool::new(false);

/// Maximum number of transactions to retain in history
const MAX_HISTORY_TRANSACTIONS: usize = 1000;

/// Kind of package operation recorded in the history.
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

/// One package affected by a transaction.
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

/// A completed transaction: what changed, when, and whether it succeeded.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Transaction {
    pub id: String,
    pub timestamp: Timestamp,
    pub transaction_type: TransactionType,
    pub changes: Vec<PackageChange>,
    pub success: bool,
}

/// Loads and appends package transactions under a cross-process file lock.
///
/// The log file is capped at [`MAX_HISTORY_TRANSACTIONS`] entries; every
/// write atomically replaces the file via a temporary file and rename.
pub struct HistoryManager {
    log_path: PathBuf,
}

impl HistoryManager {
    /// Creates a manager backed by the default history file in the data
    /// directory.
    pub fn new() -> Result<Self> {
        Self::new_in(crate::core::paths::data_dir().join("history.json"))
    }

    /// Creates a manager backed by an explicit history file path (tests,
    /// alternative stores).
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

    /// Collect deduplicated `(package, version)` pairs whose `old_version`
    /// appears in successful Remove/Update transactions within the last
    /// `days` days. Used to warn before cache cleaning destroys the archives
    /// those rollback plans depend on.
    pub fn rollback_referenced_versions(&self, days: i64) -> Result<Vec<(String, String)>> {
        let entries = self.load()?;
        let cutoff = Timestamp::now()
            .as_second()
            .saturating_sub(days.saturating_mul(24 * 60 * 60));

        let mut referenced = std::collections::BTreeSet::new();
        for entry in entries {
            if !entry.success {
                continue;
            }
            if !matches!(
                entry.transaction_type,
                TransactionType::Remove | TransactionType::Update
            ) {
                continue;
            }
            if entry.timestamp.as_second() < cutoff {
                continue;
            }
            for change in &entry.changes {
                if let Some(version) = &change.old_version {
                    referenced.insert((change.name.clone(), version.clone()));
                }
            }
        }

        Ok(referenced.into_iter().collect())
    }

    /// Loads every recorded transaction. A missing file is an empty history;
    /// a malformed file is quarantined (renamed, never deleted) and replaced
    /// with a fresh empty history so one corrupt file cannot wedge every
    /// future package operation behind a persistent persistence failure.
    pub fn load(&self) -> Result<Vec<Transaction>> {
        if !self.log_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&self.log_path)
            .with_context(|| format!("Failed to read history file: {}", self.log_path.display()))?;

        match serde_json::from_str(&content) {
            Ok(history) => Ok(history),
            Err(error) => {
                let quarantined = self.quarantine_corrupt_file(&error)?;
                warn_corrupt_history_once(&self.log_path, &quarantined, &error);
                Ok(Vec::new())
            }
        }
    }

    /// Renames a malformed history file to `<name>.corrupt-<timestamp>` so it
    /// is preserved for manual recovery, then leaves the original path free
    /// for a fresh history. Never deletes the file: if the rename fails the
    /// error propagates instead of losing the user's transaction log.
    fn quarantine_corrupt_file(&self, parse_error: &serde_json::Error) -> Result<PathBuf> {
        let parent = self
            .log_path
            .parent()
            .context("Package history path must have a parent directory")?;
        let file_name = self
            .log_path
            .file_name()
            .context("Package history path must have a file name")?
            .to_string_lossy()
            .into_owned();
        let stamp = Timestamp::now()
            .strftime("%Y%m%dT%H%M%S%.6fZ")
            .to_string();

        let mut quarantined = parent.join(format!("{file_name}.corrupt-{stamp}"));
        let mut counter = 1u32;
        while quarantined.exists() {
            quarantined = parent.join(format!("{file_name}.corrupt-{stamp}-{counter}"));
            counter += 1;
        }

        fs::rename(&self.log_path, &quarantined).with_context(|| {
            format!(
                "Malformed history file {} ({parse_error}); quarantining it to {} also failed",
                self.log_path.display(),
                quarantined.display()
            )
        })?;
        Ok(quarantined)
    }

    /// Atomically replaces the whole history with `history`.
    pub fn save(&self, history: &[Transaction]) -> Result<()> {
        let mut content =
            serde_json::to_vec_pretty(history).context("Failed to serialize history")?;
        content.push(b'\n');
        crate::core::safe_ops::atomic_write_file_sync(&self.log_path, content)
    }

    /// Persist the outcome of a package mutation without hiding either the
    /// operation error or a history persistence failure.
    pub fn finish_operation(
        &self,
        transaction_type: TransactionType,
        changes: Vec<PackageChange>,
        operation_result: Result<()>,
    ) -> Result<()> {
        if crate::core::privilege::parent_owns_history() {
            return operation_result;
        }
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

    /// Appends one transaction, holding the cross-process lock for the
    /// full load-modify-save cycle.
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

/// Warns the user exactly once per process that their history file was
/// malformed and where the quarantined copy now lives. Mirrors the
/// once-per-process warning gate in `config::settings`. Returns whether the
/// warning was emitted (i.e. this was the first corrupt load this process).
fn warn_corrupt_history_once(
    original: &Path,
    quarantined: &Path,
    parse_error: &serde_json::Error,
) -> bool {
    if CORRUPT_HISTORY_WARNED.swap(true, Ordering::Relaxed) {
        return false;
    }
    tracing::warn!(
        original = %original.display(),
        quarantined = %quarantined.display(),
        "Package history file was malformed (parse error: {parse_error}); it has been quarantined to {} and a fresh history has been started",
        quarantined.display()
    );
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Resets the process-wide corrupt-history warning gate so warning
    /// assertions are deterministic (paired with `#[serial(corrupt_history)]`).
    fn reset_corrupt_history_warning_for_tests() {
        CORRUPT_HISTORY_WARNED.store(false, Ordering::Relaxed);
    }

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
    #[serial_test::serial(history_ownership)]
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
    #[serial_test::serial(history_ownership)]
    fn elevated_child_skips_history_owned_by_its_parent() -> Result<()> {
        struct OwnershipReset;
        impl Drop for OwnershipReset {
            fn drop(&mut self) {
                crate::core::privilege::set_parent_owns_history(false);
            }
        }

        let _reset = OwnershipReset;
        crate::core::privilege::set_parent_owns_history(true);
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("history.json");
        let manager = HistoryManager::new_in(&path)?;

        manager.finish_operation(
            TransactionType::Remove,
            vec![PackageChange {
                name: "example".to_string(),
                old_version: Some("1.0".to_string()),
                new_version: None,
                source: "official".to_string(),
            }],
            Ok(()),
        )?;

        assert!(!path.exists(), "elevated child must not duplicate history");
        Ok(())
    }

    #[test]
    #[serial_test::serial(corrupt_history)]
    fn malformed_history_is_quarantined_and_history_starts_fresh() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("history.json");
        fs::write(&path, "not valid JSON")?;
        let manager = HistoryManager::new_in(&path)?;

        let history = manager.load()?;

        assert!(history.is_empty(), "a quarantined history starts fresh");
        let corrupt_names: Vec<String> = fs::read_dir(directory.path())?
            .filter_map(std::result::Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("history.json.corrupt-"))
            .collect();
        assert_eq!(corrupt_names.len(), 1, "exactly one quarantined copy");
        assert_eq!(
            fs::read_to_string(directory.path().join(&corrupt_names[0]))?,
            "not valid JSON",
            "quarantine must preserve the original bytes"
        );
        assert!(!path.exists(), "original path is free for a fresh history");

        // The warning fires once per process and names the quarantined file.
        // The once-flag is process-wide, so serialise against other tests
        // that corrupt-load history and reset it for deterministic asserts.
        reset_corrupt_history_warning_for_tests();
        let error =
            serde_json::from_str::<Vec<Transaction>>("not valid JSON").unwrap_err();
        assert!(
            warn_corrupt_history_once(&path, Path::new(&corrupt_names[0]), &error),
            "first call warns"
        );
        assert!(
            !warn_corrupt_history_once(&path, Path::new(&corrupt_names[0]), &error),
            "second call is deduplicated"
        );
        Ok(())
    }

    #[test]
    #[serial_test::serial(history_ownership)]
    fn corrupt_history_no_longer_wedges_finish_operation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("history.json");
        fs::write(&path, "[not valid JSON")?;
        let manager = HistoryManager::new_in(&path)?;

        manager
            .finish_operation(
                TransactionType::Install,
                vec![PackageChange {
                    name: "example".to_string(),
                    old_version: None,
                    new_version: Some("1.0".to_string()),
                    source: "official".to_string(),
                }],
                Ok(()),
            )
            .expect("a corrupt history file must not fail a successful operation");

        let history = manager.load()?;
        assert_eq!(history.len(), 1);
        assert!(history[0].success);
        let corrupt_exists = fs::read_dir(directory.path())?
            .filter_map(std::result::Result::ok)
            .any(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("history.json.corrupt-")
            });
        assert!(corrupt_exists, "the malformed file is preserved, not deleted");
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
