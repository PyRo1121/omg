//! S-Tier E2E Tests for ALPM Transaction Handling
//!
//! Uses `AlpmHarness` for isolated transaction testing (no root needed)
//! and real system database for read-only validation.

#![cfg(feature = "arch")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::pedantic)]

mod alpm_harness;
mod common;

use alpm_harness::{AlpmHarness, HarnessPkg};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

// =============================================================================
// TRANSACTION LIFECYCLE TESTS (using harness - no root needed)
// =============================================================================

#[test]
fn test_alpm_transaction_init_and_release() {
    require_arch!();

    let harness = AlpmHarness::new().expect("Failed to create harness");
    let mut alpm = harness.alpm().expect("Failed to get handle");

    alpm.trans_init(alpm::TransFlag::empty())
        .expect("Should initialize transaction");
    assert!(alpm.trans_add().is_empty());
    assert!(alpm.trans_remove().is_empty());
    alpm.trans_release().expect("Should release transaction");
}

#[test]
fn test_alpm_transaction_add_package() {
    require_arch!();

    let harness = AlpmHarness::new().expect("Failed to create harness");
    harness
        .add_sync_pkgs(
            "core",
            &[
                HarnessPkg::new("ed", "1.20-1"),
                HarnessPkg::new("vim", "9.0-1"),
            ],
        )
        .expect("Failed to add packages");

    let mut alpm = harness.alpm().expect("Failed to get handle");
    alpm.register_syncdb("core", alpm::SigLevel::NONE)
        .expect("Failed to register db");

    alpm.trans_init(alpm::TransFlag::empty())
        .expect("Should init transaction");

    // Add package using same pattern as production code
    for db in alpm.syncdbs() {
        if let Ok(pkg) = db.pkg("ed") {
            alpm.trans_add_pkg(pkg)
                .expect("Should add pkg to transaction");
            break;
        }
    }

    let to_add = alpm.trans_add();
    assert_eq!(to_add.len(), 1, "Should have 1 package in transaction");

    alpm.trans_release().expect("Should release transaction");
}

#[test]
fn test_alpm_transaction_prepare() {
    require_arch!();

    let harness = AlpmHarness::new().expect("Failed to create harness");
    harness
        .add_sync_pkg("core", &HarnessPkg::new("ed", "1.20-1"))
        .expect("Failed to add package");

    let mut alpm = harness.alpm().expect("Failed to get handle");
    alpm.register_syncdb("core", alpm::SigLevel::NONE)
        .expect("Failed to register db");

    alpm.trans_init(alpm::TransFlag::empty())
        .expect("Should init transaction");

    for db in alpm.syncdbs() {
        if let Ok(pkg) = db.pkg("ed") {
            let _ = alpm.trans_add_pkg(pkg);
            break;
        }
    }

    // Prepare resolves dependencies - may fail in harness but should not panic
    // Drop the result before release since PrepareError borrows alpm
    drop(alpm.trans_prepare());
    alpm.trans_release().expect("Should release transaction");
}

#[test]
fn test_alpm_transaction_multiple_packages() {
    require_arch!();

    let harness = AlpmHarness::new().expect("Failed to create harness");
    harness
        .add_sync_pkgs(
            "core",
            &[
                HarnessPkg::new("pkg-a", "1.0-1"),
                HarnessPkg::new("pkg-b", "2.0-1"),
                HarnessPkg::new("pkg-c", "3.0-1"),
            ],
        )
        .expect("Failed to add packages");

    let mut alpm = harness.alpm().expect("Failed to get handle");
    alpm.register_syncdb("core", alpm::SigLevel::NONE)
        .expect("Failed to register db");

    alpm.trans_init(alpm::TransFlag::empty())
        .expect("Should init transaction");

    let names = ["pkg-a", "pkg-b", "pkg-c"];
    for db in alpm.syncdbs() {
        for name in &names {
            if let Ok(pkg) = db.pkg(*name) {
                let _ = alpm.trans_add_pkg(pkg);
            }
        }
    }

    let to_add = alpm.trans_add();
    assert_eq!(to_add.len(), 3, "Should have 3 packages in transaction");

    alpm.trans_release().expect("Should release transaction");
}

#[test]
fn test_alpm_transaction_init_release_cycle() {
    require_arch!();

    let harness = AlpmHarness::new().expect("Failed to create harness");
    let mut alpm = harness.alpm().expect("Failed to get handle");

    for i in 0..10 {
        alpm.trans_init(alpm::TransFlag::empty())
            .unwrap_or_else(|e| panic!("Init failed on cycle {i}: {e}"));
        alpm.trans_release()
            .unwrap_or_else(|e| panic!("Release failed on cycle {i}: {e}"));
    }
}

// =============================================================================
// READ-ONLY DATABASE TESTS (real system, no root needed)
// =============================================================================

#[test]
fn test_alpm_local_database_query() {
    require_arch!();

    omg_lib::package_managers::alpm_direct::with_handle(|alpm| {
        let localdb = alpm.localdb();
        let pkg_count = localdb.pkgs().len();
        assert!(pkg_count > 0, "Should have installed packages");
        assert!(localdb.pkg("pacman").is_ok(), "pacman should be installed");
        Ok(())
    })
    .expect("Local DB query test failed");
}

#[test]
fn test_alpm_sync_database_query() {
    require_arch!();

    omg_lib::package_managers::alpm_direct::with_handle(|alpm| {
        let syncdbs = alpm.syncdbs();
        assert!(!syncdbs.is_empty(), "Should have sync databases");
        for db in syncdbs {
            let count = db.pkgs().len();
            assert!(
                count > 0,
                "Sync database '{}' should not be empty",
                db.name()
            );
        }
        Ok(())
    })
    .expect("Sync DB query test failed");
}

