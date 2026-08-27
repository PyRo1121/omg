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

/// Version type - uses `alpm_types::Version` on Arch, a dpkg-style ordered
/// newtype everywhere else.
#[cfg(feature = "arch")]
pub type Version = AlpmVersion;

/// Ordered version for non-Arch backends (homebrew, debian-pure).
///
/// Regression guard: this was a bare `String`, so `1.9 > 1.10` compared
/// lexicographically and security updates were silently reported as up to
/// date on every non-Arch build.
#[cfg(not(feature = "arch"))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct DebVersion(String);

#[cfg(not(feature = "arch"))]
impl DebVersion {
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[cfg(not(feature = "arch"))]
impl std::fmt::Display for DebVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(not(feature = "arch"))]
impl PartialOrd for DebVersion {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(not(feature = "arch"))]
impl Ord for DebVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        compare_deb_versions(&self.0, &other.0)
    }
}

/// Character weight per Debian policy §5.6.12: `~` sorts before everything
/// (including end of string), letters by ASCII, everything else after letters.
#[cfg(not(feature = "arch"))]
fn deb_char_order(c: u8) -> i64 {
    match c {
        b'~' => -1,
        b'0'..=b'9' => 0,
        _ => i64::from(c),
    }
}

/// Compare two version fragments using the dpkg alternating
/// non-digit/numeric-run algorithm.
#[cfg(not(feature = "arch"))]
fn compare_deb_fragments(mut a: &[u8], mut b: &[u8]) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    while !a.is_empty() || !b.is_empty() {
        // Compare leading runs of non-digit characters.
        while (!a.is_empty() && !a[0].is_ascii_digit()) || (!b.is_empty() && !b[0].is_ascii_digit())
        {
            let ac = a.first().map_or(0, |&c| deb_char_order(c));
            let bc = b.first().map_or(0, |&c| deb_char_order(c));
            if ac != bc {
                return ac.cmp(&bc);
            }
            if !a.is_empty() {
                a = &a[1..];
            }
            if !b.is_empty() {
                b = &b[1..];
            }
        }
        // Skip leading zeros in numeric runs.
        while a.first().is_some_and(|c| *c == b'0') {
            a = &a[1..];
        }
        while b.first().is_some_and(|c| *c == b'0') {
            b = &b[1..];
        }
        // Compare numeric runs by length then digits (both zero-trimmed).
        let a_digits = a.iter().take_while(|c| c.is_ascii_digit()).count();
        let b_digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
        match a_digits.cmp(&b_digits) {
            Ordering::Equal => {}
            other => return other,
        }
        let ordering = a[..a_digits].cmp(&b[..b_digits]);
        if ordering != Ordering::Equal {
            return ordering;
        }
        a = &a[a_digits..];
        b = &b[b_digits..];
    }
    Ordering::Equal
}

/// Split an optional epoch (`digits:`) from the rest of the version.
#[cfg(not(feature = "arch"))]
fn split_epoch(version: &str) -> (u64, &str) {
    match version.split_once(':') {
        Some((epoch, rest)) if !epoch.is_empty() && epoch.bytes().all(|c| c.is_ascii_digit()) => {
            (epoch.parse::<u64>().unwrap_or(0), rest)
        }
        _ => (0, version),
    }
}

/// dpkg-style full version comparison: epoch, upstream part, revision.
#[cfg(not(feature = "arch"))]
fn compare_deb_versions(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    let (epoch_a, rest_a) = split_epoch(a);
    let (epoch_b, rest_b) = split_epoch(b);
    match epoch_a.cmp(&epoch_b) {
        Ordering::Equal => {}
        other => return other,
    }
    // Revision is everything after the LAST hyphen; upstream is before it.
    let (upstream_a, revision_a) = match rest_a.rsplit_once('-') {
        Some((up, rev)) => (up, Some(rev)),
        None => (rest_a, None),
    };
    let (upstream_b, revision_b) = match rest_b.rsplit_once('-') {
        Some((up, rev)) => (up, Some(rev)),
        None => (rest_b, None),
    };
    match compare_deb_fragments(upstream_a.as_bytes(), upstream_b.as_bytes()) {
        Ordering::Equal => {}
        other => return other,
    }
    compare_deb_fragments(
        revision_a.unwrap_or_default().as_bytes(),
        revision_b.unwrap_or_default().as_bytes(),
    )
}

#[cfg(not(feature = "arch"))]
pub type Version = DebVersion;

/// Uniform borrowed rendering for the backend-dependent [`Version`] type.
pub(crate) trait VersionDisplay {
    fn version_string(&self) -> String;
}

#[cfg(feature = "arch")]
impl VersionDisplay for AlpmVersion {
    #[allow(
        clippy::implicit_clone,
        reason = "alpm-types Display consumes self; centralize the required clone"
    )]
    fn version_string(&self) -> String {
        self.to_string()
    }
}

#[cfg(not(feature = "arch"))]
impl VersionDisplay for DebVersion {
    fn version_string(&self) -> String {
        self.as_str().to_owned()
    }
}

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

/// Parse a version string - infallible wrapper retaining the raw text.
#[cfg(not(feature = "arch"))]
#[must_use]
#[inline]
pub fn parse_version_or_zero(s: &str) -> Version {
    Version::new(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_display_is_borrowed_and_backend_independent() {
        let version = parse_version_or_zero("1:2.3.4-5");
        assert_eq!(version.version_string(), "1:2.3.4-5");
        assert_eq!(version.version_string(), "1:2.3.4-5");
    }

    #[cfg(not(feature = "arch"))]
    #[test]
    fn deb_version_comparison_follows_dpkg_ordering() {
        // Regression: bare-String comparison called 1.9 > 1.10 and hid
        // security updates on non-Arch builds.
        let v = |s: &str| Version::new(s);
        assert!(v("1.10") > v("1.9"));
        assert!(v("5.10-1") > v("5.9-1"));
        assert!(v("10.0") > v("2.0"));
        assert!(v("20251231") > v("20250101"));
        assert!(v("2.0") > v("1:1.0"), "epoch dominates upstream part");
        assert!(v("1.0") > v("1.0~rc1"), "tilde sorts before everything");
        assert!(v("1.0-2") > v("1.0-1"));
        assert_eq!(v("1.0"), v("1.0"));
        assert!(v("1.0a") < v("1.0b"));
        assert!(v("1.0") < v("1.01"));
    }

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
    Version::new("0")
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
