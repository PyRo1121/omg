#![cfg(target_os = "windows")]

use anyhow::Result;
use omg_lib::package_managers::{PackageManager, WindowsPackageManager};

mod windows_integration {
    use super::*;

    #[tokio::test]
    async fn test_windows_package_manager_creation() {
        let pm = WindowsPackageManager::new();
        assert_eq!(pm.name(), "scoop");
    }

    #[tokio::test]
    async fn test_search_common_package() -> Result<()> {
        let pm = WindowsPackageManager::new();

        let results = pm.search("git").await?;

        assert!(!results.is_empty(), "Should find git package");
        assert!(
            results
                .iter()
                .any(|p| p.name.to_lowercase().contains("git")),
            "Results should contain git"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_search_nonexistent_package() -> Result<()> {
        let pm = WindowsPackageManager::new();

        let results = pm.search("nonexistent-package-xyz-12345").await?;

        assert!(results.is_empty(), "Should not find nonexistent package");

        Ok(())
    }

    #[tokio::test]
    async fn test_list_installed_packages() -> Result<()> {
        let pm = WindowsPackageManager::new();

        let installed = pm.list_installed().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_package_info() -> Result<()> {
        let pm = WindowsPackageManager::new();

        let info = pm.info("git").await?;

        if let Some(pkg) = info {
            assert_eq!(pkg.name, "git");
            assert!(!pkg.version.is_empty(), "Version should not be empty");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_list_updates() -> Result<()> {
        let pm = WindowsPackageManager::new();

        let updates = pm.list_updates().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_is_installed_check() -> Result<()> {
        let pm = WindowsPackageManager::new();

        let is_scoop_installed = pm.is_installed("scoop").await.unwrap();

        Ok(())
    }
}

mod windows_libscoop_integration {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_install_and_remove_package() -> Result<()> {
        let pm = WindowsPackageManager::new();

        let test_package = "hello";

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
    #[ignore]
    async fn test_sync_bucket_metadata() -> Result<()> {
        let pm = WindowsPackageManager::new();

        pm.sync().await?;

        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_update_all_packages() -> Result<()> {
        let pm = WindowsPackageManager::new();

        pm.update().await?;

        Ok(())
    }
}

mod windows_error_handling {
    use super::*;

    #[tokio::test]
    async fn test_install_invalid_package() {
        let pm = WindowsPackageManager::new();

        let result = pm
            .install(&["nonexistent-package-xyz-12345".to_string()])
            .await;

        assert!(
            result.is_err(),
            "Should fail to install nonexistent package"
        );
    }
}
