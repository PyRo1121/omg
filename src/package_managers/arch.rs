use anyhow::Result;
use async_trait::async_trait;
use colored::Colorize;

use super::PackageManager;
use crate::core::{Package, PackageSource};

/// Arch Linux package manager with enhanced UX
pub struct ArchPackageManager;

impl ArchPackageManager {
    pub fn new() -> Self {
        ArchPackageManager
    }
}

impl Default for ArchPackageManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PackageManager for ArchPackageManager {
    fn name(&self) -> &'static str {
        "pacman"
    }

    async fn search(&self, query: &str) -> Result<Vec<Package>> {
        // Direct ALPM search is handled by search_sync in alpm_direct.rs
        // but we'll keep this structure for trait compatibility
        let results = crate::package_managers::search_sync(query)?;
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
    }

    async fn install(&self, packages: &[String]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        println!(
            "{} Installing {} package(s)...",
            "OMG".cyan().bold(),
            packages.len()
        );
        for pkg in packages {
            println!("  {} {}", "→".dimmed(), pkg);
        }
        println!();

        // LIGHTNING FAST: Direct libalpm transaction
        crate::package_managers::execute_transaction(packages.to_vec(), false, false)?;

        println!("{} All packages processed successfully!", "✓".green());
        Ok(())
    }

    async fn remove(&self, packages: &[String]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        println!(
            "{} Removing {} package(s)...",
            "OMG".cyan().bold(),
            packages.len()
        );

        // LIGHTNING FAST: Direct libalpm transaction
        crate::package_managers::execute_transaction(packages.to_vec(), true, false)?;

        println!("{} Packages removed successfully!", "✓".green());
        Ok(())
    }

    async fn update(&self) -> Result<()> {
        println!("{} Updating system...\n", "OMG".cyan().bold());

        // LIGHTNING FAST: Direct libalpm sysupgrade
        crate::package_managers::execute_transaction(Vec::new(), false, true)?;

        println!("\n{} System updated successfully!", "✓".green());
        Ok(())
    }

    async fn info(&self, package: &str) -> Result<Option<Package>> {
        // LIGHTNING FAST: Direct ALPM info lookup
        if let Some(info) = crate::package_managers::get_sync_pkg_info(package)? {
            return Ok(Some(Package {
                name: info.name,
                version: info.version,
                description: info.description,
                source: PackageSource::Official,
                installed: crate::package_managers::is_installed_fast(package).unwrap_or(false),
            }));
        }
        Ok(None)
    }

    async fn list_installed(&self) -> Result<Vec<Package>> {
        // LIGHTNING FAST: Direct ALPM list
        let installed = crate::package_managers::list_installed_fast()?;
        Ok(installed
            .into_iter()
            .map(|p| Package {
                name: p.name,
                version: p.version,
                description: p.description,
                source: PackageSource::Official,
                installed: true,
            })
            .collect())
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
