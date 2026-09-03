//! S-Tier E2E Tests for ALPM Transaction Handling
//!
//! Uses `AlpmHarness` for isolated transaction testing (no root needed)
//! and real system database for read-only validation.

#![cfg(feature = "arch")]
#![expect(clippy::unwrap_used, clippy::expect_used, clippy::pedantic)]

pub mod alpm_harness;
pub mod common;

use alpm_harness::{AlpmHarness, HarnessPkg};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

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
        .add_installable_sync_pkg("core", &HarnessPkg::new("ed", "1.20-1"))
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

    // "ed" is installable (desc carries %FILENAME%/sizes and a cached
    // payload), so prepare must fully resolve the transaction instead of
    // silently failing.
    let prepared = alpm.trans_prepare();
    assert!(
        prepared.is_ok(),
        "prepare must succeed for an installable target: {:?}",
        prepared.err()
    );
    // Drop before release since PrepareError borrows alpm
    drop(prepared);
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

    for _ in 0..10 {
        alpm.trans_init(alpm::TransFlag::empty())
            .expect("Init failed in cycle");
        alpm.trans_release().expect("Release failed in cycle");
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
            // Real installed versions are strictly greater than the lowest
            // possible version, pinning that vercmp orders (not just compares
            // reflexively) through omg's alpm handle.
            let vs_zero = alpm::vercmp(version, "0");
            assert_eq!(
                vs_zero,
                std::cmp::Ordering::Greater,
                "installed version {version} must order greater than 0"
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
        // bash is an essential package on every Arch system; do not silently
        // skip the assertions below when it is absent.
        let bash = localdb
            .pkg("bash")
            .expect("bash must be installed on a real Arch system");
        {
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
fn test_alpm_log_callback_receives_transaction_events() {
    require_arch!();

    let log_count = Arc::new(AtomicU64::new(0));

    let harness = AlpmHarness::new().expect("Failed to create harness");
    harness
        .add_sync_pkg("core", &HarnessPkg::new("ed", "1.20-1"))
        .expect("Failed to add package");
    let mut alpm = harness.alpm().expect("Failed to get handle");
    alpm.register_syncdb("core", alpm::SigLevel::NONE)
        .expect("Failed to register db");

    alpm.set_log_cb(Arc::clone(&log_count), |_level, _msg, counter| {
        counter.fetch_add(1, Ordering::Relaxed);
    });

    alpm.trans_init(alpm::TransFlag::empty())
        .expect("Should init");
    for db in alpm.syncdbs() {
        if let Ok(pkg) = db.pkg("ed") {
            alpm.trans_add_pkg(pkg)
                .expect("Should add pkg to transaction");
            break;
        }
    }
    // libalpm routes its dependency-resolution messages through the log
    // callback during prepare, so the callback must have fired by now.
    drop(alpm.trans_prepare());
    alpm.trans_release().expect("Should release");

    assert!(
        log_count.load(Ordering::Relaxed) > 0,
        "log callback must receive at least one message during prepare"
    );
}

// NOTE: there is deliberately no progress-callback test here. Progress events
// only fire during payload extraction inside `trans_commit`, which requires a
// sync server and populated package cache that this desc-only harness cannot
// provide; an unasserted counter would pass even if callbacks were never
// delivered.

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
    alpm.trans_prepare()
        .expect("Prepare of an empty transaction should succeed");
    alpm.trans_release().expect("Release should work");
}
