//! Package manager trait definition

use std::future::Future;
use std::pin::Pin;

use anyhow::Result;

use crate::core::Package;

/// Trait for package manager backends (object-safe for dynamic dispatch)
///
/// Uses manually desugared async methods (returning `Pin<Box<dyn Future>>`)
/// to maintain object safety for `Arc<dyn PackageManager>`.
pub trait PackageManager: Send + Sync {
    /// Get the name of this package manager
    fn name(&self) -> &'static str;

    /// Search for packages
    fn search(
        &self,
        query: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>>;

    /// Install packages
    fn install(&self, packages: &[String])
    -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Remove packages
    fn remove(&self, packages: &[String]) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Update all packages (upgrade system)
    fn update(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Synchronize package databases (refresh metadata)
    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>>;

    /// Get information about a package
    fn info(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Package>>> + Send + '_>>;

    /// List installed packages
    fn list_installed(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>>;

    /// Get system status (total, explicit, orphans, updates)
    fn get_status(
        &self,
        fast: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(usize, usize, usize, usize)>> + Send + '_>>;

    /// List explicitly installed package names
    fn list_explicit(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>>;

    /// List available updates
    fn list_updates(
        &self,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<crate::package_managers::types::UpdateInfo>>>
                + Send
                + '_,
        >,
    >;

    /// Check if a specific package is installed
    fn is_installed(&self, package: &str) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;
}
