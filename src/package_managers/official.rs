use anyhow::Result as AnyhowResult;
use futures::FutureExt;
use futures::future::BoxFuture;
use owo_colors::OwoColorize;

use crate::core::{Package, PackageSource, is_root};
use crate::package_managers::{get_system_status, invalidate_caches, traits::PackageManager};

/// Official Arch Linux package manager with enhanced UX
pub struct OfficialPackageManager;

impl OfficialPackageManager {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub async fn sync_databases(&self) -> AnyhowResult<()> {
        if !is_root() {
            crate::core::privilege::run_self_sudo(&["sync"]).await?;
            invalidate_caches();
            return Ok(());
        }

        crate::package_managers::sync_databases_parallel().await?;
        invalidate_caches();
        Ok(())
    }
}

impl Default for OfficialPackageManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManager for OfficialPackageManager {
    fn name(&self) -> &'static str {
        "pacman"
    }

    fn search(&self, query: &str) -> BoxFuture<'static, AnyhowResult<Vec<Package>>> {
        let query = query.to_string();
        async move {
            // Offload ALPM search to blocking thread
            tokio::task::spawn_blocking(move || {
                // Direct ALPM search is handled by search_sync in alpm_direct.rs
                // but we'll keep this structure for trait compatibility
                let results = crate::package_managers::search_sync(&query)?;
                Ok(results
                    .into_iter()
                    .map(|p| Package {
                        name: p.name,
                        version: p.version,
                        description: p.description,
                        source: PackageSource::Official,
                        installed: p.installed,
                    })
                    .collect())
            })
            .await?
        }
        .boxed()
    }

    fn install(&self, packages: &[String]) -> BoxFuture<'static, AnyhowResult<()>> {
        let packages = packages.to_vec();
        async move {
            // SECURITY: Validate package names
            crate::core::security::validate_package_names(&packages)?;

            if packages.is_empty() {
                return Ok(());
            }

            if !is_root() {
                println!("{} Elevating privileges to install packages...", "→".blue());
                let mut args = vec!["install", "--"];
                let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
                args.extend_from_slice(&pkg_refs);
                crate::core::privilege::run_self_sudo(&args).await?;
                invalidate_caches();
                return Ok(());
            }

            println!(
                "{} Installing {} package(s)...",
                "OMG".cyan().bold(),
                packages.len()
            );
            for pkg in &packages {
                println!("  {} {}", "→".dimmed(), pkg);
            }
            println!();

            // LIGHTNING FAST: Direct libalpm transaction
            crate::package_managers::execute_transaction(packages, false, false)?;
            invalidate_caches();
            Ok(())
        }
        .boxed()
    }

    fn remove(&self, packages: &[String]) -> BoxFuture<'static, AnyhowResult<()>> {
        let packages = packages.to_vec();
        async move {
            // SECURITY: Validate package names
            crate::core::security::validate_package_names(&packages)?;

            if packages.is_empty() {
                return Ok(());
            }

            if !is_root() {
                println!("{} Elevating privileges to remove packages...", "→".blue());
                let mut args = vec!["remove", "--"];
                let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
                args.extend_from_slice(&pkg_refs);
                crate::core::privilege::run_self_sudo(&args).await?;
                invalidate_caches();
                return Ok(());
            }

            println!(
                "{} Removing {} package(s)...",
                "OMG".cyan().bold(),
                packages.len()
            );
            for pkg in &packages {
                println!("  {} {}", "→".dimmed(), pkg);
            }
            println!();

            crate::package_managers::execute_transaction(packages, true, false)?;
            invalidate_caches();
            Ok(())
        }
        .boxed()
    }

    fn update(&self) -> BoxFuture<'static, AnyhowResult<()>> {
        async move {
            if !is_root() {
                println!("{} Elevating privileges to update system...", "→".blue());
                crate::core::privilege::run_self_sudo(&["update"]).await?;
                invalidate_caches();
                return Ok(());
            }

            println!("{} Starting full system upgrade...", "OMG".cyan().bold());
            crate::package_managers::execute_transaction(Vec::new(), false, true)?;
            invalidate_caches();
            Ok(())
        }
        .boxed()
    }

    fn sync(&self) -> BoxFuture<'static, AnyhowResult<()>> {
        async move { OfficialPackageManager::new().sync_databases().await }.boxed()
    }

    fn info(&self, package: &str) -> BoxFuture<'static, AnyhowResult<Option<Package>>> {
        let package = package.to_string();
        async move {
            // SECURITY: Validate package name
            crate::core::security::validate_package_name(&package)?;

            // Try direct ALPM info
            if let Ok(Some(info)) = crate::package_managers::get_package_info(&package) {
                return Ok(Some(Package {
                    name: info.name,
                    version: info.version,
                    description: info.description,
                    source: PackageSource::Official,
                    installed: info.installed,
                }));
            }
            Ok(None)
        }
        .boxed()
    }

    fn list_installed(&self) -> BoxFuture<'static, AnyhowResult<Vec<Package>>> {
        async move {
            // LIGHTNING FAST: Direct ALPM list
            // Offload to blocking thread
            tokio::task::spawn_blocking(move || {
                let pkgs = crate::package_managers::list_installed_fast()?;
                Ok(pkgs
                    .into_iter()
                    .map(|p| Package {
                        name: p.name,
                        version: p.version,
                        description: p.description,
                        source: PackageSource::Official,
                        installed: true,
                    })
                    .collect())
            })
            .await?
        }
        .boxed()
    }

    fn get_status(
        &self,
        _fast: bool,
    ) -> BoxFuture<'static, AnyhowResult<(usize, usize, usize, usize)>> {
        async move { get_system_status().map_err(anyhow::Error::from) }.boxed()
    }

    fn list_explicit(&self) -> BoxFuture<'static, AnyhowResult<Vec<String>>> {
        async move {
            tokio::task::spawn_blocking(crate::package_managers::list_explicit_fast)
                .await?
        }
        .boxed()
    }

    fn list_updates(
        &self,
    ) -> BoxFuture<'static, AnyhowResult<Vec<crate::package_managers::types::UpdateInfo>>> {
        async move {
            tokio::task::spawn_blocking(move || {
                let updates = crate::package_managers::get_update_list()?;
                Ok(updates
                    .into_iter()
                    .map(
                        |(name, old, new)| crate::package_managers::types::UpdateInfo {
                            name,
                            old_version: old.to_string(),
                            new_version: new.to_string(),
                            repo: "official".to_string(),
                        },
                    )
                    .collect())
            })
            .await?
        }
        .boxed()
    }

    fn is_installed(&self, package: &str) -> BoxFuture<'static, bool> {
        let package = package.to_string();
        async move { crate::package_managers::is_installed_fast(&package).unwrap_or(false) }.boxed()
    }
}

pub async fn list_orphans() -> AnyhowResult<Vec<String>> {
    crate::package_managers::list_orphans_direct()
}

pub async fn remove_orphans() -> AnyhowResult<()> {
    let orphans = list_orphans().await?;
    if orphans.is_empty() {
        println!("{} No orphan packages to remove.", "✓".green());
        return Ok(());
    }

    println!(
        "{} Found {} orphan package(s):",
        "OMG".cyan().bold(),
        orphans.len()
    );
    for pkg in &orphans {
        println!("  {} {}", "→".dimmed(), pkg);
    }

    crate::package_managers::execute_transaction(orphans, true, false)?;
    invalidate_caches();
    Ok(())
}

pub async fn list_explicit() -> AnyhowResult<Vec<String>> {
    crate::package_managers::list_explicit_fast()
}

pub async fn is_installed(package: &str) -> bool {
    crate::package_managers::is_installed_fast(package).unwrap_or(false)
}
