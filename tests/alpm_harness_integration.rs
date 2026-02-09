//! ALPM Integration Tests Using Harness
//!
//! These tests use the `AlpmHarness` to test real ALPM operations
//! in an isolated environment without modifying the system.

#![cfg(feature = "arch")]

mod alpm_harness;

use alpm_harness::{AlpmHarness, HarnessPkg};
use anyhow::Result;

#[test]
fn test_harness_package_search() -> Result<()> {
    let harness = AlpmHarness::new()?;

    // Add packages to sync database (must add all at once to same DB)
    harness.add_sync_pkgs(
        "core",
        &[
            HarnessPkg::new("firefox", "122.0-1"),
            HarnessPkg::new("chromium", "121.0-1"),
        ],
    )?;
    harness.add_sync_pkg("extra", &HarnessPkg::new("firewalld", "2.0-1"))?;

    let alpm = harness.alpm()?;
    let db = alpm.register_syncdb("core", alpm::SigLevel::NONE)?;

    // Search for packages matching "fire"
    let mut results = Vec::new();
    for pkg in db.pkgs() {
        if pkg.name().contains("fire") {
            results.push(pkg);
        }
    }

    assert_eq!(
        results.len(),
        1,
        "Should find 1 package matching 'fire' in core"
    );
    assert_eq!(results[0].name(), "firefox");
    assert_eq!(results[0].version().as_str(), "122.0-1");

    Ok(())
}

#[test]
fn test_harness_version_comparison() -> Result<()> {
    let harness = AlpmHarness::new()?;

    harness.add_sync_pkg("core", &HarnessPkg::new("vim", "9.0.2000-1"))?;
    harness.add_sync_pkg("core", &HarnessPkg::new("neovim", "0.9.5-1"))?;

    let _alpm = harness.alpm()?;

    // Test version comparison
    let v1 = alpm::Version::new("9.0.2000-1");
    let v2 = alpm::Version::new("9.0.1999-1");
    let v3 = alpm::Version::new("9.0.2000-2");

    assert!(v1 > v2, "9.0.2000 > 9.0.1999");
    assert!(v3 > v1, "9.0.2000-2 > 9.0.2000-1 (pkg release)");

    Ok(())
}

#[test]
fn test_harness_package_info() -> Result<()> {
    let harness = AlpmHarness::new()?;

    harness.add_sync_pkg("core", &HarnessPkg::new("ripgrep", "14.0.3-1"))?;

    let alpm = harness.alpm()?;
    let db = alpm.register_syncdb("core", alpm::SigLevel::NONE)?;

    let pkg = db.pkg("ripgrep")?;

    assert_eq!(pkg.name(), "ripgrep");
    assert_eq!(pkg.version().as_str(), "14.0.3-1");
    assert_eq!(pkg.desc(), Some("A test package"));
    assert_eq!(pkg.arch(), Some("x86_64"));

    Ok(())
}

#[test]
fn test_harness_multiple_databases() -> Result<()> {
    let harness = AlpmHarness::new()?;

    // Add packages to different databases
    harness.add_sync_pkg("core", &HarnessPkg::new("bash", "5.2-1"))?;
    harness.add_sync_pkg("extra", &HarnessPkg::new("vim", "9.0-1"))?;
    harness.add_sync_pkg("community", &HarnessPkg::new("rustup", "1.26-1"))?;

    let alpm = harness.alpm()?;

    // Register all databases
    let core = alpm.register_syncdb("core", alpm::SigLevel::NONE)?;
    let extra = alpm.register_syncdb("extra", alpm::SigLevel::NONE)?;
    let community = alpm.register_syncdb("community", alpm::SigLevel::NONE)?;

    // Verify packages are in correct databases
    assert!(core.pkg("bash").is_ok());
    assert!(core.pkg("vim").is_err());

    assert!(extra.pkg("vim").is_ok());
    assert!(extra.pkg("bash").is_err());

    assert!(community.pkg("rustup").is_ok());
    assert!(community.pkg("vim").is_err());

    Ok(())
}

#[test]
fn test_harness_package_not_found() -> Result<()> {
    let harness = AlpmHarness::new()?;

    harness.add_sync_pkg("core", &HarnessPkg::new("firefox", "122.0-1"))?;

    let alpm = harness.alpm()?;
    let db = alpm.register_syncdb("core", alpm::SigLevel::NONE)?;

    // Should fail for non-existent package
    let result = db.pkg("nonexistent-package");
    assert!(result.is_err(), "Should not find nonexistent package");

    Ok(())
}

#[test]
fn test_harness_epoch_version_comparison() -> Result<()> {
    let harness = AlpmHarness::new()?;

    // Test epoch handling (epoch:version-release)
    harness.add_sync_pkg("core", &HarnessPkg::new("package-a", "1:2.0-1"))?;
    harness.add_sync_pkg("core", &HarnessPkg::new("package-b", "3.0-1"))?;

    let _alpm = harness.alpm()?;

    let v_with_epoch = alpm::Version::new("1:2.0-1");
    let v_without_epoch = alpm::Version::new("3.0-1");

    // Epoch takes precedence
    assert!(
        v_with_epoch > v_without_epoch,
        "1:2.0-1 > 3.0-1 (epoch wins)"
    );

    Ok(())
}

