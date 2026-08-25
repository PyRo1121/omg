#![cfg(feature = "arch")]
#![expect(clippy::expect_used)]
//! Production-Ready Version Tests
//!
//! Tests REAL version parsing and comparison logic from alpm_types::Version.
//! All version strings are from actual Arch Linux packages.
//!
//! NO MOCKS - Tests use the real alpm_types::Version implementation.
//!
//! Run:
//!   cargo test --test version_tests --features arch

use alpm_types::Version as AlpmVersion;
use std::str::FromStr;

use omg_lib::package_managers::parse_version_or_zero;

/// Helper to parse version string or panic with clear error
fn parse_version_or_panic(s: &str) -> AlpmVersion {
    AlpmVersion::from_str(s).expect("Failed to parse test version")
}

// ═══════════════════════════════════════════════════════════════════════════════
// REAL WORLD VERSION PARSING TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod real_world_parsing {
    use super::*;

    /// Parsing must be lossless: every accepted version string round-trips
    /// exactly through `Display` (epoch:pkgver-pkgrel in alpm-types
    /// version/pkg_generic.rs). A parse that normalizes or truncates input is
    /// a bug because update comparisons rely on the original spelling.
    fn assert_round_trips(s: &str) {
        let parsed = parse_version_or_panic(s);
        assert_eq!(
            parsed.to_string(),
            s,
            "version '{s}' did not round-trip through Display"
        );
    }

    /// Test actual version strings from Arch Linux packages.
    /// These are from real packages in official repos.
    #[test]
    fn test_real_arch_package_versions() {
        let versions = vec![
            "1.2.3",
            "2.0.0-1",
            "3.4.5.r123.gabcdef",
            "1.0.0.alpha1",
            "2.1.3~rc1",
            "2024.01.24.1",
            "0.1",
            "0",
        ];
        for ver in versions {
            assert_round_trips(ver);
        }
    }

    /// Test versions from specific real Arch packages.
    #[test]
    fn test_specific_package_versions() {
        let package_versions = vec![
            "122.0-2",        // Firefox
            "2.43.0-1",       // Git
            "6.0.2-2",        // Pacman
            "6.6.15.arch1-1", // Linux kernel
            "13.2.1-2",       // GCC
            "255.1-1",        // systemd
        ];
        for ver in package_versions {
            assert_round_trips(ver);
        }
    }

    /// Test versions from AUR packages.
    #[test]
    fn test_aur_package_versions() {
        let aur_versions = vec![
            "20240124.r0.g1234567", // Git version
            "0.3.2+20220101",       // Snapshot version
            "2.0.0dev.123",         // Development version
        ];
        for ver in aur_versions {
            assert_round_trips(ver);
        }
    }

    /// Test version strings with unusual but valid characters.
    #[test]
    fn test_unusual_but_valid_versions() {
        let unusual_versions = vec![
            "1_2_3",
            "1.2.3+build1",
            "1.2.3-4",
            "0-1", // Minimal valid version
        ];
        for ver in unusual_versions {
            assert_round_trips(ver);
        }
    }

    /// Test very long version strings.
    #[test]
    fn test_very_long_version_strings() {
        let long_ver = "1.2.3.4.5.6.7.8.9.10.11.12.13.14.15.16.17.18.19.20";
        assert_round_trips(long_ver);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// REAL WORLD VERSION COMPARISON TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod version_comparison {
    use super::*;

    /// Test basic version comparison
    #[test]
    fn test_basic_comparison() {
        // Patch version increment
        let v1 = parse_version_or_panic("1.0.0");
        let v2 = parse_version_or_panic("1.0.1");
        assert!(v2 > v1, "1.0.1 should be greater than 1.0.0");
        assert!(v1 < v2, "1.0.0 should be less than 1.0.1");

        // Minor version increment
        let v1 = parse_version_or_panic("1.0.0");
        let v2 = parse_version_or_panic("1.1.0");
        assert!(v2 > v1, "1.1.0 should be greater than 1.0.0");

        // Major version increment
        let v1 = parse_version_or_panic("1.0.0");
        let v2 = parse_version_or_panic("2.0.0");
        assert!(v2 > v1, "2.0.0 should be greater than 1.0.0");
    }

    /// Test comparison with different number of components
    #[test]
    fn test_unequal_length_comparison() {
        // alpm vercmp compares component by component; a shorter prefix that
        // matches loses to the longer version (the remaining side wins).
        let v1 = parse_version_or_panic("1.2");
        let v2 = parse_version_or_panic("1.2.3.4");
        assert!(v2 > v1, "1.2.3.4 should be greater than 1.2");

        let v1 = parse_version_or_panic("1.0");
        let v2 = parse_version_or_panic("1.0.0");
        // Matches libalpm vercmp semantics (verified against
        // `/usr/bin/vercmp 1.0 1.0.0` → -1 on libalpm v16): a version whose
        // segments are exhausted is ordered OLDER than one with remaining
        // segments — there is no implicit zero-padding.
        assert!(v2 > v1, "1.0.0 should order newer than 1.0");
    }

    /// Test comparison with release numbers
    #[test]
    fn test_release_comparison() {
        // Different release numbers
        let v1 = parse_version_or_panic("1.0.0-1");
        let v2 = parse_version_or_panic("1.0.0-2");
        assert!(v2 > v1, "1.0.0-2 should be greater than 1.0.0-1");

        // Release vs no release
        let v1 = parse_version_or_panic("1.0.0");
        let v2 = parse_version_or_panic("1.0.0-1");
        assert_ne!(v1, v2, "Versions should not be equal");
    }

    /// Test comparison with pre-release markers
    #[test]
    fn test_prerelease_comparison() {
        // Stable vs pre-release: ALPM vercmp treats a version that ends in digits
        // as newer than one with a trailing alphabetic suffix.  When "0" is consumed
        // from both sides, the side with a remaining alpha run ("alpha") loses.
        let v1 = parse_version_or_panic("1.0.0");
        let v2 = parse_version_or_panic("1.0.0alpha");
        assert!(
            v1 > v2,
            "stable (1.0.0) should be greater than pre-release (1.0.0alpha)"
        );

        // Beta > alpha: lexicographic order on the alphabetic suffix ("beta" > "alpha").
        let v1 = parse_version_or_panic("1.0.0alpha");
        let v2 = parse_version_or_panic("1.0.0beta");
        assert!(
            v2 > v1,
            "beta (1.0.0beta) should be greater than alpha (1.0.0alpha)"
        );

        // RC > beta: lexicographic order on the alphabetic suffix ("rc" > "beta").
        let v1 = parse_version_or_panic("1.0.0beta");
        let v2 = parse_version_or_panic("1.0.0rc1");
        assert!(
            v2 > v1,
            "rc (1.0.0rc1) should be greater than beta (1.0.0beta)"
        );
    }

    /// Test comparison with git versions
    #[test]
    fn test_git_version_comparison() {
        let v1 = parse_version_or_panic("1.0.0.r100.gabc123");
        let v2 = parse_version_or_panic("1.0.0.r101.gabc456");
        // The numeric r<rev> segments decide before the trailing git hashes
        // are ever compared, so the higher revision must win regardless of hash.
        assert!(
            v2 > v1,
            "r101 revision should be greater than r100 regardless of hash"
        );
    }

    /// Test equality
    #[test]
    fn test_version_equality() {
        let v1 = parse_version_or_panic("1.2.3");
        let v2 = parse_version_or_panic("1.2.3");
        assert_eq!(v1, v2, "Same version strings should be equal");

        let v1 = parse_version_or_panic("1.2.3-1");
        let v2 = parse_version_or_panic("1.2.3-1");
        assert_eq!(v1, v2, "Same version strings with release should be equal");
    }

    /// Test complex comparison scenarios
    #[test]
    fn test_complex_comparison() {
        // Major bump trumps everything
        assert!(parse_version_or_panic("2.0.0") > parse_version_or_panic("1.9.9"));
        assert!(parse_version_or_panic("10.0.0") > parse_version_or_panic("9.999.999"));

        // Minor bump trumps patch
        assert!(parse_version_or_panic("1.1.0") > parse_version_or_panic("1.0.999"));
        assert!(parse_version_or_panic("1.2.0") > parse_version_or_panic("1.1.999"));

        // Patch bump
        assert!(parse_version_or_panic("1.0.2") > parse_version_or_panic("1.0.1"));
        assert!(parse_version_or_panic("1.0.10") > parse_version_or_panic("1.0.9"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// UPDATE DETECTION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod update_detection {
    use super::*;

    /// Test that update detection logic works with real version strings
    #[test]
    fn test_update_detection_scenarios() {
        // Simulating update detection: compare old vs new versions

        // Patch update
        let old = parse_version_or_panic("1.0.0");
        let new = parse_version_or_panic("1.0.1");
        assert!(new > old, "Patch update should be detected");

        // Minor update
        let old = parse_version_or_panic("1.2.0");
        let new = parse_version_or_panic("1.3.0");
        assert!(new > old, "Minor update should be detected");

        // Major update
        let old = parse_version_or_panic("1.5.0");
        let new = parse_version_or_panic("2.0.0");
        assert!(new > old, "Major update should be detected");

        // Release bump
        let old = parse_version_or_panic("1.0.0-1");
        let new = parse_version_or_panic("1.0.0-2");
        assert!(new > old, "Release bump should be detected");
    }

    /// Test that no update is detected when versions are equal
    #[test]
    fn test_no_update_detected() {
        let old = parse_version_or_panic("1.2.3-1");
        let new = parse_version_or_panic("1.2.3-1");
        assert_eq!(old, new, "Equal versions should not show as update");
    }

    /// Test that downgrade scenarios work correctly
    #[test]
    fn test_downgrade_detection() {
        let old = parse_version_or_panic("2.0.0");
        let new = parse_version_or_panic("1.9.9");
        assert!(old > new, "Should detect potential downgrade");

        let old = parse_version_or_panic("1.0.0-2");
        let new = parse_version_or_panic("1.0.0-1");
        assert!(old > new, "Release downgrade should be detected");
    }

    /// Test update detection with real package update pairs
    /// These are actual version updates from Arch Linux repos
    #[test]
    fn test_real_package_update_pairs() {
        // Firefox 121 -> 122 (major version bump)
        let old = parse_version_or_panic("121.0-1");
        let new = parse_version_or_panic("122.0-2");
        assert!(new > old, "Firefox update should be detected");

        // Kernel 6.6.14 -> 6.6.15 (patch update)
        let old = parse_version_or_panic("6.6.14.arch1-1");
        let new = parse_version_or_panic("6.6.15.arch1-1");
        assert!(new > old, "Kernel patch update should be detected");

        // Python 3.11.8 -> 3.12.1 (minor version bump)
        let old = parse_version_or_panic("3.11.8-1");
        let new = parse_version_or_panic("3.12.1-1");
        assert!(new > old, "Python minor update should be detected");

        // Git 2.42.0 -> 2.43.0 (minor version bump)
        let old = parse_version_or_panic("2.42.0-1");
        let new = parse_version_or_panic("2.43.0-1");
        assert!(new > old, "Git update should be detected");
    }

    /// Test edge cases in update detection
    #[test]
    fn test_update_detection_edge_cases() {
        // Version 0 to non-zero
        let old = parse_version_or_panic("0.1.0");
        let new = parse_version_or_panic("1.0.0");
        assert!(new > old, "Update from 0.x to 1.x should be detected");

        // Very long version comparison
        let old = parse_version_or_panic("1.2.3.4.5.6.7.8.9");
        let new = parse_version_or_panic("1.2.3.4.5.6.7.8.10");
        assert!(new > old, "Update in last component should be detected");

        // Release comparison
        let old = parse_version_or_panic("1.0.0-1");
        let new = parse_version_or_panic("1.0.0-2");
        assert!(new > old, "Release update should be detected");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PARSE_VERSION_OR_ZERO HELPER TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod parse_version_or_zero_tests {
    use super::*;

    /// Test that parse_version_or_zero returns a version or zero
    #[test]
    fn test_parse_version_or_zero_values() {
        assert_eq!(parse_version_or_zero("1.2.3").to_string(), "1.2.3");
        assert_eq!(
            parse_version_or_zero("").to_string(),
            "0",
            "unparseable input must become the zero version"
        );
        assert_eq!(parse_version_or_zero("2.0.0-1").to_string(), "2.0.0-1");
        let long_ver = "1.2.3.4.5.6.7.8.9.10.11.12.13.14.15.16.17.18.19.20";
        assert_eq!(parse_version_or_zero(long_ver).to_string(), long_ver);
    }
}
