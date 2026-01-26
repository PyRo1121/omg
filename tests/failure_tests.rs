//! Failure Scenario Tests
//!
//! Tests critical failure modes using the isolated ALPM harness.

#![cfg(feature = "arch")]
#![allow(clippy::uninlined_format_args)]

mod alpm_harness;
use alpm_harness::{AlpmHarness, HarnessPkg};
use anyhow::Result;
use omg_lib::package_managers::alpm_ops;
use serial_test::serial;

use omg_lib::core::paths;

#[test]
#[serial]
fn test_conflicting_packages_fails_gracefully() -> Result<()> {
    let harness = AlpmHarness::new()?;

    // Create two conflicting packages
    let mut pkg_a = HarnessPkg::new("pkg-a", "1.0.0");
    pkg_a.desc.push_str("%CONFLICTS%\npkg-b\n\n");

    let mut pkg_b = HarnessPkg::new("pkg-b", "1.0.0");
    pkg_b.desc.push_str("%CONFLICTS%\npkg-a\n\n");

    harness.add_sync_pkg("core", &pkg_a)?;
    harness.add_sync_pkg("extra", &pkg_b)?;

    // SAFE path overrides (no unsafe needed!)
    paths::set_test_overrides(
        Some(harness.root().to_path_buf()),
        Some(harness.db_path().to_path_buf()),
    );

    // Ensure we reset after the test
    scopeguard::defer! {
        paths::reset_test_overrides();
    }

    let mut alpm = harness.alpm()?;
    alpm.register_syncdb("core", alpm::SigLevel::NONE)?;
    alpm.register_syncdb("extra", alpm::SigLevel::NONE)?;

    // Execute transaction using the injected handle
    let result = alpm_ops::execute_transaction(
        vec!["pkg-a".to_string(), "pkg-b".to_string()],
        false,
        false,
        Some(&mut alpm),
    );

    assert!(result.is_err(), "Transaction should fail due to conflicts");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("conflicting packages")
            || err.contains("Transaction failed")
            || err.contains("pkg-a"),
        "Error message should mention conflicts or transaction failure, got: {}",
        err
    );

    Ok(())
}

#[test]
#[serial]
fn test_permission_denied_on_cache_fails_gracefully() -> Result<()> {
    let harness = AlpmHarness::new()?;

    let pkg = HarnessPkg::new("pkg-a", "1.0.0");
    harness.add_sync_pkg("core", &pkg)?;

    // Make the root directory read-only to simulate permission issues
    let sync_dir = harness.db_path().join("sync");
    let mut perms = std::fs::metadata(&sync_dir)?.permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&sync_dir, perms)?;

    // Ensure we reset permissions so harness can be cleaned up
    scopeguard::defer! {
        if let Ok(meta) = std::fs::metadata(&sync_dir) {
            use std::os::unix::fs::PermissionsExt;
            let mut p = meta.permissions();
            p.set_mode(0o755); // Restore standard directory permissions
            let _ = std::fs::set_permissions(&sync_dir, p);
        }
    }

    paths::set_test_overrides(
        Some(harness.root().to_path_buf()),
        Some(harness.db_path().to_path_buf()),
    );

    scopeguard::defer! {
        paths::reset_test_overrides();
    }

    // This should fail during handle creation or DB registration
    let result = alpm_ops::execute_transaction(vec!["pkg-a".to_string()], false, false, None);

    assert!(
        result.is_err(),
        "Transaction should fail due to permissions"
    );

    Ok(())
}

#[test]
#[serial]
fn test_missing_dependency_fails_gracefully() -> Result<()> {
    let harness = AlpmHarness::new()?;

    // Create a package with a missing dependency
    let mut pkg_a = HarnessPkg::new("pkg-a", "1.0.0");
    pkg_a.desc.push_str("%DEPENDS%\nnon-existent-dep\n\n");

    harness.add_sync_pkg("core", &pkg_a)?;

    paths::set_test_overrides(
        Some(harness.root().to_path_buf()),
        Some(harness.db_path().to_path_buf()),
    );

    scopeguard::defer! {
        paths::reset_test_overrides();
    }

    let mut alpm = harness.alpm()?;
    alpm.register_syncdb("core", alpm::SigLevel::NONE)?;

    let result =
        alpm_ops::execute_transaction(vec!["pkg-a".to_string()], false, false, Some(&mut alpm));

    assert!(
        result.is_err(),
        "Transaction should fail due to missing dependency"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("could not satisfy dependencies")
            || err.contains("Preparation Error")
            || err.contains("not found"),
        "Error message should mention dependency failure, got: {}",
        err
    );

    Ok(())
}

#[test]
#[serial]
fn test_locked_database_shows_friendly_message() -> Result<()> {
    let harness = AlpmHarness::new()?;

    // Manually create a lock file
    let lock_file = harness.db_path().join("db.lck");
    std::fs::File::create(&lock_file)?;

    paths::set_test_overrides(
        Some(harness.root().to_path_buf()),
        Some(harness.db_path().to_path_buf()),
    );

    scopeguard::defer! {
        paths::reset_test_overrides();
    }

    // Attempting to create an Alpm handle or start a transaction should fail
    // when the handle is NOT provided (production path).
    let result = alpm_ops::execute_transaction(
        vec!["any-pkg".to_string()],
        false,
        false,
        None, // No injected handle, force creation of new one
    );

    assert!(result.is_err(), "Transaction should fail due to lock file");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("Database is locked") || err.contains("another package manager"),
        "Error message should mention the lock, got: {}",
        err
    );

    Ok(())
}
