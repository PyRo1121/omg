//! Integration tests for OMG CLI commands
//!
//! These tests require the arch feature as they test pacman-specific functionality.

#![cfg(feature = "arch")]

pub mod common;

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

// Falsifiable contract: `info <installed-package>` must succeed and render
// the package name plus a version token (src: `omg info` handler renders
// `<name> <version>` as its first line).
#[test]
fn test_info_pacman() {
    let package_name = "pacman";
    let result = run_omg(&["info", package_name]);
    common::assertions::assert_package_info(&result, package_name);
}

// Falsifiable contract: a missing package must FAIL and say so — the old
// `... || !result.success` disjunction passed even on unrelated failures.
#[test]
fn test_info_nonexistent_package() {
    use common::fixtures::packages::NONEXISTENT;

    // ===== ARRANGE =====
    let nonexistent_package = NONEXISTENT[0];

    // ===== ACT =====
    let result = run_omg(&["info", nonexistent_package]);

    // ===== ASSERT =====
    result.assert_failure();
    let combined = result.combined_output();
    assert!(
        combined.contains("not found") || combined.contains(nonexistent_package),
        "info of a missing package must name the failure. Got:\n{combined}"
    );
}

#[test]
fn test_list_explicit() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["explicit"]);

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

// Falsifiable contract: install --dry-run exits 0 without installing and
// previews BOTH the exact package and that no changes will be made.
#[test]
fn test_install_dry_run() {
    // ===== ARRANGE =====
    let package = "vim";

    // ===== ACT =====
    let result = run_omg(&["install", "--dry-run", package]);

    // ===== ASSERT =====
    result.assert_success();
    result.assert_stdout_contains(package);
    assert!(
        result.stdout_contains("dry run"),
        "Install preview should state dry-run mode. Got:\n{}",
        result.stdout
    );
}

// Falsifiable contract: remove --dry-run exits 0 without removing anything
// and previews BOTH the exact package and that no changes will be made.
#[test]
fn test_remove_dry_run() {
    // ===== ARRANGE =====
    let package = "pacman";

    // ===== ACT =====
    let result = run_omg(&["remove", "--dry-run", package]);

    // ===== ASSERT =====
    result.assert_success();
    result.assert_stdout_contains(package);
    assert!(
        result.stdout_contains("dry run"),
        "Remove preview should state dry-run mode. Got:\n{}",
        result.stdout
    );
}

// Falsifiable contract: doctor must run to completion (exit 0; individual
// check failures do not fail the command, src/cli/doctor.rs:38 `run` always
// returns Ok) and render its report header "OMG Doctor Checking system
// health...".
#[test]
fn test_doctor_command() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["doctor"]);

    // ===== ASSERT =====
    result.assert_success();
    result.assert_stdout_contains("Doctor");
    assert!(
        result.stdout_contains("health")
            || result.stdout_contains("✓")
            || result.stdout_contains("✗"),
        "Doctor should show check results. Got:\n{}",
        result.stdout
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
    assert!(
        result.stderr_contains("error") || result.stderr_contains("unrecognized"),
        "Invalid command should report an error. Got:\n{}",
        result.stderr
    );
}

/// Empty search returns all packages (valid behavior)
/// Falsifiable contract: empty query must succeed AND render the results
/// block; the old `success || !stdout.is_empty()` disjunction passed even
/// when search crashed with non-empty stderr.
#[test]
fn test_search_empty_query() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["search", ""]);

    // ===== ASSERT =====
    result.assert_success();
    result.assert_stdout_contains("Search Results");
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
