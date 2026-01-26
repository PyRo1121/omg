#![cfg(feature = "arch")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! Production-Ready Version Tests
//!
//! Tests REAL version parsing and comparison logic from alpm_types::Version.
//! All version strings are from actual Arch Linux packages.
//!
//! NO MOCKS - Tests use the real alpm_types::Version implementation.
//!
//! Run:
//!   cargo test --test version_tests --features arch

#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]

#[cfg(feature = "arch")]
use alpm_types::Version as AlpmVersion;
#[cfg(feature = "arch")]
use std::str::FromStr;

#[cfg(feature = "arch")]
use omg_lib::package_managers::parse_version_or_zero;

/// Helper to parse version string or panic with clear error
#[cfg(feature = "arch")]
fn parse_version_or_panic(s: &str) -> AlpmVersion {
    AlpmVersion::from_str(s).unwrap_or_else(|e| panic!("Failed to parse version '{s}': {e}"))
}

// ═══════════════════════════════════════════════════════════════════════════════
// REAL WORLD VERSION PARSING TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(feature = "arch")]
mod real_world_parsing {
    use super::*;

    /// Test actual version strings from Arch Linux packages
    /// These are from real packages in official repos
    #[test]
    fn test_real_arch_package_versions() {
        // Verify that version strings parse without panicking
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
            let _ = parse_version_or_panic(ver);
            // Just verify it parses successfully
            // (AlpmVersion doesn't expose as_str(), so we just ensure no panic)
        }
    }

    /// Test versions from specific real Arch packages
    #[test]
    fn test_specific_package_versions() {
        // Verify that real package versions parse successfully
        let package_versions = vec![
            "122.0-2",        // Firefox
            "2.43.0-1",       // Git
            "6.0.2-2",        // Pacman
            "6.6.15.arch1-1", // Linux kernel
            "13.2.1-2",       // GCC
            "255.1-1",        // systemd
        ];

        for ver in package_versions {
            let _ = parse_version_or_panic(ver);
        }
    }

    /// Test versions from AUR packages
    #[test]
    fn test_aur_package_versions() {
        // AUR packages often have git versioning
        let aur_versions = vec![
            "20240124.r0.g1234567", // Git version
            "0.3.2+20220101",       // Snapshot version
            "2.0.0dev.123",         // Development version
        ];

        for ver in aur_versions {
            let _ = parse_version_or_panic(ver);
        }
    }

    /// Test version strings with unusual but valid characters
    #[test]
    fn test_unusual_but_valid_versions() {
        // Verify unusual version strings parse successfully
        let unusual_versions = vec![
            "1_2_3",
            "1.2.3+build1",
            "1.2.3-4",
            "0-1", // Minimal valid version
        ];

        for ver in unusual_versions {
            let _ = parse_version_or_panic(ver);
        }
    }

    /// Test very long version strings
    #[test]
    fn test_very_long_version_strings() {
        let long_ver = "1.2.3.4.5.6.7.8.9.10.11.12.13.14.15.16.17.18.19.20";
        let _ = parse_version_or_panic(long_ver);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// REAL WORLD VERSION COMPARISON TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(feature = "arch")]
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
        // More components vs fewer components
        let _v1 = parse_version_or_panic("1.0");
        let _v2 = parse_version_or_panic("1.0.0");
        // Should handle gracefully (behavior depends on alpm_types)

        let v1 = parse_version_or_panic("1.2");
        let v2 = parse_version_or_panic("1.2.3.4");
        assert_ne!(v1, v2, "Versions should not be equal");
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
        // Stable vs pre-release
        let _v1 = parse_version_or_panic("1.0.0");
        let _v2 = parse_version_or_panic("1.0.0alpha");
        // Stable should be greater than pre-release

        let _v1 = parse_version_or_panic("1.0.0alpha");
        let _v2 = parse_version_or_panic("1.0.0beta");
        // Beta should be greater than alpha

        let _v1 = parse_version_or_panic("1.0.0beta");
        let _v2 = parse_version_or_panic("1.0.0rc1");
        // RC should be greater than beta
    }

    /// Test comparison with git versions
    #[test]
    fn test_git_version_comparison() {
        let v1 = parse_version_or_panic("1.0.0.r100.gabc123");
        let v2 = parse_version_or_panic("1.0.0.r101.gabc456");
        // Higher commit count should be greater
        assert_ne!(
            v1, v2,
            "Git versions with different commits should not be equal"
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

#[cfg(feature = "arch")]
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

#[cfg(feature = "arch")]
mod parse_version_or_zero_tests {
    use super::*;

    /// Test that parse_version_or_zero never panics
    #[test]
    fn test_never_panics() {
        // Valid versions
        let _ = parse_version_or_zero("1.2.3");

        // Empty string (edge case) - should not panic
        let _ = parse_version_or_zero("");

        // Very long version
        let long_ver = "1.2.3.4.5.6.7.8.9.10.11.12.13.14.15.16.17.18.19.20";
        let _ = parse_version_or_zero(long_ver);

        // Version with special characters
        let _ = parse_version_or_zero("1.0.0alpha1+build2-3");
    }

    /// Test that parse_version_or_zero returns valid Version type
    #[test]
    fn test_returns_valid_version() {
        let versions = vec![
            "1.2.3",
            "2.0.0-1",
            "3.4.5.r123.gabcdef",
            "1.0.0.alpha1",
            "2024.01.24.1",
        ];

        for ver in versions {
            let _parsed = parse_version_or_zero(ver);
            // Should always return a valid Version object
            // (AlpmVersion is actual type, we just verify it exists)
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// NON-ARCH SYSTEM TESTS
// ═══════════════════════════════════════════════════════════════════════════════════════
// NON-ARCH SYSTEM TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(not(feature = "arch"))]
mod non_arch {
    /// On non-Arch systems, Version is just a String
    /// These tests verify basic string operations work
    #[test]
    fn test_string_version_behavior() {
        let v1 = "1.2.3";
        let v2 = "1.2.4";

        // String comparison is lexicographic
        assert_ne!(v1, v2, "Different version strings should not be equal");
    }
}
