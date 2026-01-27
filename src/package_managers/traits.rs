//! Package manager trait definition

use anyhow::Result;
use async_trait::async_trait;

use crate::core::Package;

/// Trait for package manager backends (object-safe for dynamic dispatch)
///
/// Uses `async_trait` to enable async methods in object-safe traits.
/// This allows `Arc<dyn PackageManager>` for runtime polymorphism.
///
/// Rust 2026 Note: We keep async_trait for object safety (required for dyn).
/// trait-variant generates native async fn which returns impl Future, making
/// the trait NOT object-safe. Since this codebase uses Arc<dyn PackageManager>
/// extensively, we must keep async_trait for compatibility.
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

    /// Update all packages (upgrade system)
    async fn update(&self) -> Result<()>;

    /// Synchronize package databases (refresh metadata)
    async fn sync(&self) -> Result<()>;

    /// Get information about a package
    async fn info(&self, package: &str) -> Result<Option<Package>>;

    /// List installed packages
    async fn list_installed(&self) -> Result<Vec<Package>>;

    /// Get system status (total, explicit, orphans, updates)
    async fn get_status(&self, fast: bool) -> Result<(usize, usize, usize, usize)>;

    /// List explicitly installed package names
    async fn list_explicit(&self) -> Result<Vec<String>>;

    /// List available updates
    async fn list_updates(&self) -> Result<Vec<crate::package_managers::types::UpdateInfo>>;

    /// Check if a specific package is installed
    async fn is_installed(&self, package: &str) -> bool;
}
