#![cfg(feature = "debian-pure")]
#![expect(clippy::unwrap_used, clippy::expect_used, clippy::pedantic)]
//! Integration tests for Pure Rust Debian implementation
//!
//! This verifies that the `PureDebianPackageManager` works correctly
//! even without the C-based rust-apt dependency.

pub mod common;

use common::*;
use omg_lib::package_managers::PackageManager;

#[test]
fn test_pure_debian_manager_name() {
    init_test_env();
    let pm = omg_lib::package_managers::debian_pure::PureDebianPackageManager::new();
    assert_eq!(pm.name(), "apt-pure");
}

#[test]
fn test_debian_backend_decision() {
    // Assert the resolver's backend decisions directly instead of exercising
    // mock state wiring through process-global environment overrides.
    use omg_lib::package_managers::mock::backend_name_for_distro;

    assert_eq!(backend_name_for_distro("debian"), "apt");
    assert_eq!(backend_name_for_distro("ubuntu"), "apt");

    // The pure backend keeps its own identity when selected for debian-pure.
    let pure = omg_lib::package_managers::debian_pure::PureDebianPackageManager::new();
    assert_eq!(pure.name(), "apt-pure");
}

#[test]
fn test_pure_debian_search_mock() {
    init_test_env();
    // We can't easily test the real debian_db without /var/lib/dpkg/status
    // but we can ensure it doesn't panic when calling methods.
    let pm = omg_lib::package_managers::debian_pure::PureDebianPackageManager::new();

    // This will likely return empty list on non-Debian systems but should not panic
    let _result = pm.search("bash");
}
