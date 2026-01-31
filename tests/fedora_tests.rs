#![cfg(all(target_os = "linux", feature = "fedora"))]

use anyhow::Result;
use omg_lib::package_managers::{DnfPackageManager, PackageManager};

mod dnf_integration {
    use super::*;

    #[tokio::test]
    async fn test_dnf_package_manager_creation() {
        let pm = DnfPackageManager::new();
        assert_eq!(pm.name(), "dnf");
    }

    #[tokio::test]
    async fn test_search_common_package() -> Result<()> {
        let pm = DnfPackageManager::new();

        let results = pm.search("vim").await?;

        assert!(!results.is_empty(), "Should find vim package");
        assert!(
            results.iter().any(|p| p.name.contains("vim")),
            "Results should contain vim"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_search_nonexistent_package() -> Result<()> {
        let pm = DnfPackageManager::new();

        let results = pm.search("nonexistent-package-xyz-12345").await?;

        assert!(results.is_empty(), "Should not find nonexistent package");

        Ok(())
    }

    #[tokio::test]
    async fn test_list_installed_packages() -> Result<()> {
        let pm = DnfPackageManager::new();

        let installed = pm.list_installed().await?;

        assert!(
            !installed.is_empty(),
            "Should have installed packages on Fedora system"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_package_info() -> Result<()> {
        let pm = DnfPackageManager::new();

        let info = pm.info("vim-minimal").await?;

        if let Some(pkg) = info {
            assert!(pkg.name.contains("vim"));
            assert!(!pkg.version.is_empty(), "Version should not be empty");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_list_updates() -> Result<()> {
        let pm = DnfPackageManager::new();

        let _updates = pm.list_updates().await?;

        Ok(())
    }

    #[tokio::test]
    async fn test_is_installed_check() -> Result<()> {
        let pm = DnfPackageManager::new();

        let is_bash_installed = pm.is_installed("bash").await;
        assert!(
            is_bash_installed,
            "bash should be installed on Fedora system"
        );

        Ok(())
    }
}

mod dnf_rpm_database {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn test_rpm_database_exists() {
        let rpm_db_path = Path::new("/var/lib/rpm/rpmdb.sqlite");

        if rpm_db_path.exists() {
            assert!(rpm_db_path.is_file(), "RPM database should be a file");
        } else {
            eprintln!("WARNING: RPM database not found at /var/lib/rpm/rpmdb.sqlite");
        }
    }

    #[tokio::test]
    async fn test_rpm_database_query() -> Result<()> {
        let pm = DnfPackageManager::new();

        let installed = pm.list_installed().await?;

        assert!(
            !installed.is_empty(),
            "Should read packages from RPM database"
        );

        let has_rpm = installed.iter().any(|p| p.name.contains("rpm"));
        assert!(has_rpm, "Should find rpm package itself in database");

        Ok(())
    }
}

mod dnf_operations {
    use super::*;

    #[tokio::test]
    #[ignore = "requires root privileges and modifies system"]
    async fn test_install_and_remove_package() -> Result<()> {
        if !omg_lib::core::is_root() {
            eprintln!("Skipping: requires root");
            return Ok(());
        }

        let pm = DnfPackageManager::new();

        let test_package = "nano";

        let initial_check = pm.is_installed(test_package).await;
        if initial_check {
            pm.remove(&[test_package.to_string()]).await?;
        }

        pm.install(&[test_package.to_string()]).await?;
        assert!(
            pm.is_installed(test_package).await,
            "Package should be installed"
        );

        pm.remove(&[test_package.to_string()]).await?;
        assert!(
            !pm.is_installed(test_package).await,
            "Package should be removed"
        );

        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires root privileges and modifies system"]
    async fn test_update_all_packages() -> Result<()> {
        if !omg_lib::core::is_root() {
            eprintln!("Skipping: requires root");
            return Ok(());
        }

        let pm = DnfPackageManager::new();

        pm.update().await?;

        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires root privileges and modifies system"]
    async fn test_sync_repository_metadata() -> Result<()> {
        if !omg_lib::core::is_root() {
            eprintln!("Skipping: requires root");
            return Ok(());
        }

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

    #[tokio::test]
    async fn test_remove_not_installed_package() {
        if !omg_lib::core::is_root() {
            return;
        }

        let pm = DnfPackageManager::new();

        let _result = pm.remove(&["not-installed-package-xyz".to_string()]).await;
    }

    #[tokio::test]
    async fn test_empty_package_list_install() {
        if !omg_lib::core::is_root() {
            return;
        }

        let pm = DnfPackageManager::new();

        let _result = pm.install(&[]).await;
    }
}

mod dnf_performance {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn test_search_performance() -> Result<()> {
        let pm = DnfPackageManager::new();

        let start = Instant::now();
        let _results = pm.search("python").await?;
        let duration = start.elapsed();

        assert!(
            duration.as_millis() < 500,
            "Search should complete in <500ms with SQLite cache, got {duration:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_list_installed_performance() -> Result<()> {
        let pm = DnfPackageManager::new();

        let start = Instant::now();
        let _installed = pm.list_installed().await?;
        let duration = start.elapsed();

        assert!(
            duration.as_millis() < 100,
            "List installed should complete in <100ms (SQLite query), got {duration:?}"
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_multiple_searches_consistency() -> Result<()> {
        let pm = DnfPackageManager::new();

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

mod dnf_copr {
    use super::*;

    #[tokio::test]
    async fn test_copr_search() -> Result<()> {
        let pm = DnfPackageManager::new();

        let _results = pm.search("rust").await?;

        Ok(())
    }
}

mod dnf_repository_metadata {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn test_repository_files_exist() {
        let repos_dir = Path::new("/etc/yum.repos.d");

        if !repos_dir.exists() {
            eprintln!("WARNING: /etc/yum.repos.d not found");
            return;
        }

        assert!(repos_dir.is_dir(), "Repos directory should exist");
    }

    #[tokio::test]
    async fn test_parse_repository_metadata() -> Result<()> {
        use std::fs;

        let repos_dir = Path::new("/etc/yum.repos.d");

        if !repos_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(repos_dir)?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "repo") {
                let content = fs::read_to_string(&path)?;

                assert!(
                    content.contains('[') && content.contains(']'),
                    "Repo file should contain sections"
                );

                break;
            }
        }

        Ok(())
    }
}