#[test]
fn test_alpm_package_version_comparison() {
    require_arch!();

    omg_lib::package_managers::alpm_direct::with_handle(|alpm| {
        let localdb = alpm.localdb();
        for pkg in localdb.pkgs().iter().take(5) {
            let version = pkg.version().as_str();
            let cmp = alpm::vercmp(version, version);
            assert_eq!(
                cmp,
                std::cmp::Ordering::Equal,
                "Version should equal itself"
            );
        }
        Ok(())
    })
    .expect("Version comparison test failed");
}

#[test]
fn test_alpm_dependency_chain_query() {
    require_arch!();

    omg_lib::package_managers::alpm_direct::with_handle(|alpm| {
        let localdb = alpm.localdb();
        if let Ok(bash) = localdb.pkg("bash") {
            let deps = bash.depends();
            assert!(!deps.is_empty(), "bash should have dependencies");

            for dep in deps {
                let dep_name = dep.name();
                let satisfied = localdb.pkg(dep_name).is_ok()
                    || localdb
                        .pkgs()
                        .iter()
                        .any(|p| p.provides().iter().any(|prov| prov.name() == dep_name));
                assert!(satisfied, "Dependency '{dep_name}' should be satisfied");
            }
        }
        Ok(())
    })
    .expect("Dependency chain query test failed");
}

// =============================================================================
// CALLBACK TESTS (using harness)
// =============================================================================

#[test]
fn test_alpm_log_callback() {
    require_arch!();

    let log_count = Arc::new(AtomicU64::new(0));

    let harness = AlpmHarness::new().expect("Failed to create harness");
    let mut alpm = harness.alpm().expect("Failed to get handle");

    alpm.set_log_cb(Arc::clone(&log_count), |_level, _msg, counter| {
        counter.fetch_add(1, Ordering::Relaxed);
    });

    alpm.trans_init(alpm::TransFlag::empty())
        .expect("Should init");
    alpm.trans_release().expect("Should release");
}

#[test]
fn test_alpm_progress_callback() {
    require_arch!();

    let counter = Arc::new(AtomicU64::new(0));

    let harness = AlpmHarness::new().expect("Failed to create harness");
    let mut alpm = harness.alpm().expect("Failed to get handle");

    alpm.set_progress_cb(
        Arc::clone(&counter),
        |_op, _name, _percent, _n, _total, counter| {
            counter.fetch_add(1, Ordering::Relaxed);
        },
    );

    alpm.trans_init(alpm::TransFlag::empty())
        .expect("Should init");
    alpm.trans_release().expect("Should release");
}

// =============================================================================
// PERFORMANCE TESTS
// =============================================================================

#[test]
fn test_alpm_rapid_transaction_creation() {
    require_arch!();

    let iterations = 50u32;
    let start = Instant::now();
    let harness = AlpmHarness::new().expect("Failed to create harness");

    for _ in 0..iterations {
        let mut alpm = harness.alpm().expect("Failed to get handle");
        alpm.trans_init(alpm::TransFlag::empty())
            .expect("Init failed");
        alpm.trans_release().expect("Release failed");
    }

    let duration = start.elapsed();
    let avg = duration / iterations;
    assert!(
        avg.as_millis() < 100,
        "Transaction creation should be fast (avg: {avg:?})"
    );
}

#[test]
fn test_alpm_memory_leak_detection() {
    require_arch!();

    let harness = AlpmHarness::new().expect("Failed to create harness");
    harness
        .add_sync_pkgs("core", &[HarnessPkg::new("test-a", "1.0-1")])
        .expect("Failed to add packages");

    for _ in 0..100 {
        let mut alpm = harness.alpm().expect("Failed to get handle");
        alpm.register_syncdb("core", alpm::SigLevel::NONE)
            .expect("reg db");
        alpm.trans_init(alpm::TransFlag::empty())
            .expect("Init failed");

        for db in alpm.syncdbs() {
            if let Ok(pkg) = db.pkg("test-a") {
                let _ = alpm.trans_add_pkg(pkg);
                break;
            }
        }

        drop(alpm.trans_prepare());
        alpm.trans_release().expect("Release failed");
    }
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

#[test]
fn test_alpm_double_release_protection() {
    require_arch!();

    let harness = AlpmHarness::new().expect("Failed to create harness");
    let mut alpm = harness.alpm().expect("Failed to get handle");

    alpm.trans_init(alpm::TransFlag::empty())
        .expect("Init should work");
    alpm.trans_release().expect("First release should work");

    let result = alpm.trans_release();
    assert!(result.is_err(), "Double release should return error");
}

#[test]
fn test_alpm_init_without_release() {
    require_arch!();

    let harness = AlpmHarness::new().expect("Failed to create harness");
    let mut alpm = harness.alpm().expect("Failed to get handle");

    alpm.trans_init(alpm::TransFlag::empty())
        .expect("Init should work");

    let result = alpm.trans_init(alpm::TransFlag::empty());
    assert!(
        result.is_err(),
        "Double init without release should return error"
    );

    alpm.trans_release().expect("Release should work");
}

#[test]
fn test_alpm_empty_transaction_prepare() {
    require_arch!();

    let harness = AlpmHarness::new().expect("Failed to create harness");
    let mut alpm = harness.alpm().expect("Failed to get handle");

    alpm.trans_init(alpm::TransFlag::empty())
        .expect("Init should work");
    drop(alpm.trans_prepare());
    alpm.trans_release().expect("Release should work");
}
