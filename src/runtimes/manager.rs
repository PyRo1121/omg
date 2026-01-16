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
