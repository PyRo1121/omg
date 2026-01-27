//! Integration tests for OMG CLI commands
//!
//! These tests require the arch feature as they test pacman-specific functionality.

#![cfg(feature = "arch")]

use std::process::Command;

fn omg_cmd() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omg"));
    cmd.env("NO_COLOR", "1");
    cmd
}

#[test]
fn test_help_command() {
    // ===== ARRANGE =====
    let mut cmd = omg_cmd();
    cmd.arg("--help");

    // ===== ACT =====
    let output = cmd.output().expect("Failed to run omg");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // ===== ASSERT =====
    assert!(output.status.success());
    assert!(stdout.contains("omg"));
    assert!(stdout.contains("search"));
    assert!(stdout.contains("install"));
}

#[test]
fn test_version_command() {
    // ===== ARRANGE =====
    let mut cmd = omg_cmd();
    cmd.arg("--version");

    // ===== ACT =====
    let output = cmd.output().expect("Failed to run omg");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // ===== ASSERT =====
    assert!(output.status.success());
    assert!(stdout.contains("omg"));
}

#[test]
fn test_search_help() {
    // ===== ARRANGE =====
    let mut cmd = omg_cmd();
    cmd.args(["search", "--help"]);

    // ===== ACT =====
    let output = cmd.output().expect("Failed to run omg");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // ===== ASSERT =====
    assert!(output.status.success());
    assert!(stdout.contains("search") || stdout.contains("Search"));
}

#[test]
fn test_search_pacman() {
    // ===== ARRANGE =====
    let query = "pacman";
    let mut cmd = omg_cmd();
    cmd.args(["search", query]);

    // ===== ACT =====
    let output = cmd.output().expect("Failed to run omg");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // ===== ASSERT =====
    assert!(output.status.success());
    assert!(
        stdout.contains(query),
        "Search for '{query}' should return results containing '{query}'"
    );
}

#[test]
fn test_info_pacman() {
    // ===== ARRANGE =====
    let package_name = "pacman";
    let mut cmd = omg_cmd();
    cmd.args(["info", package_name]);

    // ===== ACT =====
    let output = cmd.output().expect("Failed to run omg");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // ===== ASSERT =====
    assert!(output.status.success());
    assert!(stdout.contains(package_name));
}

#[test]
fn test_info_nonexistent_package() {
    // ===== ARRANGE =====
    let nonexistent_package = "this-package-definitely-does-not-exist-12345";
    let mut cmd = omg_cmd();
    cmd.args(["info", nonexistent_package]);

    // ===== ACT =====
    let output = cmd.output().expect("Failed to run omg");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // ===== ASSERT =====
    assert!(
        combined.contains("not found")
            || combined.contains("No package")
            || !output.status.success(),
        "Should indicate package not found"
    );
}

#[test]
fn test_list_explicit() {
    let output = omg_cmd()
        .args(["list", "--explicit"])
        .output()
        .expect("Failed to run omg");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.is_empty(), "Should list explicit packages");
}

#[test]
fn test_status_command() {
    let output = omg_cmd().arg("status").output().expect("Failed to run omg");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("package") || stdout.contains("Package") || stdout.contains("installed"),
        "Status should show package information"
    );
}

#[test]
fn test_install_dry_run() {
    let output = omg_cmd()
        .args(["install", "--dry-run", "vim"])
        .output()
        .expect("Failed to run omg");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("vim") || combined.contains("dry") || combined.contains("Dry"),
        "Dry run should mention the package or dry run mode"
    );
}

#[test]
fn test_remove_dry_run() {
    let output = omg_cmd()
        .args(["remove", "--dry-run", "pacman"])
        .output()
        .expect("Failed to run omg");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("pacman") || combined.contains("dry") || combined.contains("Dry"),
        "Dry run should mention the package or dry run mode"
    );
}

#[test]
fn test_doctor_command() {
    let output = omg_cmd().arg("doctor").output().expect("Failed to run omg");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("✓")
            || stdout.contains("✗")
            || stdout.contains("check")
            || stdout.contains("Check"),
        "Doctor should show check results"
    );
}

#[test]
fn test_invalid_command() {
    let output = omg_cmd()
        .arg("this-is-not-a-valid-command")
        .output()
        .expect("Failed to run omg");

    assert!(!output.status.success(), "Invalid command should fail");
}

/// Empty search returns all packages (valid behavior)
#[test]
fn test_search_empty_query() {
    let output = omg_cmd()
        .args(["search", ""])
        .output()
        .expect("Failed to run omg");

    // Empty search is valid - returns all packages
    // Just verify it doesn't crash and produces output
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() || !stdout.is_empty(),
        "Empty search should succeed or produce output"
    );
}

#[test]
fn test_verbose_flag() {
    let output = omg_cmd()
        .args(["-v", "status"])
        .output()
        .expect("Failed to run omg");

    assert!(output.status.success());
}

#[test]
fn test_double_verbose_flag() {
    let output = omg_cmd()
        .args(["-vv", "status"])
        .output()
        .expect("Failed to run omg");

    assert!(output.status.success());
}
