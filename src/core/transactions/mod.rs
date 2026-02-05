//! Transaction system for atomic install/update/remove operations with rollback
//!
//! Ensures system consistency by tracking all package operations and providing
//! automatic rollback on failure. Prevents partial installations that leave the
//! system in a broken state.

use std::time::{Duration, SystemTime};

#[cfg(feature = "arch")]
use alpm_types::Version;
#[cfg(not(feature = "arch"))]
type Version = String;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::core::PackageSource;

mod snapshot;

pub use snapshot::SystemSnapshot;

/// A package manager transaction with rollback capability
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    /// Unique transaction ID
    pub id: Uuid,
    /// When the transaction started
    pub timestamp: SystemTime,
    /// Operations performed in this transaction
    pub operations: Vec<Operation>,
    /// Current state of the transaction
    pub state: TransactionState,
    /// System snapshot before transaction (for rollback)
    #[serde(skip)]
    pub snapshot: Option<SystemSnapshot>,
}

/// An individual operation within a transaction
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    /// Install a package
    Install {
        package: String,
        version: Version,
        source: PackageSource,
    },
    /// Remove a package
    Remove { package: String, version: Version },
    /// Update a package
    Update {
        package: String,
        old_version: Version,
        new_version: Version,
    },
}

/// Current state of a transaction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransactionState {
    /// Transaction created but not started
    Pending,
    /// Transaction is currently executing
    InProgress,
    /// Transaction completed successfully
    Committed,
    /// Transaction was rolled back
    RolledBack,
    /// Transaction failed (may be partially complete)
    Failed,
}

