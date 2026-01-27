#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! Logic tests for package manager abstraction
//!
//! These tests verify that the package manager abstraction works across
//! different simulated distributions using the `OMG_TEST_DISTRO` override.

mod common;

use crate::common::*;
use omg_lib::package_managers::get_package_manager;

use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn test_distro_override_arch() {
    // ===== ARRANGE =====
    init_test_env();
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("OMG_TEST_DISTRO", "arch");
    }

    // ===== ACT =====
    let pm = get_package_manager();

    // ===== ASSERT =====
    assert_eq!(pm.name(), "pacman");
}

#[test]
#[cfg(feature = "debian")]
fn test_distro_override_debian() {
    // ===== ARRANGE =====
    init_test_env();
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("OMG_TEST_DISTRO", "debian");
    }

    // ===== ACT =====
    let pm = get_package_manager();

    // ===== ASSERT =====
    assert_eq!(pm.name(), "apt");
}

#[test]
#[cfg(feature = "debian")]
fn test_distro_override_ubuntu() {
    // ===== ARRANGE =====
    init_test_env();
    let _guard = ENV_LOCK.lock().unwrap();
    unsafe {
        std::env::set_var("OMG_TEST_DISTRO", "ubuntu");
    }

    // ===== ACT =====
    let pm = get_package_manager();

    // ===== ASSERT =====
    assert_eq!(pm.name(), "apt");
}

#[tokio::test]
async fn test_mock_package_manager_search() {
    use crate::common::fixtures::{PackageFixture, PackageFixtureExt};
    use crate::common::mocks::{MockPackageDb, MockPackageManager};
    use omg_lib::package_managers::PackageManager;

    // ===== ARRANGE =====
    let test_package = PackageFixture::new()
        .name("test-pkg")
        .version("1.0.0")
        .description("A test package")
        .to_mock_package();
    let db = MockPackageDb::with_packages(vec![test_package]);
    let pm = MockPackageManager::new(db);

    // ===== ACT =====
    let results = pm.search("test").await.unwrap();

    // ===== ASSERT =====
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].name, "test-pkg");
    assert!(!results[0].installed);
}

#[tokio::test]
async fn test_mock_package_manager_install() {
    use crate::common::fixtures::{PackageFixture, PackageFixtureExt};
    use crate::common::mocks::{MockPackageDb, MockPackageManager};
    use omg_lib::package_managers::PackageManager;

    // ===== ARRANGE =====
    let test_package = PackageFixture::new()
        .name("test-pkg")
        .version("1.0.0")
        .description("A test package")
        .to_mock_package();
    let db = MockPackageDb::with_packages(vec![test_package]);
    let pm = MockPackageManager::new(db);
    let package_name = "test-pkg".to_string();

    // ===== ACT =====
    pm.install(std::slice::from_ref(&package_name))
        .await
        .unwrap();

    // ===== ASSERT =====
    let results = pm.search("test").await.unwrap();
    assert!(results[0].installed);
}

#[tokio::test]
async fn test_mock_package_manager_info() {
    use crate::common::fixtures::{PackageFixture, PackageFixtureExt};
    use crate::common::mocks::{MockPackageDb, MockPackageManager};
    use omg_lib::package_managers::PackageManager;

    // ===== ARRANGE =====
    let test_package = PackageFixture::new()
        .name("test-pkg")
        .version("1.0.0")
        .description("A test package")
        .to_mock_package();
    let db = MockPackageDb::with_packages(vec![test_package]);
    let pm = MockPackageManager::new(db);
    pm.install(&["test-pkg".to_string()]).await.unwrap();

    // ===== ACT =====
    let info = pm.info("test-pkg").await.unwrap().unwrap();

    // ===== ASSERT =====
    #[cfg(not(feature = "arch"))]
    assert_eq!(info.version, "1.0.0");
    #[cfg(feature = "arch")]
    assert_eq!(info.version.to_string(), "1.0.0");
    assert!(info.installed);
}
