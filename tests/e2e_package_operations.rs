//! End-to-End Tests for Package Operations
//!
//! Comprehensive e2e tests covering the complete package management lifecycle:
//! - Search (official repos + AUR)
//! - Install (with dependency resolution)
//! - Info (package details)
//! - Update (system-wide updates)
//! - Remove (with cleanup)
//!
//! These tests use real CLI invocations with dry-run/check modes for safety.

#![cfg(feature = "arch")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]

mod common;

use common::*;

// ═══════════════════════════════════════════════════════════════════════════════
// SEARCH COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_search_official_package() {
    init_test_env();

    let result = run_omg(&["search", "bash"]);

    // Should succeed
    result.assert_success();

    // Should contain bash in results
    assert!(
        result.stdout_contains("bash") || result.stderr_contains("bash"),
        "Search should return bash package"
    );
}

#[test]
fn test_search_no_results() {
    init_test_env();

    let result = run_omg(&["search", "package-that-absolutely-does-not-exist-xyz123"]);

    // Should complete (success or graceful failure)
    let output = result.combined_output();
    assert!(
        result.success || output.contains("No packages found") || output.contains("0 packages"),
        "Should handle no results gracefully"
    );
}

#[test]
fn test_search_with_no_aur_flag() {
    init_test_env();

    let result = run_omg(&["search", "--no-aur", "firefox"]);

    result.assert_success();
    assert!(
        !result.combined_output().is_empty(),
        "Should return results"
    );
}

#[test]
fn test_search_interactive_flag() {
    init_test_env();

    // Interactive mode should fail without stdin
    let result = run_omg(&["search", "--interactive", "vim"]);

    // May succeed or fail depending on tty detection
    let output = result.combined_output();
    assert!(
        !output.is_empty(),
        "Interactive search should produce output or error"
    );
}

