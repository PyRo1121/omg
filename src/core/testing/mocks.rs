//! Mock implementations for testing

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::Result;
use std::sync::Mutex;

use crate::core::{Package, PackageSource};
use crate::package_managers::parse_version_or_zero;
use crate::package_managers::{PackageManager, types::UpdateInfo};

/// Mock package manager with configurable behavior
pub struct TestPackageManager {
    packages: Arc<Mutex<HashMap<String, Package>>>,
    installed: Arc<Mutex<std::collections::HashSet<String>>>,
    updates: Arc<Mutex<Vec<UpdateInfo>>>,
    fail_operations: Arc<Mutex<bool>>,
    search_delay_ms: u64,
}

impl TestPackageManager {
    /// Create a new test package manager
    pub fn new() -> Self {
        Self {
            packages: Arc::new(Mutex::new(HashMap::new())),
            installed: Arc::new(Mutex::new(std::collections::HashSet::new())),
            updates: Arc::new(Mutex::new(Vec::new())),
            fail_operations: Arc::new(Mutex::new(false)),
            search_delay_ms: 0,
        }
    }

    /// Add a package to the mock database
    pub fn add_package(&self, name: &str, version: &str, description: &str) {
        let mut packages = self.packages.lock().expect("lock poisoned");
        packages.insert(
            name.to_string(),
            Package {
                name: name.to_string(),
                version: parse_version_or_zero(version),
                description: description.to_string(),
                source: PackageSource::Official,
                installed: self
                    .installed
                    .lock()
                    .expect("lock poisoned")
                    .contains(&name.to_string()),
            },
        );
    }

    /// Mark a package as installed
    pub fn install_package(&self, name: &str) {
        self.installed
            .lock()
            .expect("lock poisoned")
            .insert(name.to_string());
        // Update the package in the database
        if let Some(pkg) = self.packages.lock().expect("lock poisoned").get_mut(name) {
            pkg.installed = true;
        }
    }

    /// Remove a package (mark as not installed)
    pub fn remove_package(&self, name: &str) {
        self.installed
            .lock()
            .expect("lock poisoned")
            .remove(&name.to_string());
        if let Some(pkg) = self.packages.lock().expect("lock poisoned").get_mut(name) {
            pkg.installed = false;
        }
    }

    /// Set available updates
    pub fn set_updates(&self, updates: Vec<UpdateInfo>) {
        *self.updates.lock().expect("lock poisoned") = updates;
    }

    /// Configure whether operations should fail
    pub fn set_fail_operations(&self, fail: bool) {
        *self.fail_operations.lock().expect("lock poisoned") = fail;
    }

    /// Set artificial delay for operations (useful for testing async behavior)
    pub fn set_search_delay(&mut self, delay_ms: u64) {
        self.search_delay_ms = delay_ms;
    }

    /// Create with common test packages
    pub fn with_defaults() -> Self {
        let pm = Self::new();
        pm.add_package("firefox", "122.0-1", "Web browser");
        pm.add_package("git", "2.43.0-1", "Version control");
        pm.add_package("pacman", "6.0.2-1", "Package manager");
        pm.add_package("vim", "9.0.0-1", "Text editor");
        pm.install_package("pacman");
        pm.install_package("git");
        pm
    }

    /// Create with update scenario
    pub fn with_updates() -> Self {
        let pm = Self::with_defaults();
        pm.set_updates(vec![
            UpdateInfo {
                name: "firefox".to_string(),
                old_version: "121.0-1".to_string(),
                new_version: "122.0-1".to_string(),
                repo: "extra".to_string(),
            },
            UpdateInfo {
                name: "vim".to_string(),
                old_version: "8.0-1".to_string(),
                new_version: "9.0.0-1".to_string(),
                repo: "extra".to_string(),
            },
        ]);
        pm
    }

    /// Helper to check if a package is in the database
    pub fn has_package(&self, name: &str) -> bool {
        self.packages
            .lock()
            .expect("lock poisoned")
            .contains_key(name)
    }

    /// Helper to get the number of packages in the database
    pub fn package_count(&self) -> usize {
        self.packages.lock().expect("lock poisoned").len()
    }
}

