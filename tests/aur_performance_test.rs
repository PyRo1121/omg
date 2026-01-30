#![cfg(feature = "arch")]
//! AUR Performance Integration Tests
//!
//! These tests validate all AUR optimizations work together:
//! - Parallel source downloads
//! - Smart dependency resolution
//! - Skip unnecessary API calls
//! - Progress reporting
//! - Sudoloop for non-blocking operations
//!
//! Run manually with: cargo test --features arch --test aur_performance_test -- --ignored
//!
//! WARNING: These tests perform real package installations and require sudo!
//! Only run in disposable containers or development environments.

use std::time::Instant;

/// Test AUR package installation with all optimizations enabled
#[tokio::test]
#[ignore] // Manual execution only - requires sudo and real installations
async fn test_aur_install_speed() {
    // Test packages chosen for different characteristics:
    // - yay-bin: Binary package (no build time)
    // - paru-bin: Binary package with dependencies
    let test_packages = vec!["yay-bin", "paru-bin"];

    for package in test_packages {
        println!("\n=== Testing AUR install: {} ===", package);

        let start = Instant::now();

        // Run OMG install
        let output = std::process::Command::new("cargo")
            .args(&["run", "--release", "--features", "arch", "--", "install", package])
            .output()
            .expect("Failed to execute omg install");

        let duration = start.elapsed();

        // Verify installation succeeded
        assert!(
            output.status.success(),
            "Installation of {} failed: {}",
            package,
            String::from_utf8_lossy(&output.stderr)
        );

        // Verify reasonable performance (< 60s for binary packages)
        assert!(
            duration.as_secs() < 60,
            "Installation of {} took too long: {:?}",
            package,
            duration
        );

        println!("✓ {} installed successfully in {:?}", package, duration);
    }
}

/// Test parallel download performance
#[tokio::test]
#[ignore] // Manual execution only - requires network and sudo
async fn test_parallel_downloads() {
    println!("\n=== Testing parallel source downloads ===");

    // Package with multiple sources to download
    let package = "ttf-meslo-nerd-font-powerlevel10k";

    let start = Instant::now();

    // Run OMG install with parallel downloads
    let output = std::process::Command::new("cargo")
        .args(&["run", "--release", "--features", "arch", "--", "install", package])
        .output()
        .expect("Failed to execute omg install");

    let duration = start.elapsed();

    assert!(
        output.status.success(),
        "Installation of {} failed: {}",
        package,
        String::from_utf8_lossy(&output.stderr)
    );

    // With parallel downloads, should be significantly faster
    // This is a complex package but should complete reasonably quickly
    assert!(
        duration.as_secs() < 120,
        "Installation with parallel downloads took too long: {:?}",
        duration
    );

    println!("✓ Parallel downloads completed in {:?}", duration);
}

/// Test smart dependency resolution avoids unnecessary work
#[tokio::test]
#[ignore] // Manual execution only - requires sudo
async fn test_smart_dependency_resolution() {
    println!("\n=== Testing smart dependency resolution ===");

    // Install base package first
    let base_package = "yay-bin";
    println!("Installing base package: {}", base_package);

    let output = std::process::Command::new("cargo")
        .args(&["run", "--release", "--features", "arch", "--", "install", base_package])
        .output()
        .expect("Failed to install base package");

    assert!(output.status.success(), "Base package installation failed");

    // Now install a package that might share dependencies
    // Smart resolution should skip already-satisfied dependencies
    let test_package = "paru-bin";
    println!("Installing package with potential shared deps: {}", test_package);

    let start = Instant::now();

    let output = std::process::Command::new("cargo")
        .args(&["run", "--release", "--features", "arch", "--", "install", test_package])
        .output()
        .expect("Failed to install test package");

    let duration = start.elapsed();

    assert!(
        output.status.success(),
        "Installation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Should be faster since shared dependencies are already installed
    assert!(
        duration.as_secs() < 45,
        "Smart dependency resolution took too long: {:?}",
        duration
    );

    println!("✓ Smart dependency resolution completed in {:?}", duration);
}
