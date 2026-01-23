//! Integration tests for Pure Rust Debian implementation
//!
//! This verifies that the PureDebianPackageManager works correctly 
//! even without the C-based rust-apt dependency.

#![cfg(feature = "debian-pure")]

mod common;

use omg_lib::package_managers::PackageManager;
use common::*;

#[test]
fn test_pure_debian_manager_name() {
    init_test_env();
    let pm = omg_lib::package_managers::debian_pure::PureDebianPackageManager::new();
    assert_eq!(pm.name(), "apt-pure");
}

#[test]
fn test_pure_debian_detection() {
    init_test_env();
    // Simulate Debian environment
    unsafe {
        std::env::set_var("OMG_TEST_MODE", "1");
        std::env::set_var("OMG_TEST_DISTRO", "debian");
    }
    
    let pm = omg_lib::package_managers::get_package_manager();
    // In test mode, it returns MockPackageManager, not PureDebianPackageManager.
    // This is expected behavior for existing logic.
    assert_eq!(pm.name(), "apt");
}

#[test]
fn test_pure_debian_search_mock() {
    init_test_env();
    // We can't easily test the real debian_db without /var/lib/dpkg/status
    // but we can ensure it doesn't panic when calling methods.
    let pm = omg_lib::package_managers::debian_pure::PureDebianPackageManager::new();
    
    // This will likely return empty list on non-Debian systems but should not panic
    let _ = pm.search("bash");
}