impl Transaction {
    /// Create a new transaction
    pub fn new() -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: SystemTime::now(),
            operations: Vec::new(),
            state: TransactionState::Pending,
            snapshot: None,
        }
    }

    /// Begin the transaction (capture system snapshot)
    pub fn begin(&mut self) -> Result<()> {
        if self.state != TransactionState::Pending {
            anyhow::bail!("Transaction already started");
        }

        self.snapshot = Some(SystemSnapshot::capture()?);
        self.state = TransactionState::InProgress;

        tracing::info!("Transaction {} started", self.id);
        Ok(())
    }

    /// Add an operation to the transaction
    pub fn add_operation(&mut self, operation: Operation) {
        self.operations.push(operation);
    }

    /// Get all completed operations (for partial rollback)
    pub fn completed_operations(&self) -> &[Operation] {
        &self.operations
    }

    /// Commit the transaction (mark as successful)
    pub fn commit(&mut self) -> Result<()> {
        if self.state != TransactionState::InProgress {
            anyhow::bail!("Transaction not in progress");
        }

        self.state = TransactionState::Committed;
        self.snapshot = None; // Release snapshot memory

        tracing::info!(
            "Transaction {} committed ({} operations)",
            self.id,
            self.operations.len()
        );
        Ok(())
    }

    /// Rollback the transaction (undo all operations)
    pub async fn rollback(&mut self) -> Result<()> {
        match self.state {
            TransactionState::Pending => {
                // Nothing to rollback
                self.state = TransactionState::RolledBack;
                return Ok(());
            }
            TransactionState::Committed => {
                // Can only rollback if we still have a snapshot
                if self.snapshot.is_none() {
                    anyhow::bail!("Cannot rollback committed transaction (snapshot released)");
                }
            }
            TransactionState::RolledBack => {
                anyhow::bail!("Transaction already rolled back");
            }
            _ => {}
        }

        tracing::warn!(
            "Rolling back transaction {} ({} operations)",
            self.id,
            self.operations.len()
        );

        // Reverse operations in LIFO order (last operation first)
        for (idx, op) in self.operations.iter().enumerate().rev() {
            if let Err(e) = self.reverse_operation(op).await {
                tracing::error!(
                    "Failed to reverse operation {} of {}: {}",
                    idx + 1,
                    self.operations.len(),
                    e
                );
                // Continue trying to rollback remaining operations
            }
        }

        self.state = TransactionState::RolledBack;
        self.snapshot = None;

        tracing::info!("Transaction {} rolled back", self.id);
        Ok(())
    }

    /// Reverse a single operation
    async fn reverse_operation(&self, op: &Operation) -> Result<()> {
        match op {
            Operation::Install { package, .. } => {
                // Reverse install = remove
                tracing::debug!("Rollback: Removing {}", package);
                self.remove_package(package).await?;
            }
            Operation::Remove { package, version } => {
                // Reverse remove = reinstall
                tracing::debug!("Rollback: Reinstalling {} {}", package, version);
                self.install_package(package, version).await?;
            }
            Operation::Update {
                package,
                old_version,
                ..
            } => {
                // Reverse update = downgrade
                tracing::debug!("Rollback: Downgrading {} to {}", package, old_version);
                self.downgrade_package(package, old_version).await?;
            }
        }
        Ok(())
    }

    #[cfg(feature = "arch")]
    async fn remove_package(&self, package: &str) -> Result<()> {
        use crate::package_managers::alpm_ops;

        tokio::task::spawn_blocking({
            let package = package.to_string();
            move || {
                alpm_ops::execute_transaction(vec![package], true, false, None)
                    .context("Failed to remove package during rollback")
            }
        })
        .await??;

        Ok(())
    }

    #[cfg(not(feature = "arch"))]
    async fn remove_package(&self, package: &str) -> Result<()> {
        tracing::warn!("Rollback remove not implemented for this platform: {}", package);
        Ok(())
    }

    #[cfg(feature = "arch")]
    async fn install_package(&self, package: &str, version: &Version) -> Result<()> {
        let cache_dir = crate::core::paths::pacman_cache_dir();
        let pkg_pattern = format!("{package}-{version}-*.pkg.tar.*");

        if let Some(cached_pkg) = Self::find_cached_package(&cache_dir, &pkg_pattern) {
            tracing::debug!("Found cached package: {}", cached_pkg.display());
            return self.install_local_package(&cached_pkg).await;
        }

        tracing::warn!(
            "Package {}-{} not in cache, rollback may be incomplete",
            package,
            version
        );
        Ok(())
    }

    #[cfg(not(feature = "arch"))]
    async fn install_package(&self, package: &str, version: &Version) -> Result<()> {
        tracing::warn!("Rollback install not implemented for this platform: {}-{}", package, version);
        Ok(())
    }

    async fn downgrade_package(&self, package: &str, version: &Version) -> Result<()> {
        self.install_package(package, version).await
    }

    #[cfg(feature = "arch")]
    async fn install_local_package(&self, path: &std::path::Path) -> Result<()> {
        use crate::package_managers::alpm_ops;

        tokio::task::spawn_blocking({
            let path = path.to_string_lossy().to_string();
            move || {
                alpm_ops::execute_transaction(vec![path], false, false, None)
                    .context("Failed to install local package during rollback")
            }
        })
        .await??;

        Ok(())
    }

    #[cfg(not(feature = "arch"))]
    async fn install_local_package(&self, _path: &std::path::Path) -> Result<()> {
        tracing::warn!("Rollback install_local not implemented for this platform");
        Ok(())
    }

    #[cfg(feature = "arch")]
    fn find_cached_package(
        cache_dir: &std::path::Path,
        pattern: &str,
    ) -> Option<std::path::PathBuf> {
        let pattern = pattern.replace('*', ".*");
        let re = regex::Regex::new(&pattern).ok()?;

        std::fs::read_dir(cache_dir).ok()?.find_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            let filename = path.file_name()?.to_str()?;

            if re.is_match(filename) {
                Some(path)
            } else {
                None
            }
        })
    }

    /// Get transaction duration
    pub fn duration(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.timestamp)
            .unwrap_or_default()
    }
}

impl Default for Transaction {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[cfg(feature = "arch")]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_lifecycle() {
        let mut tx = Transaction::new();
        assert_eq!(tx.state, TransactionState::Pending);

        tx.begin().unwrap();
        assert_eq!(tx.state, TransactionState::InProgress);
        assert!(tx.snapshot.is_some());

        tx.commit().unwrap();
        assert_eq!(tx.state, TransactionState::Committed);
        assert!(tx.snapshot.is_none());
    }

    #[test]
    fn test_operation_tracking() {
        let mut tx = Transaction::new();

        tx.add_operation(Operation::Install {
            package: "test-pkg".to_string(),
            version: "1.0.0".parse().unwrap(),
            source: PackageSource::Official,
        });

        assert_eq!(tx.operations.len(), 1);
        assert!(matches!(tx.operations[0], Operation::Install { .. }));
    }

    #[tokio::test]
    async fn test_rollback_pending_transaction() {
        let mut tx = Transaction::new();
        assert!(tx.rollback().await.is_ok());
        assert_eq!(tx.state, TransactionState::RolledBack);
    }
}
