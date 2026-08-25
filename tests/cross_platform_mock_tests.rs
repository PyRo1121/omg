//! Cross-platform mock package manager tests.
//!
//! These tests verify that mock package managers for supported platforms (Arch, Debian,
//! Fedora, macOS) work correctly and maintain proper state isolation.
//!
//! ## State Isolation
//!
//! Each platform uses a separate state file (`mock_state_{platform}.json`) to prevent
//! cross-contamination during tests.
//!
//! ## Test Organization
//!
//! - `all_platforms`: Basic smoke tests for each platform
//! - `search_functionality`: Search across platforms
//! - `package_info`: Info command tests
//! - `install_remove_operations`: Install/remove on all platforms
//! - `update_scenarios`: Update detection and version comparison
//! - `status_reporting`: Status command (total, explicit, orphans, updates)
//! - `consistency_checks`: State persistence and platform isolation
//! - `edge_cases`: Error handling and edge cases

use std::sync::LazyLock;

use anyhow::Result;
use omg_lib::package_managers::{PackageManager, mock::MockPackageManager as MockBackend};
use serial_test::serial;
use tempfile::TempDir;

static TEST_DATA_DIR: LazyLock<TempDir> =
    LazyLock::new(|| TempDir::new().expect("failed to create isolated mock state directory"));

struct MockPackageManager;

impl MockPackageManager {
    fn arch() -> MockBackend {
        MockBackend::new_in("arch", TEST_DATA_DIR.path())
    }

    fn debian() -> MockBackend {
        MockBackend::new_in("debian", TEST_DATA_DIR.path())
    }

    fn fedora() -> MockBackend {
        MockBackend::new_in("fedora", TEST_DATA_DIR.path())
    }

    fn macos() -> MockBackend {
        MockBackend::new_in("macos", TEST_DATA_DIR.path())
    }
}

mod all_platforms {
    use super::*;

    #[tokio::test]
    async fn test_arch_mock() -> Result<()> {
        let pm = MockPackageManager::arch();
        assert_eq!(pm.name(), "pacman");

        let results = pm.search("git").await?;
        assert!(!results.is_empty(), "Should find git in Arch mock");
        assert!(results.iter().any(|p| p.name == "git"));

        Ok(())
    }

    #[tokio::test]
    async fn test_debian_mock() -> Result<()> {
        let pm = MockPackageManager::debian();
        assert_eq!(pm.name(), "apt");

        let results = pm.search("git").await?;
        assert!(!results.is_empty(), "Should find git in Debian mock");
        assert!(results.iter().any(|p| p.name == "git"));

        Ok(())
    }

    #[tokio::test]
    async fn test_fedora_mock() -> Result<()> {
        let pm = MockPackageManager::fedora();
        assert_eq!(pm.name(), "dnf");

        let results = pm.search("git").await?;
        assert!(!results.is_empty(), "Should find git in Fedora mock");
        assert!(results.iter().any(|p| p.name == "git"));

        let results = pm.search("rust").await?;
        assert!(results.iter().any(|p| p.name == "rust"));

        Ok(())
    }

    #[tokio::test]
    async fn test_macos_mock() -> Result<()> {
        let pm = MockPackageManager::macos();
        assert_eq!(pm.name(), "homebrew");

        let results = pm.search("git").await?;
        assert!(!results.is_empty(), "Should find git in macOS mock");
        assert!(results.iter().any(|p| p.name == "git"));

        let results = pm.search("node").await?;
        assert!(results.iter().any(|p| p.name == "node"));

        Ok(())
    }
}

mod install_remove_operations {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_arch_install_remove() -> Result<()> {
        let pm = MockPackageManager::arch();

        pm.remove(&["firefox".to_string()]).await?;

        assert!(!pm.is_installed("firefox").await.unwrap());

        pm.install(&["firefox".to_string()]).await?;
        assert!(pm.is_installed("firefox").await.unwrap());

        let installed = pm.list_installed().await?;
        assert!(installed.iter().any(|p| p.name == "firefox"));

