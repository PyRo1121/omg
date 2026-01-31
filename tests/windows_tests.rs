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
            results.iter().any(|p| p.name.to_lowercase().contains("git")),
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
        
        let is_scoop_installed = pm.is_installed("scoop").await;
        
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
        
        let initial_check = pm.is_installed(test_package).await;
        if initial_check {
            pm.remove(&[test_package.to_string()]).await?;
        }
        
        pm.install(&[test_package.to_string()]).await?;
        assert!(pm.is_installed(test_package).await, "Package should be installed");
        
        pm.remove(&[test_package.to_string()]).await?;
        assert!(!pm.is_installed(test_package).await, "Package should be removed");
        
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
        
        let result = pm.install(&["nonexistent-package-xyz-12345".to_string()]).await;
        
        assert!(result.is_err(), "Should fail to install nonexistent package");
    }

    #[tokio::test]
    async fn test_remove_not_installed_package() {
        let pm = WindowsPackageManager::new();
        
        let result = pm.remove(&["not-installed-package-xyz".to_string()]).await;
        
    }

    #[tokio::test]
    async fn test_empty_package_list_install() {
        let pm = WindowsPackageManager::new();
        
        let result = pm.install(&[]).await;
        
    }
}

mod windows_performance {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_search_performance() -> Result<()> {
        let pm = WindowsPackageManager::new();
        
        let start = Instant::now();
        let _results = pm.search("git").await?;
        let duration = start.elapsed();
        
        assert!(
            duration.as_millis() < 100,
            "Search should complete in <100ms with mmap cache, got {:?}",
            duration
        );
        
        Ok(())
    }

    #[tokio::test]
    async fn test_list_installed_performance() -> Result<()> {
        let pm = WindowsPackageManager::new();
        
        let start = Instant::now();
        let _installed = pm.list_installed().await?;
        let duration = start.elapsed();
        
        assert!(
            duration.as_millis() < 500,
            "List installed should complete in <500ms, got {:?}",
            duration
        );
        
        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_searches_consistency() -> Result<()> {
        let pm = WindowsPackageManager::new();
        
        let results1 = pm.search("git").await?;
        let results2 = pm.search("git").await?;
        
        assert_eq!(
            results1.len(),
            results2.len(),
            "Multiple searches should return consistent results"
        );
        
        Ok(())
    }
}

mod windows_registry {
    use super::*;

    #[tokio::test]
    async fn test_registry_enumeration() -> Result<()> {
        let pm = WindowsPackageManager::new();
        
        let all_packages = pm.list_installed().await?;
        
        let has_registry_packages = all_packages
            .iter()
            .any(|p| p.name.contains("Microsoft") || p.name.contains("Windows"));
        
        Ok(())
    }
}
