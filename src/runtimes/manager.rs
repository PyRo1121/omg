//! Runtime version manager trait and implementation

use anyhow::Result;
use async_trait::async_trait;

use crate::core::{Runtime, RuntimeVersion};
// use crate::config::Settings;

/// Trait for runtime version managers
#[async_trait]
pub trait RuntimeManager: Send + Sync {
    /// Get the runtime type
    fn runtime(&self) -> Runtime;

    /// List available versions for download
    async fn list_available(&self) -> Result<Vec<String>>;

    /// List installed versions
    fn list_installed(&self) -> Result<Vec<RuntimeVersion>>;

    /// Install a specific version
    async fn install(&self, version: &str) -> Result<()>;

    /// Uninstall a specific version
    fn uninstall(&self, version: &str) -> Result<()>;

    /// Get the currently active version
    fn active_version(&self) -> Result<Option<String>>;

    /// Set the active version
    fn set_active(&self, version: &str) -> Result<()>;

    /// Get the path to a specific version's binaries
    fn version_bin_path(&self, version: &str) -> Result<std::path::PathBuf>;
}

/*
/// Get the versions directory for a runtime
pub fn versions_dir(settings: &Settings, runtime: Runtime) -> std::path::PathBuf {
    settings.versions_dir().join(runtime.to_string())
}

/// Detect version from version file in current or parent directories
pub fn detect_version_file(runtime: Runtime) -> Option<String> {
    let version_file = runtime.version_file();
    let mut current_dir = std::env::current_dir().ok()?;

    loop {
        let version_path = current_dir.join(version_file);
        if version_path.exists() {
            return std::fs::read_to_string(version_path)
                .ok()
                .map(|s| s.trim().to_string());
        }

        if !current_dir.pop() {
            break;
        }
    }

    None
}
*/
