#![cfg(feature = "arch")]
#![expect(clippy::unwrap_used, clippy::expect_used, clippy::pedantic)]
//! Integration tests for omg install command
//!
//! Tests both official and AUR package installation on Arch Linux

pub mod common;

use common::*;
use std::process::Command;

/// Remove a package using pacman
fn remove_package(pkg: &str) -> bool {
    Command::new("sudo")
        .args(["pacman", "-Rdd", "--noconfirm", pkg])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

#[test]
fn test_install_official_package() {
    require_system_tests!();
    require_arch!();

    // Use a small, safe package for testing
    let test_pkg = "ripgrep";

    // Skip if already installed
    if is_package_installed(test_pkg) {
        println!("Package {} already installed, skipping test", test_pkg);
        return;
    }

    // Try to install
    let result = run_omg(&["install", "-y", test_pkg]);

    // Cleanup
    if is_package_installed(test_pkg) {
        remove_package(test_pkg);
    }

    result.assert_success();
    assert!(
        result.contains("installed") || result.contains("success"),
        "Expected success message in output"
    );
}

#[test]
fn test_install_aur_package() {
    require_system_tests!();
    require_arch!();

    // Test with a small AUR package
    let test_pkg = "helium-browser-bin";

    // Skip if already installed
    if is_package_installed(test_pkg) {
        println!("Package {} already installed, skipping test", test_pkg);
        return;
    }

    // Try to install from AUR
    let result = run_omg(&["install", "-y", test_pkg]);

    // Cleanup
    if is_package_installed(test_pkg) {
        remove_package(test_pkg);
    }

    result.assert_success();
    assert!(
        result.contains("installed") || result.contains("success"),
        "Expected success message in output"
    );
}

#[test]
fn test_install_mixed_packages() {
    require_system_tests!();
    require_arch!();

    // Test installing both official and AUR packages in one command
    let official_pkg = "bat";
    let aur_pkg = "helium-browser-bin";

    // Skip if already installed
    let official_installed = is_package_installed(official_pkg);
    let aur_installed = is_package_installed(aur_pkg);

    if official_installed && aur_installed {
        println!("Packages already installed, skipping test");
        return;
    }

    // Try to install both
    let result = run_omg(&["install", "-y", official_pkg, aur_pkg]);

    // Cleanup
    if is_package_installed(official_pkg) && !official_installed {
        remove_package(official_pkg);
    }
    if is_package_installed(aur_pkg) && !aur_installed {
        remove_package(aur_pkg);
    }

    result.assert_success();
    assert!(
        result.contains("installed") || result.contains("success"),
        "Expected success message in output"
    );
}

#[test]
fn test_install_nonexistent_package() {
    require_system_tests!();
    require_arch!();

    let fake_pkg = "this-package-does-not-exist-12345";

    let result = run_omg(&["install", fake_pkg]);

    // Should fail
    result.assert_failure();

    // Should contain error message
    let output = result.combined_output();
    assert!(
        output.contains("not found") || output.contains("Package not found"),
        "Expected 'not found' in output, got: {}",
        output
    );
}
