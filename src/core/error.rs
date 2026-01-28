//! Error types for OMG with helpful suggestions
//!
//! Error codes follow the pattern `OMG-Ennn`:
//! - E001-E099: Package-related errors
//! - E100-E199: Runtime/version errors
//! - E200-E299: System/IO errors
//! - E300-E399: Network errors
//! - E400-E499: Configuration errors
//! - E500-E599: Daemon errors

use thiserror::Error;

/// Convenience Result type for OMG operations
pub type Result<T> = std::result::Result<T, OmgError>;

#[derive(Error, Debug)]
pub enum OmgError {
    #[error("[OMG-E001] Package not found: {0}")]
    PackageNotFound(String),

    #[error("[OMG-E101] Version not found: {runtime} {version}")]
    VersionNotFound { runtime: String, version: String },

    #[error("[OMG-E102] Runtime not supported: {0}")]
    UnsupportedRuntime(String),

    #[error("[OMG-E201] Database error: {0}")]
    DatabaseError(#[from] redb::Error),

    #[error("[OMG-E202] IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("[OMG-E301] Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("[OMG-E401] Configuration error: {0}")]
    ConfigError(String),

    #[error("[OMG-E501] Daemon not running")]
    DaemonNotRunning,

    #[error("[OMG-E203] Permission denied: {0}")]
    PermissionDenied(String),

    #[error("[OMG-E302] Rate limit exceeded")]
    RateLimitExceeded {
        /// Seconds to wait before retrying (from Retry-After header)
        retry_after: Option<u64>,
        /// Human-readable message
        message: String,
    },

    #[error("{0}")]
    Other(String),
}

impl OmgError {
    /// Get the error code for this error variant
    ///
    /// # Rust 2026: const fn for compile-time error code extraction
    #[must_use]
    pub const fn code(&self) -> Option<&'static str> {
        match self {
            Self::PackageNotFound(_) => Some("OMG-E001"),
            Self::VersionNotFound { .. } => Some("OMG-E101"),
            Self::UnsupportedRuntime(_) => Some("OMG-E102"),
            Self::DatabaseError(_) => Some("OMG-E201"),
            Self::IoError(_) => Some("OMG-E202"),
            Self::PermissionDenied(_) => Some("OMG-E203"),
            Self::NetworkError(_) => Some("OMG-E301"),
            Self::RateLimitExceeded { .. } => Some("OMG-E302"),
            Self::ConfigError(_) => Some("OMG-E401"),
            Self::DaemonNotRunning => Some("OMG-E501"),
            Self::Other(_) => None,
        }
    }
}

impl From<anyhow::Error> for OmgError {
    fn from(err: anyhow::Error) -> Self {
        Self::Other(err.to_string())
    }
}

impl OmgError {
    /// Get a helpful suggestion for how to fix this error
    #[must_use]
    pub fn suggestion(&self) -> Option<&'static str> {
        match self {
            Self::PackageNotFound(_) => Some("Try: omg search <query> to find available packages"),
            Self::VersionNotFound { runtime, .. } => match runtime.as_str() {
                "node" => Some("Try: omg list node --available to see available versions"),
                "python" => Some("Try: omg list python --available to see available versions"),
                "go" => Some("Try: omg list go --available to see available versions"),
                "rust" => Some("Try: omg list rust --available to see available versions"),
                "bun" => Some("Try: omg list bun --available to see available versions"),
                "ruby" => Some("Try: omg list ruby --available to see available versions"),
                "java" => Some("Try: omg list java --available to see available versions"),
                _ => Some("Try: omg list <runtime> --available to see available versions"),
            },
            Self::UnsupportedRuntime(_) => {
                Some("Supported runtimes: node, python, go, rust, ruby, java, bun")
            }
            Self::DaemonNotRunning => Some(
                "Start the daemon with: omgd start\nOr run without daemon: omg --no-daemon <command>",
            ),
            Self::PermissionDenied(_) => {
                Some("Try running with sudo, or check file/directory permissions")
            }
            Self::NetworkError(_) => Some(
                "Check your internet connection and try again.\nIf behind a proxy, set HTTP_PROXY/HTTPS_PROXY",
            ),
            Self::ConfigError(_) => Some(
                "Check ~/.config/omg/config.toml for syntax errors.\nReset with: rm ~/.config/omg/config.toml",
            ),
            Self::DatabaseError(_) => {
                Some("Database may be corrupted. Try: rm -rf ~/.local/share/omg/db && omg sync")
            }
            Self::RateLimitExceeded { .. } => {
                Some("Wait for the cooldown period, then retry your request")
            }
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
    if msg.contains("rate limit") || msg.contains("too many requests") {
        return Some("Wait for the cooldown period, then retry your request");
    }
    if msg.contains("no such file") || msg.contains("file not found") {
        return Some("Check that the file path is correct and the file exists");
    }
    if msg.contains("lock") && (msg.contains("exists") || msg.contains("conflict")) {
        return Some(
            "A lock file exists. Another process might be running, or remove the lock file manually.",
        );
    }

    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;

    #[test]
    fn test_package_not_found_suggestion() {
        let err = OmgError::PackageNotFound("foo".to_string());
        assert!(err.suggestion().is_some());
        assert!(err.suggestion().unwrap().contains("search"));
    }

    #[test]
    fn test_version_not_found_suggestion() {
        let err = OmgError::VersionNotFound {
            runtime: "node".to_string(),
            version: "99.0.0".to_string(),
        };
        assert!(err.suggestion().is_some());
        assert!(err.suggestion().unwrap().contains("node"));
    }

    #[test]
    fn test_unsupported_runtime_suggestion() {
        let err = OmgError::UnsupportedRuntime("unknown".to_string());
        assert!(err.suggestion().is_some());
        assert!(err.suggestion().unwrap().contains("node"));
    }

    #[test]
    fn test_daemon_not_running_suggestion() {
        let err = OmgError::DaemonNotRunning;
        assert!(err.suggestion().is_some());
        assert!(err.suggestion().unwrap().contains("omgd"));
    }

    #[test]
    fn test_format_error_with_suggestion() {
        let err = OmgError::PackageNotFound("test".to_string());
        let formatted = format_error_with_suggestion(&err);
        assert!(formatted.contains("Error:"));
        assert!(formatted.contains("💡"));
    }

    #[test]
    fn test_suggest_for_anyhow_permission() {
        let err = anyhow::anyhow!("permission denied: /etc/foo");
        assert!(suggest_for_anyhow(&err).is_some());
    }

    #[test]
    fn test_suggest_for_anyhow_network() {
        let err = anyhow::anyhow!("connection refused");
        assert!(suggest_for_anyhow(&err).is_some());
    }

    #[test]
    fn test_suggest_for_anyhow_none() {
        let err = anyhow::anyhow!("some random error");
        assert!(suggest_for_anyhow(&err).is_none());
    }
}
