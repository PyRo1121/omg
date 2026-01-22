//! Package manager trait definition

use anyhow::Result;
use futures::future::BoxFuture;

use crate::core::Package;

/// Trait for package manager backends (Dyn-compatible)
pub trait PackageManager: Send + Sync {
    /// Get the name of this package manager
    fn name(&self) -> &'static str;

    /// Search for packages
    fn search(&self, query: &str) -> BoxFuture<'static, Result<Vec<Package>>>;

    /// Install packages
    fn install(&self, packages: &[String]) -> BoxFuture<'static, Result<()>>;

    /// Remove packages
    fn remove(&self, packages: &[String]) -> BoxFuture<'static, Result<()>>;

    /// Update all packages (upgrade system)
    fn update(&self) -> BoxFuture<'static, Result<()>>;

    /// Synchronize package databases (refresh metadata)
    fn sync(&self) -> BoxFuture<'static, Result<()>>;

    /// Get information about a package
    fn info(&self, package: &str) -> BoxFuture<'static, Result<Option<Package>>>;

    /// List installed packages
    fn list_installed(&self) -> BoxFuture<'static, Result<Vec<Package>>>;

    /// Get system status (total, explicit, orphans, updates)
    fn get_status(&self, fast: bool) -> BoxFuture<'static, Result<(usize, usize, usize, usize)>>;

    /// List explicitly installed package names
    fn list_explicit(&self) -> BoxFuture<'static, Result<Vec<String>>>;

    /// List available updates
    fn list_updates(
        &self,
    ) -> BoxFuture<'static, Result<Vec<crate::package_managers::types::UpdateInfo>>>;

    /// Check if a specific package is installed
    fn is_installed(&self, package: &str) -> BoxFuture<'static, bool>;
}
