use anyhow::Result as AnyhowResult;
use async_trait::async_trait;
use futures::future::Future;
use owo_colors::OwoColorize;

use crate::core::{Package, PackageSource, is_root, privilege};
use crate::package_managers::{get_system_status, invalidate_caches, traits::PackageManager};

/// Arch Linux package manager (ALPM) implementation
pub struct ArchPackageManager;

impl ArchPackageManager {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Default for ArchPackageManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper to run a privileged operation, either directly or via sudo.
async fn run_privileged_operation<F, Fut>(
    command: &str,
    packages: &[String],
    operation: F,
) -> AnyhowResult<()>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = AnyhowResult<()>>,
{
    if !is_root() {
        println!("{} Elevating privileges for {command}...", "→".blue());
        let mut args = vec![command, "--"];
        let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
        args.extend_from_slice(&pkg_refs);
        privilege::run_self_sudo(&args).await?;
        invalidate_caches();
        return Ok(());
    }

    operation().await?;
    invalidate_caches();
    Ok(())
}

#[async_trait]
impl PackageManager for ArchPackageManager {
    fn name(&self) -> &'static str {
        "pacman"
    }

    async fn search(&self, query: &str) -> AnyhowResult<Vec<Package>> {
        let query = query.to_string();
        // Offload ALPM search to blocking thread
        tokio::task::spawn_blocking(move || {
            // Direct ALPM search is handled by search_sync in alpm_direct.rs
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

    async fn install(&self, packages: &[String]) -> AnyhowResult<()> {
        crate::core::security::validate_package_names(packages)?;
        if packages.is_empty() {
            return Ok(());
        }

        let pkgs_clone = packages.to_vec();
        let packages = packages.to_vec();
        run_privileged_operation("install", &packages, || async move {
            tokio::task::spawn_blocking(move || {
                crate::package_managers::execute_transaction(pkgs_clone, false, false, None)
            })
            .await??;
            Ok(())
        })
        .await
    }

    async fn remove(&self, packages: &[String]) -> AnyhowResult<()> {
        crate::core::security::validate_package_names(packages)?;
        if packages.is_empty() {
            return Ok(());
        }

        let pkgs_clone = packages.to_vec();
        let packages = packages.to_vec();
        run_privileged_operation("remove", &packages, || async move {
            tokio::task::spawn_blocking(move || {
                crate::package_managers::execute_transaction(pkgs_clone, true, false, None)
            })
            .await??;
            Ok(())
        })
        .await
    }

    async fn update(&self) -> AnyhowResult<()> {
        run_privileged_operation("update", &[], || async {
            println!("{} Starting full system upgrade...", "OMG".cyan().bold());
            tokio::task::spawn_blocking(move || {
                crate::package_managers::execute_transaction(Vec::new(), false, true, None)
            })
            .await??;
            Ok(())
        })
        .await
    }

    async fn sync(&self) -> AnyhowResult<()> {
        run_privileged_operation("sync", &[], || async {
            crate::package_managers::sync_databases_parallel().await?;
            Ok(())
        })
        .await
    }

    async fn info(&self, package: &str) -> AnyhowResult<Option<Package>> {
        // SECURITY: Validate package name
        crate::core::security::validate_package_name(package)?;

        // Try direct ALPM info
        if let Ok(Some(info)) = crate::package_managers::get_package_info(package) {
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

    async fn list_installed(&self) -> AnyhowResult<Vec<Package>> {
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

    async fn get_status(&self, _fast: bool) -> AnyhowResult<(usize, usize, usize, usize)> {
        get_system_status()
    }

    async fn list_explicit(&self) -> AnyhowResult<Vec<String>> {
        tokio::task::spawn_blocking(crate::package_managers::list_explicit_fast).await?
    }

    async fn list_updates(&self) -> AnyhowResult<Vec<crate::package_managers::types::UpdateInfo>> {
        tokio::task::spawn_blocking(crate::package_managers::get_update_list).await?
    }

    async fn is_installed(&self, package: &str) -> bool {
        crate::package_managers::is_installed_fast(package).unwrap_or(false)
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

    crate::package_managers::execute_transaction(orphans, true, false, None)?;
    invalidate_caches();
    Ok(())
}

pub async fn list_explicit() -> AnyhowResult<Vec<String>> {
    crate::package_managers::list_explicit_fast()
}

pub async fn is_installed(package: &str) -> bool {
    crate::package_managers::is_installed_fast(package).unwrap_or(false)
}
