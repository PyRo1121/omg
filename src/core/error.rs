//! Error types for OMG

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
