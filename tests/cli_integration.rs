//! Integration tests for OMG CLI commands
//!
//! These tests require the arch feature as they test pacman-specific functionality.

#![cfg(feature = "arch")]

mod common;

use common::*;

#[test]
fn test_help_command() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["--help"]);

    // ===== ASSERT =====
    result.assert_success();
    result.assert_stdout_contains("omg");
    result.assert_stdout_contains("search");
    result.assert_stdout_contains("install");
}

#[test]
fn test_version_command() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["--version"]);

    // ===== ASSERT =====
    result.assert_success();
    result.assert_stdout_contains("omg");
}

#[test]
fn test_search_help() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["search", "--help"]);

    // ===== ASSERT =====
    result.assert_success();
    assert!(result.stdout_contains("search") || result.stdout_contains("Search"));
}

#[test]
fn test_search_pacman() {
    // ===== ARRANGE =====
    let query = "pacman";

    // ===== ACT =====
    let result = run_omg(&["search", query]);

    // ===== ASSERT =====
    result.assert_success();
    result.assert_stdout_contains(query);
}

#[test]
fn test_info_pacman() {
    let package_name = "pacman";
    let result = run_omg(&["info", package_name]);
    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        result.success || combined.contains("not found"),
        "Expected success or 'not found'. Got exit {}: {}",
        result.exit_code,
        combined
    );
}

#[test]
fn test_info_nonexistent_package() {
    use common::fixtures::packages::NONEXISTENT;

    // ===== ARRANGE =====
    let nonexistent_package = NONEXISTENT[0];

    // ===== ACT =====
    let result = run_omg(&["info", nonexistent_package]);

    // ===== ASSERT =====
    let combined = result.combined_output();
    assert!(
        combined.contains("not found") || combined.contains("No package") || !result.success,
        "Should indicate package not found"
    );
}

#[test]
fn test_list_explicit() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["list", "--explicit"]);

    // ===== ASSERT =====
    result.assert_success();
    assert!(!result.stdout.is_empty(), "Should list explicit packages");
}

#[test]
fn test_status_command() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["status"]);

    // ===== ASSERT =====
    result.assert_success();
    assert!(
        result.stdout_contains("package")
            || result.stdout_contains("Package")
            || result.stdout_contains("installed"),
        "Status should show package information"
    );
}

#[test]
fn test_install_dry_run() {
    // ===== ARRANGE =====
    let package = "vim";

    // ===== ACT =====
    let result = run_omg(&["install", "--dry-run", package]);

    // ===== ASSERT =====
    let combined = result.combined_output();
    assert!(
        combined.contains(package) || combined.contains("dry") || combined.contains("Dry"),
        "Dry run should mention the package or dry run mode"
    );
}

#[test]
fn test_remove_dry_run() {
    // ===== ARRANGE =====
    let package = "pacman";

    // ===== ACT =====
    let result = run_omg(&["remove", "--dry-run", package]);

    // ===== ASSERT =====
    let combined = result.combined_output();
    assert!(
        combined.contains(package) || combined.contains("dry") || combined.contains("Dry"),
        "Dry run should mention the package or dry run mode"
    );
}

#[test]
fn test_doctor_command() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["doctor"]);

    // ===== ASSERT =====
    assert!(
        result.stdout_contains("✓")
            || result.stdout_contains("✗")
            || result.stdout_contains("check")
            || result.stdout_contains("Check"),
        "Doctor should show check results"
    );
}

#[test]
fn test_invalid_command() {
    // ===== ARRANGE =====
    let invalid_cmd = "this-is-not-a-valid-command";

    // ===== ACT =====
    let result = run_omg(&[invalid_cmd]);

    // ===== ASSERT =====
    result.assert_failure();
}

/// Empty search returns all packages (valid behavior)
#[test]
fn test_search_empty_query() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["search", ""]);

    // ===== ASSERT =====
    // Empty search is valid - returns all packages
    // Just verify it doesn't crash and produces output
    assert!(
        result.success || !result.stdout.is_empty(),
        "Empty search should succeed or produce output"
    );
}

#[test]
fn test_verbose_flag() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["-v", "status"]);

    // ===== ASSERT =====
    result.assert_success();
}

#[test]
fn test_double_verbose_flag() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["-vv", "status"]);

    // ===== ASSERT =====
    result.assert_success();
}
