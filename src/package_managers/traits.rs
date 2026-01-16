//! Package manager trait definition

use anyhow::Result;

use crate::core::Package;

/// Trait for package manager backends (Rust 2024 native async traits)
pub trait PackageManager: Send + Sync {
    /// Get the name of this package manager
    fn name(&self) -> &'static str;

    /// Search for packages
    fn search(&self, query: &str) -> impl Future<Output = Result<Vec<Package>>> + Send;

    /// Install packages
    fn install(&self, packages: &[String]) -> impl Future<Output = Result<()>> + Send;

    /// Remove packages
    fn remove(&self, packages: &[String]) -> impl Future<Output = Result<()>> + Send;

    /// Update all packages
    fn update(&self) -> impl Future<Output = Result<()>> + Send;

    /// Get information about a package
    fn info(&self, package: &str) -> impl Future<Output = Result<Option<Package>>> + Send;

    /// List installed packages
    fn list_installed(&self) -> impl Future<Output = Result<Vec<Package>>> + Send;
}
