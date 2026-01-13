//! Package manager trait definition

use anyhow::Result;
use async_trait::async_trait;

use crate::core::Package;

/// Trait for package manager backends
#[async_trait]
pub trait PackageManager: Send + Sync {
    /// Get the name of this package manager
    fn name(&self) -> &'static str;

    /// Search for packages
    async fn search(&self, query: &str) -> Result<Vec<Package>>;

    /// Install packages
    async fn install(&self, packages: &[String]) -> Result<()>;

    /// Remove packages
    async fn remove(&self, packages: &[String]) -> Result<()>;

    /// Update all packages
    async fn update(&self) -> Result<()>;

    /// Get information about a package
    async fn info(&self, package: &str) -> Result<Option<Package>>;

    /// List installed packages
    async fn list_installed(&self) -> Result<Vec<Package>>;
}
