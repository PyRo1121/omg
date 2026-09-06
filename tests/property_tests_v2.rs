//! Advanced Property-Based Testing for OMG
//!
//! Uses proptest to discover edge cases and verify invariants across:
//! - Version parsing and comparison
//! - UpdateType classification
//! - Elm Architecture model updates
//! - Package name validation
//!
//! Run: cargo test --test property_tests_v2
//!
//! For faster iteration: cargo test --test property_tests_v2 -- --test-threads=1

#![expect(clippy::pedantic)]

use proptest::prelude::*;

pub mod common;
use common::assertions::assert_process_completed;
use common::*;

// ═══════════════════════════════════════════════════════════════════════════════
// VERSION PARSING PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_same_version_is_not_an_update(
        major in 0u32..50u32,
        minor in 0u32..50u32,
        patch in 0u32..50u32
    ) {
        use omg_lib::cli::tea::UpdateType;

        let version = format!("{major}.{minor}.{patch}");
        let update_type = UpdateType::from_versions(&version, &version);

        prop_assert_eq!(update_type, UpdateType::Unknown);
    }

    #[test]
    fn prop_major_bump_detected(
        old_minor in 0u32..10u32,
        old_patch in 0u32..10u32,
        new_major in 1u32..20u32, // Ensure at least 1 to be > old
        new_minor in 0u32..10u32,
        new_patch in 0u32..10u32
    ) {
        use omg_lib::cli::tea::UpdateType;

        let old_major = 0u32;
        let old_version = format!("{old_major}.{old_minor}.{old_patch}");
        let new_version = format!("{new_major}.{new_minor}.{new_patch}");

        let update_type = UpdateType::from_versions(&old_version, &new_version);

        prop_assert_eq!(update_type, UpdateType::Major);
    }

    #[test]
    fn prop_minor_bump_detected(
        major in 0u32..10u32,
        old_minor in 0u32..10u32,
        new_minor in 1u32..20u32,
        old_patch in 0u32..10u32,
        new_patch in 0u32..10u32
    ) {
        use omg_lib::cli::tea::UpdateType;

        // Ensure new_minor > old_minor
        let (old_minor, new_minor) = if old_minor < new_minor {
            (old_minor, new_minor)
        } else {
            (0, new_minor)
        };

        let old_version = format!("{major}.{old_minor}.{old_patch}");
        let new_version = format!("{major}.{new_minor}.{new_patch}");

        let update_type = UpdateType::from_versions(&old_version, &new_version);

        // Should be Minor unless it's also a Major bump (which it isn't by construction)
        prop_assert_eq!(update_type, UpdateType::Minor);
    }

    #[test]
    fn prop_patch_bump_detected(
        major in 0u32..10u32,
        minor in 0u32..10u32,
        old_patch in 0u32..10u32,
        new_patch in 1u32..20u32
    ) {
        use omg_lib::cli::tea::UpdateType;

        // Ensure new_patch > old_patch
        let (old_patch, new_patch) = if old_patch < new_patch {
            (old_patch, new_patch)
        } else {
            (0, new_patch)
        };

        let old_version = format!("{major}.{minor}.{old_patch}");
        let new_version = format!("{major}.{minor}.{new_patch}");

        let update_type = UpdateType::from_versions(&old_version, &new_version);

        prop_assert_eq!(update_type, UpdateType::Patch);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PACMAN VERSION FORMAT PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_pacman_version_format(
        major in 0u32..50u32,
        minor in 0u32..50u32,
        patch in 0u32..50u32,
        pkgrel in 1u32..10u32
    ) {
        use omg_lib::cli::tea::UpdateType;

        let v1 = format!("{major}.{minor}.{patch}-{pkgrel}");
        let v2 = format!("{major}.{minor}.{}-{pkgrel}", patch + 1);

        let update_type = UpdateType::from_versions(&v1, &v2);

        prop_assert_eq!(update_type, UpdateType::Patch);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PACKAGE NAME VALIDATION PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #[test]
    fn prop_valid_package_names(
        name in "[a-z]{1,10}"
    ) {
        use omg_lib::core::security;

        // Valid package names should pass validation
        let result = security::validate_package_name(&name);
        prop_assert!(result.is_ok());
    }

    #[test]
    fn prop_shell_chars_rejected(
        base in "[a-z]{1,10}",
        shell_char in prop::sample::select(vec![';', '|', '&', '$', '`', '(', ')', '<', '>', '\n', '\r', '\t'])
    ) {
        use omg_lib::core::security;

        let name = format!("{}{}{}", base, shell_char, base);
        let result = security::validate_package_name(&name);

        // Should reject invalid characters
        prop_assert!(result.is_err());
    }

    #[test]
    fn prop_path_traversal_rejected(
        base in "[a-z]{1,10}",
        traversal in prop::sample::select(vec!["../", "..\\\\", "/../"])
    ) {
        use omg_lib::core::security;

        let name = format!("{}{}", base, traversal);
        let result = security::validate_package_name(&name);

        // Should reject path traversal
        prop_assert!(result.is_err());
    }
}

// Regular unit test for empty name (proptest requires at least one parameter)
#[test]
fn test_empty_name_rejected() {
    use omg_lib::core::security;

    let result = security::validate_package_name("");
    assert!(result.is_err());
}

#[test]
fn test_update_type_major_detection() {
    use omg_lib::cli::tea::UpdateType;
    assert_eq!(
        UpdateType::from_versions("1.0.0", "2.0.0"),
        UpdateType::Major
    );
}

#[test]
fn test_update_type_minor_detection() {
    use omg_lib::cli::tea::UpdateType;
    assert_eq!(
        UpdateType::from_versions("1.0.0", "1.1.0"),
        UpdateType::Minor
    );
}

#[test]
fn test_update_type_patch_detection() {
    use omg_lib::cli::tea::UpdateType;
    assert_eq!(
        UpdateType::from_versions("1.0.0", "1.0.1"),
        UpdateType::Patch
    );
}

#[test]
fn test_pacman_version_format() {
    use omg_lib::cli::tea::UpdateType;
    assert_eq!(
        UpdateType::from_versions("1.15.6-1", "1.15.8-1"),
        UpdateType::Patch
    );
}

#[test]
fn test_package_name_validation_valid() {
    use omg_lib::core::security;
    assert!(security::validate_package_name("firefox").is_ok());
    assert!(security::validate_package_name("vim").is_ok());
    assert!(security::validate_package_name("libfoo").is_ok());
}

#[test]
fn test_package_name_rejection_shell_chars() {
    use omg_lib::core::security;
    assert!(security::validate_package_name("foo;bar").is_err());
    assert!(security::validate_package_name("foo|bar").is_err());
    assert!(security::validate_package_name("foo$(bar)").is_err());
}

#[test]
fn test_package_name_rejection_path_traversal() {
    use omg_lib::core::security;
    assert!(security::validate_package_name("../../etc/passwd").is_err());
    assert!(security::validate_package_name("foo/../../bar").is_err());
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI ARGUMENT PARSING PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]

    #[test]
    fn prop_cli_never_crashes(
        command in "[a-z]{1,10}",
        args in "[^\x00]{0,100}"
    ) {
        let result = run_omg(&[&command, &args]);
        assert_process_completed(&result);
    }
}