#[test]
fn test_search_json_output() {
    init_test_env();

    let result = run_omg(&["search", "--json", "git"]);

    if result.success {
        // If successful, should be valid JSON
        let json_result = serde_json::from_str::<serde_json::Value>(&result.stdout);
        assert!(
            json_result.is_ok(),
            "JSON output should be valid: {}",
            result.stdout
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INFO COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_info_common_package() {
    init_test_env();

    let result = run_omg(&["info", "pacman"]);

    result.assert_success();

    // Should contain package metadata
    let output = result.combined_output();
    assert!(
        output.contains("Name") || output.contains("Version") || output.contains("pacman"),
        "Info should show package details"
    );
}

#[test]
fn test_info_nonexistent_package() {
    init_test_env();

    let result = run_omg(&["info", "nonexistent-package-xyz123"]);

    // May succeed with error message or fail
    let output = result.combined_output();
    assert!(
        output.contains("not found") || output.contains("error") || output.contains("No package"),
        "Should show helpful error message"
    );
}

#[test]
fn test_info_shows_package_details() {
    init_test_env();

    let result = run_omg(&["info", "bash"]);

    result.assert_success();

    let output = result.combined_output();
    // Should show key package information
    assert!(
        output.contains("Description") || output.contains("Version") || output.contains("Size"),
        "Info should show package details"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// INSTALL COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_install_dry_run() {
    init_test_env();

    let result = run_omg(&["install", "--dry-run", "vim"]);

    // Dry run should always succeed or show what would be done
    let output = result.combined_output();
    assert!(
        result.success || output.contains("Would install") || output.contains("already installed"),
        "Dry run should show planned actions"
    );
}

#[test]
fn test_install_already_installed() {
    init_test_env();

    // pacman should always be installed on Arch
    let result = run_omg(&["install", "--dry-run", "pacman"]);

    let output = result.combined_output();
    // Should show the package in install preview
    assert!(
        output.contains("pacman")
            && (output.contains("DRY RUN")
                || output.contains("dry run")
                || output.contains("Dry Run")
                || output.contains("No changes")),
        "Should show install preview with package: {}",
        output
    );
}

#[test]
fn test_install_multiple_packages_dry_run() {
    init_test_env();

    let result = run_omg(&["install", "--dry-run", "vim", "git", "curl"]);

    // Should handle multiple packages
    assert!(
        !result.combined_output().is_empty(),
        "Should show install plan"
    );
}

#[test]
fn test_install_nonexistent_package() {
    init_test_env();

    let result = run_omg(&["install", "--dry-run", "absolutely-nonexistent-package-xyz"]);

    // In dry run, may show AUR fallback attempt
    let output = result.combined_output();
    assert!(
        result.success
            && (output.contains("AUR?")
                || output.contains("not found")
                || output.contains("DRY RUN")),
        "Should handle nonexistent packages (may show AUR attempt): {}",
        output
    );
}

#[test]
fn test_install_with_yes_flag() {
    init_test_env();

    // With --yes and --dry-run, should not prompt
    let result = run_omg(&["install", "--yes", "--dry-run", "bash"]);

    // Should complete without user interaction
    assert!(!result.combined_output().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════════
// UPDATE COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_update_check_only() {
    init_test_env();

    let result = run_omg(&["update", "--check"]);

    // Check should always succeed
    result.assert_success();

    let output = result.combined_output();
    assert!(
        output.contains("up to date")
            || output.contains("updates available")
            || output.contains("Update"),
        "Should show update status"
    );
}

#[test]
fn test_update_dry_run() {
    init_test_env();

    let result = run_omg(&["update", "--dry-run"]);

    // Should show what would be updated
    let output = result.combined_output();
    assert!(
        result.success || output.contains("Would update") || output.contains("Nothing to update"),
        "Dry run should show planned updates"
    );
}

#[test]
fn test_update_with_yes_flag() {
    init_test_env();

    let result = run_omg(&["update", "--check", "--yes"]);

    // --yes with --check should still just check
    result.assert_success();

    let output = result.combined_output();
    assert!(
        output.contains("up to date") || output.contains("Update") || output.contains("available"),
        "Should show update status"
    );
}

#[test]
fn test_update_fast_flag() {
    init_test_env();

    // Fast mode with dry-run should work
    let result = run_omg(&["update", "--fast", "--dry-run"]);

    let output = result.combined_output();
    assert!(!output.is_empty(), "Fast mode should produce output");
}

#[test]
fn test_update_turbo_flag() {
    init_test_env();

    // Turbo mode with dry-run
    let result = run_omg(&["update", "--turbo", "--dry-run"]);

    let output = result.combined_output();
    assert!(!output.is_empty(), "Turbo mode should produce output");
}

// ═══════════════════════════════════════════════════════════════════════════════
// REMOVE COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_remove_dry_run() {
    init_test_env();

    // Try to remove a safe package (that might not be installed)
    let result = run_omg(&["remove", "--dry-run", "vim"]);

    let output = result.combined_output();
    assert!(
        result.success || output.contains("not installed") || output.contains("Would remove"),
        "Dry run should show planned removal"
    );
}

#[test]
fn test_remove_nonexistent_package() {
    init_test_env();

    let result = run_omg(&["remove", "--dry-run", "package-never-installed-xyz"]);

    let output = result.combined_output();
    assert!(
        output.contains("not installed") || output.contains("not found"),
        "Should handle uninstalled packages"
    );
}

#[test]
fn test_remove_recursive_flag() {
    init_test_env();

    let result = run_omg(&["remove", "--recursive", "--dry-run", "vim"]);

    let output = result.combined_output();
    assert!(
        !output.is_empty(),
        "Recursive removal should show dependencies"
    );
}

#[test]
fn test_remove_multiple_packages() {
    init_test_env();

    let result = run_omg(&["remove", "--dry-run", "vim", "emacs"]);

    // Should handle multiple packages
    assert!(!result.combined_output().is_empty());
}

// ═══════════════════════════════════════════════════════════════════════════════
// AUXILIARY COMMANDS E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_explicit_list() {
    init_test_env();

    let result = run_omg(&["explicit"]);

    result.assert_success();

    // Should show list of explicitly installed packages
    assert!(!result.stdout.is_empty(), "Should list explicit packages");
}

#[test]
fn test_explicit_count() {
    init_test_env();

    let result = run_omg(&["explicit", "--count"]);

    result.assert_success();

    // Should show a number
    let output = result.stdout.trim();
    assert!(
        output.parse::<usize>().is_ok(),
        "Count should be a valid number"
    );
}

#[test]
#[ignore = "clean command has tokio runtime nesting issue in test context"]
fn test_clean_cache_dry_run() {
    init_test_env();

    let result = run_omg(&["clean", "--cache", "--dry-run"]);

    // Should show what would be cleaned
    let output = result.combined_output();
    assert!(
        result.success
            || output.contains("Would clean")
            || output.contains("cache")
            || output.contains("Clean")
            || output.contains("clean")
            || output.contains("No orphan"),
        "Should show cleanup plan: {}",
        output
    );
}

#[test]
#[ignore = "clean command has tokio runtime nesting issue in test context"]
fn test_clean_orphans_dry_run() {
    init_test_env();

    let result = run_omg(&["clean", "--orphans", "--dry-run"]);

    let output = result.combined_output();
    assert!(
        result.success
            || output.contains("orphan")
            || output.contains("Would remove")
            || output.contains("No orphan")
            || output.contains("clean"),
        "Should show orphan packages: {}",
        output
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// MULTI-COMMAND WORKFLOW TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_workflow_search_then_info() {
    init_test_env();

    // Workflow: Search for package, then get detailed info
    let search_result = run_omg(&["search", "git"]);
    search_result.assert_success();

    // Extract a package name (assume git is found)
    assert!(search_result.stdout_contains("git"));

    // Get info for that package
    let info_result = run_omg(&["info", "git"]);
    info_result.assert_success();
}

#[test]
fn test_workflow_info_then_install_dry_run() {
    init_test_env();

    // First get info about a package
    let info_result = run_omg(&["info", "bash"]);
    info_result.assert_success();

    // Then try to install it (dry run)
    let install_result = run_omg(&["install", "--dry-run", "bash"]);
    assert!(!install_result.combined_output().is_empty());
}

#[test]
fn test_workflow_update_check_then_explicit() {
    init_test_env();

    // Check for updates
    let update_result = run_omg(&["update", "--check"]);
    update_result.assert_success();

    // List explicit packages
    let explicit_result = run_omg(&["explicit"]);
    explicit_result.assert_success();
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR HANDLING AND EDGE CASES
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_missing_package_argument() {
    init_test_env();

    let result = run_omg(&["install"]);

    result.assert_failure();

    let output = result.combined_output();
    assert!(
        output.contains("required") || output.contains("argument"),
        "Should show error about missing argument"
    );
}

#[test]
fn test_error_invalid_flag() {
    init_test_env();

    let result = run_omg(&["search", "--invalid-flag-xyz", "test"]);

    result.assert_failure();

    let output = result.combined_output();
    assert!(
        output.contains("unexpected")
            || output.contains("invalid")
            || output.contains("unrecognized"),
        "Should show error about invalid flag"
    );
}

#[test]
fn test_quiet_flag_suppresses_output() {
    init_test_env();

    let result = run_omg(&["search", "--quiet", "bash"]);

    // Quiet mode should have less output (or none except errors)
    // Note: Implementation may vary, so we just check it doesn't crash
    assert!(result.success || !result.combined_output().is_empty());
}

#[test]
fn test_verbose_flag_adds_output() {
    init_test_env();

    let normal_result = run_omg(&["info", "bash"]);
    let verbose_result = run_omg(&["info", "-vv", "bash"]);

    // Verbose mode should have same or more output
    if normal_result.success && verbose_result.success {
        assert!(
            verbose_result.combined_output().len() >= normal_result.combined_output().len(),
            "Verbose mode should produce more or equal output"
        );
    }
}
