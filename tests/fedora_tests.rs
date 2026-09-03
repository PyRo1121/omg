#![cfg(all(target_os = "linux", feature = "fedora"))]

use anyhow::Result;
use omg_lib::package_managers::{DnfPackageManager, PackageManager};

pub mod common;
pub mod platform_semantics;

use platform_semantics::{assert_no_arch_terms, assert_no_debian_terms, assert_no_macos_terms};

mod dnf_integration {
    use super::*;

    #[tokio::test]
    async fn test_dnf_package_manager_creation() {
        let pm = DnfPackageManager::new();
        assert_eq!(pm.name(), "dnf");
        let identity = pm.name().to_string();
        assert_no_debian_terms(&identity, "Fedora package manager identity");
        assert_no_arch_terms(&identity, "Fedora package manager identity");
        assert_no_macos_terms(&identity, "Fedora package manager identity");
    }

    #[tokio::test]
    async fn test_search_common_package() {
        require_system_tests!();

        let pm = DnfPackageManager::new();

        let results = pm.search("vim").await.unwrap();

        assert!(!results.is_empty(), "Should find vim package");
        assert!(
            results.iter().any(|p| p.name.contains("vim")),
            "Results should contain vim"
        );
    }

    /// Searches shell out to the host's dnf/rpm, so they require a real
    /// Fedora-class system just like the other dnf_integration tests.
    #[tokio::test]
    async fn test_search_nonexistent_package() -> Result<()> {
        // Gate inline (instead of require_system_tests!, whose `return;` is
        // incompatible with this test's Result signature).
        if common::TestConfig::default().skip_if_no_system("dnf_search_nonexistent") {
            common::report_skip("system tests disabled (set OMG_RUN_SYSTEM_TESTS=1)");
            return Ok(());
        }

        let pm = DnfPackageManager::new();

        let results = pm.search("nonexistent-package-xyz-12345").await?;

        assert!(results.is_empty(), "Should not find nonexistent package");

        Ok(())
    }

    #[tokio::test]
    async fn test_list_installed_packages() {
        require_system_tests!();

        let pm = DnfPackageManager::new();

        let installed = pm.list_installed().await.unwrap();

        assert!(
            !installed.is_empty(),
            "Should have installed packages on Fedora system"
        );
    }

    #[tokio::test]
    async fn test_list_updates() -> Result<()> {
        // Gate inline (instead of require_system_tests!, whose `return;` is
        // incompatible with this test's Result signature).
        if common::TestConfig::default().skip_if_no_system("dnf_list_updates") {
            common::report_skip("system tests disabled (set OMG_RUN_SYSTEM_TESTS=1)");
            return Ok(());
        }

        let pm = DnfPackageManager::new();

        // The call must succeed on a real system, and every reported update
        // must be well-formed: named package with distinct old/new versions.
        let updates = pm.list_updates().await?;
        for update in &updates {
            assert!(
                !update.name.is_empty(),
                "every update entry needs a package name"
            );
            assert!(
                update.old_version != update.new_version,
                "update for {} must change version ({} -> {})",
                update.name,
                update.old_version,
                update.new_version
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_is_installed_check() {
        require_system_tests!();

        let pm = DnfPackageManager::new();

        let is_bash_installed = pm.is_installed("bash").await.unwrap();
        assert!(
            is_bash_installed,
            "bash should be installed on Fedora system"
        );
    }
}

mod dnf_rpm_database {
    use super::*;

    #[tokio::test]
    async fn test_rpm_database_query() {
        require_system_tests!();

        let pm = DnfPackageManager::new();

        let installed = pm.list_installed().await.unwrap();

        assert!(
            !installed.is_empty(),
            "Should read packages from RPM database"
        );

        let has_rpm = installed.iter().any(|p| p.name.contains("rpm"));
        assert!(has_rpm, "Should find rpm package itself in database");
    }
}

mod dnf_operations {
    use super::*;

    #[tokio::test]
    #[ignore = "requires root privileges and modifies system"]
    async fn test_install_and_remove_package() -> Result<()> {
        let pm = DnfPackageManager::new();

        let test_package = "nano";

        let initial_check = pm.is_installed(test_package).await.unwrap();
        if initial_check {
            pm.remove(&[test_package.to_string()]).await?;
        }

        pm.install(&[test_package.to_string()]).await?;
        assert!(
            pm.is_installed(test_package).await.unwrap(),
            "Package should be installed"
        );

        pm.remove(&[test_package.to_string()]).await?;
        assert!(
            !pm.is_installed(test_package).await.unwrap(),
            "Package should be removed"
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires root privileges and modifies system"]
    async fn test_update_all_packages() -> Result<()> {
        let pm = DnfPackageManager::new();

        pm.update().await?;

        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires root privileges and modifies system"]
    async fn test_sync_repository_metadata() -> Result<()> {
        let pm = DnfPackageManager::new();

        pm.sync().await?;

        Ok(())
    }
}

mod dnf_error_handling {
    use super::*;

    #[tokio::test]
    async fn test_install_invalid_package() {
        if !omg_lib::core::is_root() {
            common::report_skip("requires root privileges");
            return;
        }

        let pm = DnfPackageManager::new();

        let result = pm
            .install(&["nonexistent-package-xyz-12345".to_string()])
            .await;

        assert!(
            result.is_err(),
            "Should fail to install nonexistent package"
        );
    }
}
