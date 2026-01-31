#![cfg(target_os = "macos")]

use anyhow::Result;
use omg_lib::package_managers::{HomebrewPackageManager, PackageManager};

mod homebrew_integration {
    use super::*;

    #[tokio::test]
    async fn test_homebrew_package_manager_creation() {
        let pm = HomebrewPackageManager::new();
        assert_eq!(pm.name(), "homebrew");
    }

    #[tokio::test]
    async fn test_search_common_formula() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        let results = pm.search("wget").await?;
        
        assert!(!results.is_empty(), "Should find wget formula");
        assert!(
            results.iter().any(|p| p.name == "wget"),
            "Results should contain wget"
        );
        
        Ok(())
    }

    #[tokio::test]
    async fn test_search_nonexistent_formula() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        let results = pm.search("nonexistent-formula-xyz-12345").await?;
        
        assert!(results.is_empty(), "Should not find nonexistent formula");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_list_installed_formulae() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        let installed = pm.list_installed().await?;
        
        Ok(())
    }

    #[tokio::test]
    async fn test_formula_info() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        let info = pm.info("wget").await?;
        
        if let Some(pkg) = info {
            assert_eq!(pkg.name, "wget");
            assert!(!pkg.description.is_empty(), "Description should not be empty");
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_list_updates() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        let updates = pm.list_updates().await?;
        
        Ok(())
    }

    #[tokio::test]
    async fn test_is_installed_check() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        let _is_brew_installed = pm.is_installed("homebrew").await;
        
        Ok(())
    }
}

mod homebrew_cellar_operations {
    use super::*;

    #[tokio::test]
    #[ignore]
    async fn test_install_and_remove_formula() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        let test_formula = "hello";
        
        let initial_check = pm.is_installed(test_formula).await;
        if initial_check {
            pm.remove(&[test_formula.to_string()]).await?;
        }
        
        pm.install(&[test_formula.to_string()]).await?;
        assert!(pm.is_installed(test_formula).await, "Formula should be installed");
        
        pm.remove(&[test_formula.to_string()]).await?;
        assert!(!pm.is_installed(test_formula).await, "Formula should be removed");
        
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_update_all_formulae() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        pm.update().await?;
        
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_sync_formula_metadata() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        pm.sync().await?;
        
        Ok(())
    }
}

mod homebrew_error_handling {
    use super::*;

    #[tokio::test]
    async fn test_install_invalid_formula() {
        let pm = HomebrewPackageManager::new();
        
        let result = pm.install(&["nonexistent-formula-xyz-12345".to_string()]).await;
        
        assert!(result.is_err(), "Should fail to install nonexistent formula");
    }

    #[tokio::test]
    async fn test_remove_not_installed_formula() {
        let pm = HomebrewPackageManager::new();
        
        let result = pm.remove(&["not-installed-formula-xyz".to_string()]).await;
        
    }

    #[tokio::test]
    async fn test_empty_formula_list_install() {
        let pm = HomebrewPackageManager::new();
        
        let result = pm.install(&[]).await;
        
    }
}

mod homebrew_performance {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_search_performance() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        let start = Instant::now();
        let _results = pm.search("python").await?;
        let duration = start.elapsed();
        
        assert!(
            duration.as_millis() < 100,
            "Search should complete in <100ms with cache, got {:?}",
            duration
        );
        
        Ok(())
    }

    #[tokio::test]
    async fn test_list_installed_performance() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        let start = Instant::now();
        let _installed = pm.list_installed().await?;
        let duration = start.elapsed();
        
        assert!(
            duration.as_millis() < 50,
            "List installed should complete in <50ms (direct filesystem), got {:?}",
            duration
        );
        
        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_searches_consistency() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        let results1 = pm.search("python").await?;
        let results2 = pm.search("python").await?;
        
        assert_eq!(
            results1.len(),
            results2.len(),
            "Multiple searches should return consistent results"
        );
        
        Ok(())
    }
}

mod homebrew_api {
    use super::*;

    #[tokio::test]
    async fn test_formula_api_fetch() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        pm.sync().await?;
        
        let results = pm.search("node").await?;
        assert!(!results.is_empty(), "Should find formulae after sync");
        
        Ok(())
    }

    #[tokio::test]
    async fn test_cask_search() -> Result<()> {
        let pm = HomebrewPackageManager::new();
        
        let results = pm.search("visual-studio-code").await?;
        
        Ok(())
    }
}

mod homebrew_cellar_detection {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn test_cellar_path_detection() {
        let arm_path = Path::new("/opt/homebrew/Cellar");
        let intel_path = Path::new("/usr/local/Cellar");
        
        let homebrew_exists = arm_path.exists() || intel_path.exists();
        
        if !homebrew_exists {
            eprintln!("WARNING: No Homebrew installation detected");
        }
    }

    #[tokio::test]
    async fn test_install_receipt_parsing() -> Result<()> {
        use std::path::PathBuf;
        use tokio::fs;
        
        let cellar_paths = vec![
            PathBuf::from("/opt/homebrew/Cellar"),
            PathBuf::from("/usr/local/Cellar"),
        ];
        
        for cellar in cellar_paths {
            if !cellar.exists() {
                continue;
            }
            
            let mut entries = fs::read_dir(&cellar).await?;
            
            if let Some(entry) = entries.next_entry().await? {
                let formula_path = entry.path();
                
                if let Some(version_dir) = fs::read_dir(&formula_path)
                    .await
                    .ok()
                    .and_then(|mut dirs| dirs.blocking_recv())
                {
                    if let Some(version_entry) = version_dir {
                        let receipt_path = version_entry.path().join("INSTALL_RECEIPT.json");
                        
                        if receipt_path.exists() {
                            let content = fs::read_to_string(&receipt_path).await?;
                            let _receipt: serde_json::Value = serde_json::from_str(&content)?;
                            
                            break;
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
}