        pm.remove(&["firefox".to_string()]).await?;
        assert!(!pm.is_installed("firefox").await.unwrap());

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_fedora_install_remove() -> Result<()> {
        let pm = MockPackageManager::fedora();

        pm.remove(&["vim-enhanced".to_string()]).await?;

        assert!(!pm.is_installed("vim-enhanced").await.unwrap());

        pm.install(&["vim-enhanced".to_string()]).await?;
        assert!(pm.is_installed("vim-enhanced").await.unwrap());

        pm.remove(&["vim-enhanced".to_string()]).await?;
        assert!(!pm.is_installed("vim-enhanced").await.unwrap());

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_macos_install_remove() -> Result<()> {
        let pm = MockPackageManager::macos();

        pm.remove(&["wget".to_string()]).await?;

        assert!(!pm.is_installed("wget").await.unwrap());

        pm.install(&["wget".to_string()]).await?;
        assert!(pm.is_installed("wget").await.unwrap());

        pm.remove(&["wget".to_string()]).await?;
        assert!(!pm.is_installed("wget").await.unwrap());

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_multiple_package_operations() -> Result<()> {
        let pm = MockPackageManager::macos();

        let packages = vec!["git".to_string(), "wget".to_string(), "node".to_string()];

        pm.install(&packages).await?;

        for pkg in &packages {
            assert!(
                pm.is_installed(pkg).await.unwrap(),
                "{pkg} should be installed"
            );
        }

        pm.remove(&packages).await?;

        for pkg in &packages {
            assert!(
                !pm.is_installed(pkg).await.unwrap(),
                "{pkg} should not be installed"
            );
        }

        Ok(())
    }
}

mod update_scenarios {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_arch_updates() -> Result<()> {
        let pm = MockPackageManager::arch();

        pm.set_installed_version("firefox", "121.0")?;
        pm.set_available_version("firefox", "123.0")?;

        let updates = pm.list_updates().await?;

        assert!(
            !updates.is_empty(),
            "Should have updates for firefox: {updates:?}"
        );
        assert!(
            updates.iter().any(|u| u.name == "firefox"),
            "Firefox should be in update list"
        );

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_macos_updates() -> Result<()> {
        let pm = MockPackageManager::macos();

        pm.set_installed_version("node", "20.10.0")?;
        pm.set_available_version("node", "20.12.0")?;

        let updates = pm.list_updates().await?;

        assert!(!updates.is_empty(), "Should have updates: {updates:?}");
        assert!(
            updates.iter().any(|u| u.name == "node"),
            "node should appear in the update list: {updates:?}"
        );

        Ok(())
    }
}

mod search_functionality {
    use super::*;

    #[tokio::test]
    async fn test_fuzzy_search_across_platforms() -> Result<()> {
        let platforms = vec![
            MockPackageManager::arch(),
            MockPackageManager::debian(),
            MockPackageManager::fedora(),
            MockPackageManager::macos(),
        ];

        for pm in platforms {
            let results = pm.search("git").await?;
            assert!(!results.is_empty(), "{} should find git", pm.name());
            assert!(results.iter().any(|p| p.name.contains("git")));
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_description_search() -> Result<()> {
        let pm = MockPackageManager::fedora();

        let results = pm.search("programming").await?;
        assert!(
            results.iter().any(|p| p.name == "rust"),
            "Should find Rust by description search"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_search() -> Result<()> {
        let pm = MockPackageManager::arch();

        let results = pm.search("nonexistent-package-xyz-12345").await?;
        assert!(results.is_empty(), "Should not find nonexistent package");

        Ok(())
    }
}

mod package_info {
    use super::*;

    #[tokio::test]
    async fn test_info_all_platforms() -> Result<()> {
        let test_cases = vec![
            (MockPackageManager::arch(), "git"),
            (MockPackageManager::debian(), "git"),
            (MockPackageManager::fedora(), "git"),
            (MockPackageManager::macos(), "git"),
        ];

        for (pm, pkg) in test_cases {
            let info = pm.info(pkg).await?;
            assert!(info.is_some(), "{} should have info for {}", pm.name(), pkg);

            let package = info.unwrap();
            assert_eq!(package.name, pkg);
            assert!(!package.description.is_empty());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_info_nonexistent() -> Result<()> {
        let pm = MockPackageManager::fedora();

        let info = pm.info("nonexistent-package").await?;
        assert!(
            info.is_none(),
            "Should not find info for nonexistent package"
        );

        Ok(())
    }
}

mod status_reporting {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_get_status_all_platforms() -> Result<()> {
        let platforms = vec![
            MockPackageManager::arch(),
            MockPackageManager::debian(),
            MockPackageManager::fedora(),
            MockPackageManager::macos(),
        ];

        for pm in platforms {
            // Ensure at least one package is installed so status has data
            pm.install(&["git".to_string()]).await?;

            let (total, explicit, _orphans, _updates) = pm.get_status(false).await?;

            // We just installed git, so both counters must reflect it — not
            // merely be nonzero for incidental reasons.
            assert!(total >= 1, "{} should count the installed git", pm.name());
            assert!(
                explicit >= 1,
                "{} should count installed git as explicit",
                pm.name()
            );
            assert!(total >= explicit, "Total should be >= explicit");
        }

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_list_explicit() -> Result<()> {
        let pm = MockPackageManager::macos();

        pm.install(&["git".to_string(), "wget".to_string()]).await?;

        let explicit = pm.list_explicit().await?;

        assert!(explicit.contains(&"git".to_string()));
        assert!(explicit.contains(&"wget".to_string()));

        Ok(())
    }
}

mod consistency_checks {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_state_persistence() -> Result<()> {
        let pm1 = MockPackageManager::fedora();

        pm1.remove(&["rust".to_string()]).await?;

        pm1.install(&["rust".to_string()]).await?;
        assert!(pm1.is_installed("rust").await.unwrap());

        let pm2 = MockPackageManager::fedora();
        assert!(
            pm2.is_installed("rust").await.unwrap(),
            "State should persist across instances"
        );

        pm2.remove(&["rust".to_string()]).await?;

        let pm3 = MockPackageManager::fedora();
        assert!(
            !pm3.is_installed("rust").await.unwrap(),
            "Removal should persist"
        );

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_platform_isolation() -> Result<()> {
        let pm_fedora = MockPackageManager::fedora();
        let pm_arch = MockPackageManager::arch();

        pm_fedora.remove(&["rust".to_string()]).await?;
        pm_arch.remove(&["rust".to_string()]).await?;

        pm_fedora.install(&["rust".to_string()]).await?;

        assert!(pm_fedora.is_installed("rust").await.unwrap());
        assert!(
            !pm_arch.is_installed("rust").await.unwrap(),
            "Fedora installation should not affect Arch mock"
        );

        Ok(())
    }
}

mod edge_cases {
    use super::*;

    #[tokio::test]
    #[serial]
    async fn test_install_already_installed() -> Result<()> {
        let pm = MockPackageManager::macos();

        pm.remove(&["git".to_string()]).await?;

        pm.install(&["git".to_string()]).await?;
        assert!(pm.is_installed("git").await.unwrap());

        pm.install(&["git".to_string()]).await?;
        assert!(
            pm.is_installed("git").await.unwrap(),
            "Reinstall should be idempotent"
        );

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_remove_not_installed() -> Result<()> {
        let pm = MockPackageManager::arch();

        let result = pm.remove(&["not-installed-package".to_string()]).await;
        assert!(
            result.is_ok(),
            "Removing non-installed package should not error"
        );

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_empty_package_list() -> Result<()> {
        let pm = MockPackageManager::debian();

        // Start from a known point: git is absent.
        pm.remove(&["git".to_string()]).await?;

        // Empty batches are accepted as no-ops...
        pm.install(&[]).await?;
        pm.remove(&[]).await?;

        // ...and must not have touched any package state.
        assert!(
            !pm.is_installed("git").await.unwrap(),
            "empty install batch must not install anything"
        );
        assert_eq!(
            pm.list_installed().await?.len(),
            0,
            "empty batches must leave the installed set unchanged"
        );

        Ok(())
    }
}
