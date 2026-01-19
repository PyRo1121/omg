//! Runtime version manager trait and implementation

use anyhow::Result;

use crate::core::{Runtime, RuntimeVersion};

/// Trait for runtime version managers (Rust 2024 native async traits)
pub trait RuntimeManager: Send + Sync {
    /// Get the runtime type
    fn runtime(&self) -> Runtime;

    /// List available versions for download
    fn list_available(&self) -> impl Future<Output = Result<Vec<String>>> + Send;

    /// List installed versions
    fn list_installed(&self) -> Result<Vec<RuntimeVersion>>;

    /// Install a specific version
    fn install(&self, version: &str) -> impl Future<Output = Result<()>> + Send;

    /// Uninstall a specific version
    fn uninstall(&self, version: &str) -> Result<()>;

    /// Get the currently active version
    fn active_version(&self) -> Result<Option<String>>;

    /// Set the active version
    fn set_active(&self, version: &str) -> Result<()>;

    /// Get the path to a specific version's binaries
    fn version_bin_path(&self, version: &str) -> Result<std::path::PathBuf>;
}
