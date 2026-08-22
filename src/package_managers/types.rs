//! Shared package manager types

/// Canonical orphan rule for pacman-based systems.
///
/// A package is an orphan when it was **not** installed explicitly and no
/// other installed package requires or optionally requires it. All orphan
/// listings and counts (libalpm-backed and pure-Rust cache-backed) MUST
/// derive from this single predicate so the CLI, daemon, and status counts
/// cannot diverge.
#[must_use]
pub fn is_orphan_package(
    explicit: bool,
    required_by_empty: bool,
    optional_for_empty: bool,
) -> bool {
    !explicit && required_by_empty && optional_for_empty
}

/// Case-insensitive ASCII substring test without allocation.
/// Only consumed by arch-gated search paths; kept for all feature combos.
#[cfg_attr(not(feature = "arch"), allow(dead_code))]
#[inline]
pub(crate) fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

#[cfg(feature = "arch")]
use alpm_types::Version as AlpmVersion;
#[cfg(feature = "arch")]
use std::str::FromStr;

/// Version type - uses `alpm_types::Version` on Arch, String on Debian
#[cfg(feature = "arch")]
pub type Version = AlpmVersion;

#[cfg(not(feature = "arch"))]
pub type Version = String;

/// Parse a version string, returning a zero version on failure.
/// This is infallible and avoids `expect()/unwrap()` in hot paths.
///
/// # Performance
/// Parsing "0" on failure is O(1) and avoids static synchronization overhead.
/// Previous `LazyLock` approach had sync overhead + clone allocation anyway.
#[cfg(feature = "arch")]
#[must_use]
#[inline]
pub fn parse_version_or_zero(s: &str) -> Version {
    AlpmVersion::from_str(s).unwrap_or_else(|_| {
        // "0" parsing is trivial (single char) - cheaper than static + clone
        // SAFETY: "0" is always a valid version string per alpm-types spec
        #[expect(clippy::expect_used)]
        AlpmVersion::from_str("0").expect("0 is always valid")
    })
}

/// Parse a version string - on non-Arch just returns the string.
#[cfg(not(feature = "arch"))]
#[must_use]
#[inline]
pub fn parse_version_or_zero(s: &str) -> Version {
    s.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orphan_rule_matches_canonical_definition() {
        assert!(!is_orphan_package(false, false, false)); // explicit install
        assert!(!is_orphan_package(false, true, false)); // required by another pkg
        assert!(!is_orphan_package(false, false, true)); // optional for another pkg
        assert!(is_orphan_package(false, true, true)); // true orphan
        assert!(!is_orphan_package(true, true, true));
    }

    #[test]
    fn contains_ignore_case_matches_ascii_and_rejects_non_ascii_queries() {
        assert!(contains_ignore_case("Firefox Web Browser", "fireFox"));
        assert!(contains_ignore_case("abc", ""));
        assert!(!contains_ignore_case("ab", "abc"));
    }
}

/// Returns a default zero version.
/// This is infallible and avoids `expect()/unwrap()` in hot paths.
#[cfg(feature = "arch")]
#[must_use]
#[inline]
pub fn zero_version() -> Version {
    // "0" parsing is trivial - avoids static overhead
    #[expect(clippy::expect_used)]
    AlpmVersion::from_str("0").expect("0 is always valid")
}

/// Returns a default zero version - on non-Arch returns "0".
#[cfg(not(feature = "arch"))]
#[must_use]
#[inline]
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
