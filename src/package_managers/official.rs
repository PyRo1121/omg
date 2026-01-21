use anyhow::Result;
use futures::future::{BoxFuture, FutureExt};
use owo_colors::OwoColorize;

use super::PackageManager;
use crate::core::{Package, PackageSource, is_root};
use crate::package_managers::invalidate_caches;

/// Official Arch Linux package manager with enhanced UX
pub struct OfficialPackageManager;

impl OfficialPackageManager {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub async fn sync_databases(&self) -> Result<()> {
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

    fn search(&self, query: &str) -> BoxFuture<'static, Result<Vec<Package>>> {
        let query = query.to_string();
        async move {
            // Direct ALPM search is handled by search_sync in alpm_direct.rs
            // but we'll keep this structure for trait compatibility
            let results = crate::package_managers::search_sync(&query)?;
            Ok(results
                .into_iter()
                .map(|p| Package {
                    name: p.name,
                    version: p.version.clone(),
                    description: p.description,
                    source: PackageSource::Official,
                    installed: p.installed,
                })
                .collect())
        }
        .boxed()
    }

    fn install(&self, packages: &[String]) -> BoxFuture<'static, Result<()>> {
        let packages = packages.to_vec();
        async move {
            if packages.is_empty() {
                return Ok(());
            }

            if !is_root() {
                println!("{} Elevating privileges to install packages...", "→".blue());
                let mut args = vec!["install"];
                let pkg_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
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

            println!("{} All packages processed successfully!", "✓".green());
            invalidate_caches();
            Ok(())
        }
        .boxed()
    }

    fn remove(&self, packages: &[String]) -> BoxFuture<'static, Result<()>> {
        let packages = packages.to_vec();
        async move {
            if packages.is_empty() {
                return Ok(());
            }

            if !is_root() {
                println!("{} Elevating privileges to remove packages...", "→".blue());
                let mut args = vec!["remove"];
                let pkg_refs: Vec<&str> = packages.iter().map(|s| s.as_str()).collect();
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

            // LIGHTNING FAST: Direct libalpm transaction
            crate::package_managers::execute_transaction(packages, true, false)?;

            println!("{} Packages removed successfully!", "✓".green());
            invalidate_caches();
            Ok(())
        }
        .boxed()
    }

    fn update(&self) -> BoxFuture<'static, Result<()>> {
        async move {
            if !is_root() {
                println!("{} Elevating privileges to update system...", "→".blue());
                crate::core::privilege::run_self_sudo(&["update"]).await?;
                invalidate_caches();
                return Ok(());
            }

            println!("{} Updating system...\n", "OMG".cyan().bold());

            // STEP 1: Sync databases in PARALLEL (3-5x faster than pacman -Sy)
            crate::package_managers::sync_databases_parallel().await?;

            // STEP 2: Run sysupgrade transaction
            crate::package_managers::execute_transaction(Vec::new(), false, true)?;

            println!("\n{} System updated successfully!", "✓".green());
            invalidate_caches();
            Ok(())
        }
        .boxed()
    }

    fn sync(&self) -> BoxFuture<'static, Result<()>> {
        async move {
            OfficialPackageManager::new().sync_databases().await
        }
        .boxed()
    }

    fn info(&self, package: &str) -> BoxFuture<'static, Result<Option<Package>>> {
        let package = package.to_string();
        async move {
            // LIGHTNING FAST: Direct ALPM info lookup
            if let Some(info) = crate::package_managers::get_sync_pkg_info(&package)? {
                return Ok(Some(Package {
                    name: info.name,
                    version: info.version.clone(),
                    description: info.description,
                    source: PackageSource::Official,
                    installed: crate::package_managers::is_installed_fast(&package).unwrap_or(false),
                }));
            }
            Ok(None)
        }
        .boxed()
    }

    fn list_installed(&self) -> BoxFuture<'static, Result<Vec<Package>>> {
        async move {
            // LIGHTNING FAST: Direct ALPM list
            let installed = crate::package_managers::list_installed_fast()?;
            Ok(installed
                .into_iter()
                .map(|p| Package {
                    name: p.name,
                    version: p.version.clone(),
                    description: p.description,
                    source: PackageSource::Official,
                    installed: true,
                })
                .collect())
        }
        .boxed()
    }

    fn get_status(&self) -> BoxFuture<'static, Result<(usize, usize, usize, usize)>> {
        async move { crate::package_managers::get_system_status() }.boxed()
    }

    fn list_explicit(&self) -> BoxFuture<'static, Result<Vec<String>>> {
        async move { crate::package_managers::list_explicit_fast() }.boxed()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ADDITIONAL COMMANDS (NATIVE)
// ═══════════════════════════════════════════════════════════════════════════

/// List orphan packages (not required by any other package)
pub async fn list_orphans() -> Result<Vec<String>> {
    crate::package_managers::list_orphans_direct()
}

/// Remove orphan packages
pub async fn remove_orphans() -> Result<()> {
    let orphans = list_orphans().await?;

    if orphans.is_empty() {
        println!("{} No orphan packages found", "✓".green());
        return Ok(());
    }

    println!("{} Found {} orphan packages:", "→".blue(), orphans.len());
    for pkg in &orphans {
        println!("  {} {}", "○".dimmed(), pkg);
    }

    // LIGHTNING FAST: Direct ALPM removal
    crate::package_managers::execute_transaction(orphans, true, false)?;

    Ok(())
}

/// Check if package is installed
pub async fn is_installed(package: &str) -> bool {
    crate::package_managers::is_installed_fast(package).unwrap_or(false)
}

/// Get explicitly installed packages
pub async fn list_explicit() -> Result<Vec<String>> {
    crate::package_managers::list_explicit_fast()
}
