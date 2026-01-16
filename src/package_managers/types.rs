//! Shared package manager types

use alpm_types::Version;
use std::str::FromStr;
use std::sync::LazyLock;

/// A cached zero version to avoid repeated parsing.
static ZERO_VERSION: LazyLock<Version> = LazyLock::new(|| {
    // "0" is always a valid version string per alpm-types spec
    #[allow(clippy::expect_used)]
    Version::from_str("0").expect("0 is always valid")
});

/// Parse a version string, returning a zero version on failure.
/// This is infallible and avoids expect()/unwrap() in hot paths.
#[must_use]
pub fn parse_version_or_zero(s: &str) -> Version {
    Version::from_str(s).unwrap_or_else(|_| ZERO_VERSION.clone())
}

/// Returns a default zero version.
/// This is infallible and avoids expect()/unwrap() in hot paths.
#[must_use]
pub fn zero_version() -> Version {
    ZERO_VERSION.clone()
}

#[derive(Debug, Clone)]
pub struct LocalPackage {
    pub name: String,
    pub version: Version,
    pub description: String,
    pub install_size: i64,
    pub reason: &'static str,
}

#[derive(Debug, Clone)]
pub struct SyncPackage {
    pub name: String,
    pub version: Version,
    pub description: String,
    pub repo: String,
    pub download_size: i64,
    pub installed: bool,
}

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: Version,
    pub description: String,
    pub url: Option<String>,
    pub size: u64,
    pub install_size: Option<i64>,
    pub download_size: Option<u64>,
    pub repo: String,
    pub depends: Vec<String>,
    pub licenses: Vec<String>,
    pub installed: bool,
}