#[test]
fn test_harness_search_across_databases() -> Result<()> {
    let harness = AlpmHarness::new()?;

    // Add packages with similar names to different databases
    harness.add_sync_pkg("core", &HarnessPkg::new("python", "3.12-1"))?;
    harness.add_sync_pkg("extra", &HarnessPkg::new("python-pip", "23.0-1"))?;
    harness.add_sync_pkg("community", &HarnessPkg::new("python-requests", "2.31-1"))?;

    let alpm = harness.alpm()?;
    alpm.register_syncdb("core", alpm::SigLevel::NONE)?;
    alpm.register_syncdb("extra", alpm::SigLevel::NONE)?;
    alpm.register_syncdb("community", alpm::SigLevel::NONE)?;

    // Search across all sync databases
    let mut results = Vec::new();
    for db in alpm.syncdbs() {
        for pkg in db.pkgs() {
            if pkg.name().contains("python") {
                results.push((db.name().to_string(), pkg.name().to_string()));
            }
        }
    }

    assert_eq!(results.len(), 3, "Should find 3 python-related packages");
    assert!(results.iter().any(|(db, _)| db == "core"));
    assert!(results.iter().any(|(db, _)| db == "extra"));
    assert!(results.iter().any(|(db, _)| db == "community"));

    Ok(())
}

#[test]
fn test_harness_package_listing() -> Result<()> {
    let harness = AlpmHarness::new()?;

    // Add multiple packages (must add all at once)
    let packages: Vec<_> = (1..=5)
        .map(|i| HarnessPkg::new(&format!("pkg{i}"), "1.0-1"))
        .collect();
    harness.add_sync_pkgs("core", &packages)?;

    let alpm = harness.alpm()?;
    let db = alpm.register_syncdb("core", alpm::SigLevel::NONE)?;

    let mut packages = Vec::new();
    for pkg in db.pkgs() {
        packages.push(pkg);
    }

    assert_eq!(packages.len(), 5, "Should have 5 packages");

    // Verify all packages are present
    for i in 1..=5 {
        let pkg_name = format!("pkg{i}");
        assert!(
            packages.iter().any(|p| p.name() == pkg_name),
            "Should find {pkg_name}"
        );
    }

    Ok(())
}

#[test]
fn test_harness_alpm_initialization() -> Result<()> {
    let harness = AlpmHarness::new()?;

    // Verify ALPM can be initialized with harness paths
    let alpm = harness.alpm()?;

    assert_eq!(
        alpm.root().trim_end_matches('/'),
        harness.root().to_str().unwrap()
    );
    assert_eq!(
        alpm.dbpath().trim_end_matches('/'),
        harness.db_path().to_str().unwrap()
    );

    // Verify filesystem layout exists
    assert!(harness.root().join("var/lib/pacman").exists());
    assert!(harness.root().join("var/lib/pacman/local").exists());
    assert!(harness.root().join("var/lib/pacman/sync").exists());
    assert!(harness.root().join("var/cache/pacman/pkg").exists());

    Ok(())
}

#[test]
fn test_harness_version_sorting() -> Result<()> {
    let harness = AlpmHarness::new()?;

    // Add packages with different versions (must add all at once)
    harness.add_sync_pkgs(
        "core",
        &[
            HarnessPkg::new("kernel", "6.7.1-1"),
            HarnessPkg::new("kernel-lts", "6.6.15-1"),
            HarnessPkg::new("kernel-zen", "6.7.2-1"),
        ],
    )?;

    let alpm = harness.alpm()?;
    let db = alpm.register_syncdb("core", alpm::SigLevel::NONE)?;

    // Collect packages starting with "kernel" and sort by version
    let mut packages = Vec::new();
    for pkg in db.pkgs() {
        if pkg.name().starts_with("kernel") {
            packages.push(pkg);
        }
    }

    packages.sort_by(|a, b| b.version().cmp(a.version()));

    // Should be sorted: 6.7.2 > 6.7.1 > 6.6.15
    assert_eq!(packages[0].name(), "kernel-zen"); // 6.7.2-1
    assert_eq!(packages[1].name(), "kernel"); // 6.7.1-1
    assert_eq!(packages[2].name(), "kernel-lts"); // 6.6.15-1

    Ok(())
}

#[cfg(test)]
mod integration_with_omg {
    use super::*;

    #[test]
    fn test_omg_alpm_ops_with_harness() -> Result<()> {
        let harness = AlpmHarness::new()?;

        // Add test packages
        harness.add_sync_pkg("core", &HarnessPkg::new("test-pkg", "1.0-1"))?;

        let alpm = harness.alpm()?;
        let db = alpm.register_syncdb("core", alpm::SigLevel::NONE)?;

        // Test that OMG's ALPM operations work with harness
        // This would normally call functions from src/package_managers/alpm_ops.rs
        // For now, just verify the harness provides valid ALPM handles

        assert!(db.pkg("test-pkg").is_ok());

        Ok(())
    }
}
