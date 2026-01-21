//! Shared package manager types

#[cfg(feature = "arch")]
use alpm_types::Version as AlpmVersion;
#[cfg(feature = "arch")]
use std::str::FromStr;
#[cfg(feature = "arch")]
use std::sync::LazyLock;

/// Version type - uses `alpm_types::Version` on Arch, String on Debian
#[cfg(feature = "arch")]
pub type Version = AlpmVersion;

#[cfg(not(feature = "arch"))]
pub type Version = String;

/// A cached zero version to avoid repeated parsing.
#[cfg(feature = "arch")]
static ZERO_VERSION: LazyLock<AlpmVersion> = LazyLock::new(|| {
    // "0" is always a valid version string per alpm-types spec
    #[allow(clippy::expect_used)]
    AlpmVersion::from_str("0").expect("0 is always valid")
});

/// Parse a version string, returning a zero version on failure.
/// This is infallible and avoids `expect()/unwrap()` in hot paths.
#[cfg(feature = "arch")]
#[must_use]
pub fn parse_version_or_zero(s: &str) -> Version {
    AlpmVersion::from_str(s).unwrap_or_else(|_| ZERO_VERSION.clone())
}

/// Parse a version string - on non-Arch just returns the string.
#[cfg(not(feature = "arch"))]
#[must_use]
pub fn parse_version_or_zero(s: &str) -> Version {
    s.to_string()
}

/// Returns a default zero version.
/// This is infallible and avoids `expect()/unwrap()` in hot paths.
#[cfg(feature = "arch")]
#[must_use]
pub fn zero_version() -> Version {
    ZERO_VERSION.clone()
}

/// Returns a default zero version - on non-Arch returns "0".
#[cfg(not(feature = "arch"))]
#[must_use]
pub fn zero_version() -> Version {
    "0".to_string()
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateInfo {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
    pub repo: String,
}
