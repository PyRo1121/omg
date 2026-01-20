//! Error types for OMG with helpful suggestions

use thiserror::Error;

/// Convenience Result type for OMG operations
pub type Result<T> = std::result::Result<T, OmgError>;

#[derive(Error, Debug)]
pub enum OmgError {
    #[error("Package not found: {0}")]
    PackageNotFound(String),

    #[error("Version not found: {runtime} {version}")]
    VersionNotFound { runtime: String, version: String },

    #[error("Runtime not supported: {0}")]
    UnsupportedRuntime(String),

    #[error("Database error: {0}")]
    DatabaseError(#[from] redb::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Daemon not running")]
    DaemonNotRunning,

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("{0}")]
    Other(String),
}

impl OmgError {
    /// Get a helpful suggestion for how to fix this error
    #[must_use]
    pub fn suggestion(&self) -> Option<&'static str> {
        match self {
            Self::PackageNotFound(_) => Some(
                "Try: omg search <query> to find available packages",
            ),
            Self::VersionNotFound { runtime, .. } => {
                match runtime.as_str() {
                    "node" => Some("Try: omg list node --available to see available versions"),
                    "python" => Some("Try: omg list python --available to see available versions"),
                    "go" => Some("Try: omg list go --available to see available versions"),
                    "rust" => Some("Try: omg list rust --available to see available versions"),
                    "bun" => Some("Try: omg list bun --available to see available versions"),
                    "ruby" => Some("Try: omg list ruby --available to see available versions"),
                    "java" => Some("Try: omg list java --available to see available versions"),
                    _ => Some("Try: omg list <runtime> --available to see available versions"),
                }
            }
            Self::UnsupportedRuntime(_) => Some(
                "Supported runtimes: node, python, go, rust, ruby, java, bun",
            ),
            Self::DaemonNotRunning => Some(
                "Start the daemon with: omgd start\nOr run without daemon: omg --no-daemon <command>",
            ),
            Self::PermissionDenied(_) => Some(
                "Try running with sudo, or check file/directory permissions",
            ),
            Self::NetworkError(_) => Some(
                "Check your internet connection and try again.\nIf behind a proxy, set HTTP_PROXY/HTTPS_PROXY",
            ),
            Self::ConfigError(_) => Some(
                "Check ~/.config/omg/config.toml for syntax errors.\nReset with: rm ~/.config/omg/config.toml",
            ),
            Self::DatabaseError(_) => Some(
                "Database may be corrupted. Try: rm -rf ~/.local/share/omg/db && omg sync",
            ),
            Self::IoError(_) | Self::Other(_) => None,
        }
    }
}

/// Format an error with its suggestion for display
pub fn format_error_with_suggestion(err: &OmgError) -> String {
    let mut msg = format!("Error: {err}");
    if let Some(suggestion) = err.suggestion() {
        msg.push_str("\n\n💡 ");
        msg.push_str(suggestion);
    }
    msg
}

/// Common error suggestions for anyhow errors
pub fn suggest_for_anyhow(err: &anyhow::Error) -> Option<&'static str> {
    let msg = err.to_string().to_lowercase();
    
    if msg.contains("package not found") || msg.contains("no such package") {
        return Some("Try: omg search <query> to find available packages");
    }
    if msg.contains("version not found") || msg.contains("no matching version") {
        return Some("Try: omg list <runtime> --available to see available versions");
    }
    if msg.contains("permission denied") || msg.contains("access denied") {
        return Some("Try running with sudo, or check file/directory permissions");
    }
    if msg.contains("connection") || msg.contains("network") || msg.contains("timeout") {
        return Some("Check your internet connection and try again");
    }
    if msg.contains("not found") && msg.contains("command") {
        return Some("The required tool is not installed. Try: omg tool install <name>");
    }
    if msg.contains("daemon") {
        return Some("Start the daemon with: omgd start");
    }
    if msg.contains("no such file") || msg.contains("file not found") {
        return Some("Check that the file path is correct and the file exists");
    }
    
    None
}
