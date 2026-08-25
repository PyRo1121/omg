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
    // In test mode (OMG_TEST_MODE=1, set by init_test_env) the pure backend's
    // search returns a deterministic stub result set instead of requiring
    // /var/lib/dpkg-status (see debian_db::search_fast). The contract under
    // test: search parses into domain Packages, never panics, and surfaces a
    // match with fully populated metadata.
    let pm = omg_lib::package_managers::debian_pure::PureDebianPackageManager::new();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let results = rt.block_on(pm.search("bash")).expect("search must succeed");
    assert!(
        !results.is_empty(),
        "test-mode search must return the stub match"
    );
    assert!(
        results.iter().any(|package| package.name == "apt"),
        "expected the 'apt' stub package in test-mode results, got: {results:?}"
    );
}
