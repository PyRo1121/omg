//! System snapshot for transaction rollback

use std::collections::HashSet;

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemSnapshot {
    pub installed_packages: HashSet<String>,
}

impl SystemSnapshot {
    #[cfg(feature = "arch")]
    pub fn capture() -> Result<Self> {
        use crate::package_managers::alpm_direct;
        let packages = alpm_direct::list_installed_fast()?;
        let installed_packages: HashSet<String> = packages.into_iter().map(|p| p.name).collect();

        tracing::debug!(
            "Captured system snapshot: {} packages",
            installed_packages.len()
        );

        Ok(Self { installed_packages })
    }

    #[cfg(all(feature = "debian-pure", not(feature = "arch")))]
    pub fn capture() -> Result<Self> {
        use crate::package_managers::debian_db;
        let packages = debian_db::list_installed_fast()?;
        let installed_packages: HashSet<String> = packages.into_iter().map(|p| p.name).collect();

        tracing::debug!(
            "Captured system snapshot: {} packages",
            installed_packages.len()
        );

        Ok(Self { installed_packages })
    }

    #[cfg(not(any(feature = "arch", feature = "debian-pure")))]
    pub fn capture() -> Result<Self> {
        Ok(Self {
            installed_packages: HashSet::new(),
        })
    }

    pub fn was_installed(&self, package: &str) -> bool {
        self.installed_packages.contains(package)
    }
}

#[cfg(test)]
#[cfg(feature = "arch")]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_capture() {
        let snapshot = SystemSnapshot::capture();
        assert!(snapshot.is_ok());

        let snapshot = snapshot.unwrap();
        assert!(!snapshot.installed_packages.is_empty());
    }

    #[test]
    fn test_was_installed_check() {
        let snapshot = SystemSnapshot::capture().unwrap();
        assert!(snapshot.was_installed("pacman"));
        assert!(!snapshot.was_installed("nonexistent-package-xyz"));
    }
}
