//! Logic tests for package manager abstraction
//!
//! These tests verify that the package manager abstraction works across
//! different simulated distributions using the OMG_TEST_DISTRO override.

mod common;

use crate::common::*;
use omg_lib::package_managers::get_package_manager;

use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_distro_override_arch() {
    init_test_env();
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("OMG_TEST_DISTRO", "arch");
    }

    let pm = get_package_manager();
    assert_eq!(pm.name(), "pacman");
}

#[test]
#[cfg(feature = "debian")]
fn test_distro_override_debian() {
    init_test_env();
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("OMG_TEST_DISTRO", "debian");
    }

    let pm = get_package_manager();
    assert_eq!(pm.name(), "apt");
}

#[test]
#[cfg(feature = "debian")]
fn test_distro_override_ubuntu() {
    init_test_env();
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("OMG_TEST_DISTRO", "ubuntu");
    }

    let pm = get_package_manager();
    assert_eq!(pm.name(), "apt");
}

#[tokio::test]
async fn test_mock_package_manager_logic() {
    use crate::common::mocks::{MockPackage, MockPackageDb, MockPackageManager};
    use omg_lib::package_managers::PackageManager;

    let db = MockPackageDb::with_packages(vec![MockPackage {
        name: "test-pkg".to_string(),
        version: "1.0.0".to_string(),
        description: "A test package".to_string(),
        repo: "test".to_string(),
        dependencies: vec![],
        installed_size: 100,
    }]);

    let pm = MockPackageManager::new(db);

    // Test search
    let results = pm.search("test").await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "test-pkg");
    assert!(!results[0].installed);

    // Test install
    pm.install(&["test-pkg".to_string()]).await.unwrap();

    // Test search again (should be installed)
    let results = pm.search("test").await.unwrap();
    assert!(results[0].installed);

    // Test info
    let info = pm.info("test-pkg").await.unwrap().unwrap();
    assert_eq!(info.version.to_string(), "1.0.0");
    assert!(info.installed);
}
