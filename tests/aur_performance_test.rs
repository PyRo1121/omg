#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! AUR Performance Integration Tests
//!
//! These tests validate all AUR optimizations work together:
//! - Parallel source downloads
//! - Smart dependency resolution
//! - Skip unnecessary API calls
//! - Progress reporting
//! - Sudoloop for non-blocking operations
//!
//! Run manually with: `OMG_RUN_SYSTEM_TESTS=1 cargo test --features arch --test aur_performance_test`
//!
//! WARNING: These tests perform real package installations and require sudo!
//! Only run in disposable containers or development environments.

#![cfg(feature = "arch")]

mod common;

use common::*;
use std::time::{Duration, Instant};

/// Test AUR package installation with all optimizations enabled
#[tokio::test]
async fn test_aur_install_speed() {
    require_system_tests!();
    require_arch!();

    // Test packages chosen for different characteristics:
    // - yay-bin: Binary package (no build time)
    // - paru-bin: Binary package with dependencies
    let test_packages = vec!["yay-bin", "paru-bin"];

    for package in test_packages {
        println!("\n=== Testing AUR install: {package} ===");

        let start = Instant::now();

        // Run OMG install
        let result = run_omg(&["install", package]);

        let duration = start.elapsed();

        // Verify installation succeeded
        result.assert_success();

        // Verify reasonable performance (< 60s for binary packages)
        assert!(
            duration.as_secs() < 60,
            "Installation of {package} took too long: {duration:?}"
        );

        println!("✓ {package} installed successfully in {duration:?}");
    }
}

/// Test parallel download performance
#[tokio::test]
async fn test_parallel_downloads() {
    require_system_tests!();
    require_arch!();

    println!("\n=== Testing parallel source downloads ===");

    // Package with multiple sources to download
    let package = "ttf-meslo-nerd-font-powerlevel10k";

    let start = Instant::now();

    // Run OMG install with parallel downloads
    let result = run_omg(&["install", package]);

    let duration = start.elapsed();

    result.assert_success();

    // With parallel downloads, should be significantly faster
    // This is a complex package but should complete reasonably quickly
    assert!(
        duration.as_secs() < 120,
        "Installation with parallel downloads took too long: {duration:?}"
    );

    println!("✓ Parallel downloads completed in {duration:?}");
}

/// Test smart dependency resolution avoids unnecessary work
#[tokio::test]
async fn test_smart_dependency_resolution() {
    require_system_tests!();
    require_arch!();

    println!("\n=== Testing smart dependency resolution ===");

    // Install base package first
    let base_package = "yay-bin";
    println!("Installing base package: {base_package}");

    let result = run_omg(&["install", base_package]);
    result.assert_success();

    // Now install a package that might share dependencies
    // Smart resolution should skip already-satisfied dependencies
    let test_package = "paru-bin";
    println!("Installing package with potential shared deps: {test_package}");

    let start = Instant::now();

    let result = run_omg(&["install", test_package]);

    let duration = start.elapsed();

    result.assert_success();

    // Should be faster since shared dependencies are already installed
    assert!(
        duration.as_secs() < 45,
        "Smart dependency resolution took too long: {duration:?}"
    );

    println!("✓ Smart dependency resolution completed in {duration:?}");
}

/// Test performance reporting and progress indicators
#[tokio::test]
async fn test_progress_reporting() {
    require_system_tests!();
    require_arch!();

    println!("\n=== Testing progress reporting ===");

    let package = "yay-bin";

    let result = run_omg(&["install", package]);
    result.assert_success();

    // Check that output includes progress indicators or status messages
    // The exact format may vary, but we should see some indication of progress
    let output = result.combined_output();

    // Look for typical progress indicators
    let has_progress = output.contains("Installing")
        || output.contains("Downloading")
        || output.contains("Building")
        || output.contains("→")
        || output.contains("✓");

    assert!(
        has_progress,
        "Expected progress indicators in output, but found none"
    );

    println!("✓ Progress reporting working correctly");
}

/// Test that performance meets target thresholds
#[tokio::test]
async fn test_performance_benchmarks() {
    require_system_tests!();
    require_arch!();

    println!("\n=== Testing performance benchmarks ===");

    let benchmarks = vec![
        ("yay-bin", Duration::from_secs(45)), // Binary package should be fast
        ("paru-bin", Duration::from_secs(45)),
    ];

    for (package, max_duration) in benchmarks {
        println!("Benchmarking {package}...");

        let start = Instant::now();
        let result = run_omg(&["install", package]);
        let duration = start.elapsed();

        result.assert_success();
        result.assert_duration_under(max_duration);

        println!("✓ {package}: {duration:?} (target: {max_duration:?})");
    }
}

/// Test installation of multiple packages in sequence
#[tokio::test]
async fn test_sequential_installations() {
    require_system_tests!();
    require_arch!();

    println!("\n=== Testing sequential installations ===");

    let packages = vec!["yay-bin", "paru-bin"];

    let start = Instant::now();

    for package in &packages {
        println!("Installing {package}...");
        let result = run_omg(&["install", package]);
        result.assert_success();
    }

    let total_duration = start.elapsed();

    // With optimizations, sequential installs should complete in reasonable time
    // Even with 2 packages, should finish in < 90s total
    assert!(
        total_duration.as_secs() < 90,
        "Sequential installations took too long: {total_duration:?}"
    );

    println!("✓ Sequential installations completed in {total_duration:?}");
}

/// Test error handling doesn't significantly impact performance
#[tokio::test]
async fn test_error_handling_performance() {
    require_system_tests!();
    require_arch!();

    println!("\n=== Testing error handling performance ===");

    // Try to install a non-existent package
    let start = Instant::now();
    let result = run_omg(&["install", "this-package-does-not-exist-xyz123"]);
    let duration = start.elapsed();

    // Should fail quickly
    result.assert_failure();

    // Error handling should be fast (< 10s)
    assert!(
        duration.as_secs() < 10,
        "Error handling took too long: {duration:?}"
    );

    println!("✓ Error handling completed in {duration:?}");
}