impl Default for TestPackageManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManager for TestPackageManager {
    fn name(&self) -> &'static str {
        "test-mock"
    }

    fn search(
        &self,
        query: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        let query = query.to_lowercase();
        let delay = self.search_delay_ms;
        Box::pin(async move {
            if delay > 0 {
                tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
            }

            let fail = *self.fail_operations.lock().expect("lock poisoned");
            if fail {
                anyhow::bail!("Search operation failed (test failure mode)");
            }

            let pkgs = self.packages.lock().expect("lock poisoned");
            Ok(pkgs
                .values()
                .filter(|p| {
                    p.name.to_lowercase().contains(&query)
                        || p.description.to_lowercase().contains(&query)
                })
                .cloned()
                .collect())
        })
    }

    fn install(
        &self,
        packages: &[String],
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
            let fail = *self.fail_operations.lock().expect("lock poisoned");
            if fail {
                anyhow::bail!("Install operation failed (test failure mode)");
            }

            for pkg in packages {
                self.installed.lock().expect("lock poisoned").insert(pkg);
            }
            Ok(())
        })
    }

    fn remove(&self, packages: &[String]) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
            let fail = *self.fail_operations.lock().expect("lock poisoned");
            if fail {
                anyhow::bail!("Remove operation failed (test failure mode)");
            }

            for pkg in packages {
                self.installed.lock().expect("lock poisoned").remove(&pkg);
            }
            Ok(())
        })
    }

    fn update(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            let fail = *self.fail_operations.lock().expect("lock poisoned");
            if fail {
                anyhow::bail!("Update operation failed (test failure mode)");
            }
            Ok(())
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            let fail = *self.fail_operations.lock().expect("lock poisoned");
            if fail {
                anyhow::bail!("Sync operation failed (test failure mode)");
            }
            Ok(())
        })
    }

    fn info(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Package>>> + Send + '_>> {
        let package = package.to_string();
        Box::pin(async move {
            let fail = *self.fail_operations.lock().expect("lock poisoned");
            if fail {
                anyhow::bail!("Info operation failed (test failure mode)");
            }

            Ok(self
                .packages
                .lock()
                .expect("lock poisoned")
                .get(&package)
                .cloned())
        })
    }

    fn list_installed(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        Box::pin(async move {
            let fail = *self.fail_operations.lock().expect("lock poisoned");
            if fail {
                anyhow::bail!("List installed operation failed (test failure mode)");
            }

            let installed_set = self.installed.lock().expect("lock poisoned");
            let pkgs = self.packages.lock().expect("lock poisoned");
            Ok(pkgs
                .values()
                .filter(|p| installed_set.contains(&p.name))
                .cloned()
                .collect())
        })
    }

    fn get_status(
        &self,
        _fast: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(usize, usize, usize, usize)>> + Send + '_>> {
        Box::pin(async move {
            let fail = *self.fail_operations.lock().expect("lock poisoned");
            if fail {
                anyhow::bail!("Get status operation failed (test failure mode)");
            }

            let total = self.packages.lock().expect("lock poisoned").len();
            let explicit = self.installed.lock().expect("lock poisoned").len();
            let updates_count = self.updates.lock().expect("lock poisoned").len();
            Ok((total, explicit, 0, updates_count))
        })
    }

    fn list_explicit(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            let fail = *self.fail_operations.lock().expect("lock poisoned");
            if fail {
                anyhow::bail!("List explicit operation failed (test failure mode)");
            }

            Ok(self
                .installed
                .lock()
                .expect("lock poisoned")
                .iter()
                .cloned()
                .collect())
        })
    }

    fn list_updates(&self) -> Pin<Box<dyn Future<Output = Result<Vec<UpdateInfo>>> + Send + '_>> {
        Box::pin(async move {
            let fail = *self.fail_operations.lock().expect("lock poisoned");
            if fail {
                anyhow::bail!("List updates operation failed (test failure mode)");
            }

            Ok(self.updates.lock().expect("lock poisoned").clone())
        })
    }

    fn is_installed(&self, package: &str) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let package = package.to_string();
        Box::pin(async move {
            self.installed
                .lock()
                .expect("lock poisoned")
                .contains(&package)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_basic_operations() {
        let pm = TestPackageManager::new();
        pm.add_package("test", "1.0.0", "Test package");

        // Test search
        let results = pm.search("test").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "test");

        // Test info
        let info = pm.info("test").await.unwrap();
        assert!(info.is_some());
        assert_eq!(info.unwrap().name, "test");
    }

    #[tokio::test]
    async fn test_mock_install_remove() {
        let pm = TestPackageManager::new();
        pm.add_package("test", "1.0.0", "Test package");

        // Initially not installed
        assert!(!pm.is_installed("test").await);

        // Install
        pm.install(&["test".to_string()]).await.unwrap();
        assert!(pm.is_installed("test").await);

        // List installed
        let installed = pm.list_installed().await.unwrap();
        assert_eq!(installed.len(), 1);

        // Remove
        pm.remove(&["test".to_string()]).await.unwrap();
        assert!(!pm.is_installed("test").await);
    }

    #[tokio::test]
    async fn test_mock_failure_mode() {
        let pm = TestPackageManager::new();
        pm.set_fail_operations(true);

        assert!(pm.search("test").await.is_err());
        assert!(pm.install(&["test".to_string()]).await.is_err());
        assert!(pm.update().await.is_err());
    }

    #[tokio::test]
    async fn test_mock_with_defaults() {
        let pm = TestPackageManager::with_defaults();
        assert_eq!(pm.package_count(), 4);
        assert!(pm.has_package("firefox"));
        assert!(pm.has_package("git"));
        assert!(pm.is_installed("git").await);
        assert!(!pm.is_installed("firefox").await);
    }

    #[tokio::test]
    async fn test_mock_updates() {
        let pm = TestPackageManager::with_updates();
        let updates = pm.list_updates().await.unwrap();
        assert_eq!(updates.len(), 2);
        assert_eq!(updates[0].name, "firefox");
    }
}
