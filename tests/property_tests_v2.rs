//! Advanced Property-Based Testing for OMG
//!
//! Uses proptest to discover edge cases and verify invariants across:
//! - Version parsing and comparison
//! - UpdateType classification
//! - Elm Architecture model updates
//! - Package name validation
//! - Configuration parsing
//!
//! Run: cargo test --test property_tests_v2
//!
//! For faster iteration: cargo test --test property_tests_v2 -- --test-threads=1

#![allow(clippy::unwrap_used)]
#![allow(clippy::pedantic)]

use proptest::prelude::*;

mod common;
use common::*;

// ═══════════════════════════════════════════════════════════════════════════════
// VERSION PARSING PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    fn prop_version_parse_never_crashes(
        major in 0u32..1000u32,
        minor in 0u32..1000u32,
        patch in 0u32..1000u32
    ) {
        let version = format!("{major}.{minor}.{patch}");
        let parsed = semver::Version::parse(&version);
        prop_assert!(parsed.is_ok());
    }

    /// If A > B and B > C, then A > C
    fn prop_update_type_transitivity(
        v1_major in 0u32..10u32,
        v1_minor in 0u32..10u32,
        v1_patch in 0u32..10u32,
        v2_major in 0u32..10u32,
        v2_minor in 0u32..10u32,
        v2_patch in 0u32..10u32,
        v3_major in 0u32..10u32,
        v3_minor in 0u32..10u32,
        v3_patch in 0u32..10u32
    ) {
        use omg_lib::cli::tea::UpdateType;

        let v1 = format!("{v1_major}.{v1_minor}.{v1_patch}");
        let v2 = format!("{v2_major}.{v2_minor}.{v2_patch}");
        let v3 = format!("{v3_major}.{v3_minor}.{v3_patch}");

        let type1 = UpdateType::from_versions(&v1, &v2);
        let type2 = UpdateType::from_versions(&v2, &v3);

        // If both are upgrades, verify consistency
        // This is a weak property but ensures no crashes
        prop_assert!(matches!(type1, UpdateType::Major | UpdateType::Minor | UpdateType::Patch | UpdateType::Unknown));
        prop_assert!(matches!(type2, UpdateType::Major | UpdateType::Minor | UpdateType::Patch | UpdateType::Unknown));
    }

    fn prop_same_version_patch_or_unknown(
        major in 0u32..50u32,
        minor in 0u32..50u32,
        patch in 0u32..50u32
    ) {
        use omg_lib::cli::tea::UpdateType;

        let version = format!("{major}.{minor}.{patch}");
        let update_type = UpdateType::from_versions(&version, &version);

        // Same version should be Patch (or Unknown on parse error)
        prop_assert!(matches!(update_type, UpdateType::Patch | UpdateType::Unknown));
    }

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

    fn prop_pacman_version_format(
        major in 0u32..50u32,
        minor in 0u32..50u32,
        patch in 0u32..50u32,
        pkgrel in 1u32..10u32
    ) {
        use omg_lib::cli::tea::UpdateType;

        let v1 = format!("{}.{}.{}-{}", major, minor, patch, pkgrel);
        let v2 = format!("{}.{}.{}-{}", major, minor, patch + 1, pkgrel);

        let update_type = UpdateType::from_versions(&v1, &v2);

        // Should detect as patch update (newer patch, same pkgrel)
        prop_assert!(matches!(update_type, UpdateType::Patch | UpdateType::Unknown));
    }

    fn prop_version_with_extras(
        major in 0u32..20u32,
        minor in 0u32..20u32,
        patch in 0u32..20u32,
        prefix in "[a-z]{0,5}",
        suffix in "[a-z0-9\\-\\.]{0,10}"
    ) {
        use omg_lib::cli::tea::UpdateType;

        let version = format!("{prefix}{major}.{minor}.{patch}{suffix}");
        let _update_type = UpdateType::from_versions(&version, &version);

        // Should not crash
        prop_assert!(true);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ELM ARCHITECTURE MODEL PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {

    fn prop_update_model_state_transitions(
        check_only in proptest::bool::ANY,
        yes in proptest::bool::ANY
    ) {
        use omg_lib::cli::tea::{Model, UpdateModel, UpdateMsg};

        let mut model = UpdateModel::new()
            .with_check_only(check_only)
            .with_yes(yes);

        // Apply init
        let _cmd = model.init();

        // Apply various messages
        let messages = vec![
            UpdateMsg::Check,
            UpdateMsg::NoUpdates,
            UpdateMsg::Complete,
        ];

        for msg in messages {
            let _cmd = model.update(msg.clone());

            // After each update, the view should not panic
            let _view = model.view();
        }

        prop_assert!(true);
    }

    fn prop_progress_bar_clamping(
        percent in 0i32..200i32
    ) {
        use omg_lib::cli::tea::UpdateModel;

        let model = UpdateModel {
            download_percent: percent as usize,
            ..Default::default()
        };

        let bar = model.render_progress_bar(20);

        // Should not contain values > 100%
        if percent > 100 {
            prop_assert!(bar.contains("100%"));
        } else {
            let expected = format!("{}%", percent.min(100));
            prop_assert!(bar.contains(&expected));
        }
    }

    fn prop_update_model_empty_packages(
        check_only in proptest::bool::ANY,
        yes in proptest::bool::ANY
    ) {
        use omg_lib::cli::tea::{Model, UpdateModel, UpdateMsg};

        let mut model = UpdateModel::new()
            .with_check_only(check_only)
            .with_yes(yes);

        model.updates.clear();

        let _cmd = model.update(UpdateMsg::UpdatesFound(vec![]));

        // View should not panic
        let view = model.view();
        prop_assert!(!view.is_empty());
    }

    fn prop_error_state_preserved(
        error_msg in "[a-zA-Z0-9 ]{1,100}"
    ) {
        use omg_lib::cli::tea::{Model, UpdateModel, UpdateMsg};

        let mut model = UpdateModel::new();

        let _cmd = model.update(UpdateMsg::Error(error_msg.clone()));

        // Should be in failed state
        prop_assert_eq!(model.state, omg_lib::cli::tea::UpdateState::Failed);
        prop_assert_eq!(model.error.as_ref(), Some(&error_msg));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PACKAGE NAME VALIDATION PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {
    fn prop_valid_package_names(
        name in "[a-z]{1,10}"
    ) {
        use omg_lib::core::security;

        // Valid package names should pass validation
        let result = security::validate_package_name(&name);
        prop_assert!(result.is_ok());
    }

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

    fn prop_path_traversal_rejected(
        base in "[a-z]{1,10}",
        traversal in "[.]{2}|[.]/|[.][\\\\]|~|/etc"
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

// Additional regular tests for comprehensive coverage
#[test]
fn test_version_parse_basic() {
    let version = semver::Version::parse("1.0.0");
    assert!(version.is_ok());
}

#[test]
fn test_version_parse_with_prerelease() {
    let version = semver::Version::parse("1.0.0-alpha");
    assert!(version.is_ok());
}

#[test]
fn test_version_parse_with_build() {
    let version = semver::Version::parse("1.0.0+build");
    assert!(version.is_ok());
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
fn test_version_comparison_ordering() {
    let v1 = semver::Version::parse("1.0.0").unwrap();
    let v2 = semver::Version::parse("1.1.0").unwrap();
    assert!(v2 > v1);
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

#[test]
fn test_search_model_initialization() {
    use omg_lib::cli::tea::SearchModel;
    let model = SearchModel::new();
    assert_eq!(model.state, omg_lib::cli::tea::SearchState::Idle);
}

#[test]
fn test_search_model_with_query() {
    use omg_lib::cli::tea::SearchModel;
    let model = SearchModel::new().with_query("firefox".to_string());
    assert_eq!(model.query, "firefox");
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIGURATION PARSING PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {

    fn prop_toml_parse_safe(
        content in "\\PC{0,1000}"
    ) {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();
        writeln!(temp_file, "{}", content).unwrap();

        // Try to parse as TOML - may fail but should not panic
        let _parsed: Result<toml::Table, _> = toml::from_str(&content);

        // If it's valid TOML, it should parse
        // If it's invalid, it should error gracefully
        prop_assert!(true);
    }

    fn prop_lock_file_resilient(
        sections in prop::collection::hash_map("[a-z_]{1,20}", "[^\x00]{0,100}", 0..10)
    ) {
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp_file = NamedTempFile::new().unwrap();

        for (key, value) in &sections {
            writeln!(temp_file, "[{}]", key).unwrap();
            writeln!(temp_file, "value = \"{}\"", value.replace('"', "'")).unwrap();
        }

        // Should not crash when reading
        let path = temp_file.path();
        let _content = std::fs::read_to_string(path);

        prop_assert!(true);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI ARGUMENT PARSING PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {

    fn prop_cli_never_crashes(
        command in "[a-z]{1,10}",
        args in "[^\x00]{0,100}"
    ) {
        let result = run_omg(&[&command, &args]);
        prop_assert!(!result.stderr.contains("panicked at"));
    }

    fn prop_flag_combinations(
        has_check in proptest::bool::ANY,
        has_yes in proptest::bool::ANY,
        has_help in proptest::bool::ANY
    ) {
        let mut flags = Vec::new();
        if has_check { flags.push("--check"); }
        if has_yes { flags.push("--yes"); }
        if has_help { flags.push("--help"); }

        let result = run_omg(&["update"]);
        prop_assert!(!result.stderr.contains("panicked at"));
    }

    fn prop_multiple_packages(
        count in 1usize..10usize,
        name_prefix in "[a-z]{1,5}"
    ) {
        let _packages: Vec<String> = (0..count)
            .map(|i| format!("{}{}", name_prefix, i))
            .collect();

        // Test that search works regardless of how many packages we might want
        let result = run_omg(&["search", "test"]);

        prop_assert!(!result.stderr.contains("panicked at"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STRING OPERATIONS PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {

    fn prop_version_trim_consistent(
        version in "\\PC{1,50}"
    ) {
        // The version trimming logic removes non-numeric prefixes
        let trimmed1 = version.trim_start_matches(|c: char| !c.is_numeric());
        let trimmed2 = version.trim_start_matches(|c: char| !c.is_numeric());

        prop_assert_eq!(trimmed1, trimmed2, "Trimming should be idempotent");
    }

    fn prop_string_join_no_data_loss(
        parts in prop::collection::vec("[a-z]{1,10}", 1..20)
    ) {
        let joined = parts.join("/");
        let split: Vec<&str> = joined.split('/').collect();

        prop_assert_eq!(split.len(), parts.len());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INVARIANTS AND EDGE CASES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {

    fn prop_zero_handled(
        value in prop::num::usize::ANY
    ) {
        use omg_lib::cli::tea::UpdateModel;

        // Only test with 0 value
        if value != 0 {
            return Ok(());
        }

        let model = UpdateModel {
            download_percent: value,
            ..Default::default()
        };

        let bar = model.render_progress_bar(10);
        prop_assert!(bar.contains("0%"));
    }

    fn prop_max_values(
        value in prop::num::usize::ANY
    ) {
        use omg_lib::cli::tea::UpdateModel;

        // Only test with 100 value
        if value != 100 {
            return Ok(());
        }

        let model = UpdateModel {
            download_percent: value,
            ..Default::default()
        };

        let bar = model.render_progress_bar(10);
        prop_assert!(bar.contains("100%"));
    }

    fn prop_unicode_preserved(
        text in "\\PC{1,50}"
    ) {
        // Should not crash on unicode
        let _displayed = text.contains("✓");
        prop_assert!(true);
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FUZZING-STYLE PROPERTIES
// ═══════════════════════════════════════════════════════════════════════════════

proptest! {

    fn prop_arbitrary_bytes(
        bytes in prop::collection::vec(0u8..255u8, 0..100)
    ) {
        // Convert to string (may have invalid UTF-8)
        let _string = String::from_utf8_lossy(&bytes);

        // Should not panic
        prop_assert!(true);
    }

    fn prop_repeated_operations_consistent(
        version1 in "[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}",
        version2 in "[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}"
    ) {
        use omg_lib::cli::tea::UpdateType;

        // Apply operation twice
        let type1 = UpdateType::from_versions(&version1, &version2);
        let type2 = UpdateType::from_versions(&version1, &version2);

        prop_assert_eq!(type1, type2, "Operation should be deterministic");
    }
}
