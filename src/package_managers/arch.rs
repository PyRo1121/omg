use std::future::Future;
use std::pin::Pin;

use anyhow::Result as AnyhowResult;
use owo_colors::OwoColorize;

use crate::core::{Package, PackageSource, can_write_pacman_db, privilege};
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
///
/// Priority:
/// 1. If we have Linux capabilities (`CAP_DAC_OVERRIDE`) -> run directly (FASTEST)
/// 2. If already root -> run directly
/// 3. Otherwise -> elevate via sudo (with ultra-fast elevated path)
pub async fn run_privileged_operation<F, Fut>(
    command: &str,
    packages: &[String],
    operation: F,
) -> AnyhowResult<()>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = AnyhowResult<()>>,
{
    // FAST PATH: Check capabilities first (zero-overhead if set)
    if can_write_pacman_db() {
        tracing::debug!("Using direct ALPM access (capabilities or root)");
        operation().await?;
        invalidate_caches();
        return Ok(());
    }

    // FALLBACK: Elevate via sudo (uses ultra-fast elevated path)
    tracing::debug!("Elevating privileges for {command}");
    let mut args = vec![command, "--"];
    let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
    args.extend_from_slice(&pkg_refs);
    privilege::run_self_sudo(&args).await?;
    invalidate_caches();
    Ok(())
}

impl PackageManager for ArchPackageManager {
    fn name(&self) -> &'static str {
        "pacman"
    }

    #[inline]
    fn search(
        &self,
        query: &str,
    ) -> Pin<Box<dyn Future<Output = AnyhowResult<Vec<Package>>> + Send + '_>> {
        let query = query.to_string();
        Box::pin(async move {
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
        })
    }

    fn install(
        &self,
        packages: &[String],
    ) -> Pin<Box<dyn Future<Output = AnyhowResult<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
            crate::core::security::validate_package_names_or_files(&packages)?;
            if packages.is_empty() {
                return Ok(());
            }

            run_privileged_operation("install", &packages, || {
                let pkgs = packages.clone();
                async move {
                    tokio::task::spawn_blocking(move || {
                        crate::package_managers::execute_transaction(pkgs, false, false, None)
                    })
                    .await??;
                    Ok(())
                }
            })
            .await
        })
    }

    fn remove(
        &self,
        packages: &[String],
    ) -> Pin<Box<dyn Future<Output = AnyhowResult<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
            crate::core::security::validate_package_names(&packages)?;
            if packages.is_empty() {
                return Ok(());
            }

            run_privileged_operation("remove", &packages, || {
                let pkgs = packages.clone();
                async move {
                    tokio::task::spawn_blocking(move || {
                        crate::package_managers::execute_transaction(pkgs, true, false, None)
                    })
                    .await??;
                    Ok(())
                }
            })
            .await
        })
    }

    fn update(&self) -> Pin<Box<dyn Future<Output = AnyhowResult<()>> + Send + '_>> {
        Box::pin(async move {
            run_privileged_operation("update", &[], || async {
                let prefix = format!("{}", "OMG".cyan().bold());
                tracing::info!("{prefix} Starting full system upgrade...");
                tokio::task::spawn_blocking(move || {
                    crate::package_managers::execute_transaction(Vec::new(), false, true, None)
                })
                .await??;
                Ok(())
            })
            .await
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = AnyhowResult<()>> + Send + '_>> {
        Box::pin(async move {
            run_privileged_operation("sync", &[], || async {
                crate::package_managers::sync_databases_parallel().await?;
                Ok(())
            })
            .await
        })
    }

    #[inline]
    fn info(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = AnyhowResult<Option<Package>>> + Send + '_>> {
        let package = package.to_string();
        Box::pin(async move {
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
        })
    }

    #[inline]
    fn list_installed(
        &self,
    ) -> Pin<Box<dyn Future<Output = AnyhowResult<Vec<Package>>> + Send + '_>> {
        Box::pin(async move {
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
        })
    }

    fn get_status(
        &self,
        _fast: bool,
    ) -> Pin<Box<dyn Future<Output = AnyhowResult<(usize, usize, usize, usize)>> + Send + '_>> {
        Box::pin(async move { get_system_status() })
    }

    #[inline]
    fn list_explicit(
        &self,
    ) -> Pin<Box<dyn Future<Output = AnyhowResult<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            tokio::task::spawn_blocking(crate::package_managers::list_explicit_fast).await?
        })
    }

    #[inline]
    fn list_updates(
        &self,
    ) -> Pin<
        Box<
            dyn Future<Output = AnyhowResult<Vec<crate::package_managers::types::UpdateInfo>>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            tokio::task::spawn_blocking(crate::package_managers::get_update_list).await?
        })
    }

    fn is_installed(&self, package: &str) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = crate::package_managers::is_installed_fast(package).unwrap_or_default();
        Box::pin(async move { result })
    }
}

pub async fn list_orphans() -> AnyhowResult<Vec<String>> {
    crate::package_managers::list_orphans_direct()
}

pub async fn remove_orphans() -> AnyhowResult<()> {
    let orphans = list_orphans().await?;
    if orphans.is_empty() {
        tracing::info!("{} No orphan packages to remove.", "✓".green());
        return Ok(());
    }

    let prefix = format!("{}", "OMG".cyan().bold());
    let count = orphans.len();
    tracing::info!("{prefix} Found {count} orphan package(s):");
    for pkg in &orphans {
        tracing::info!("  {} {}", "→".dimmed(), pkg);
    }

    crate::package_managers::execute_transaction(orphans, true, false, None)?;
    invalidate_caches();
    Ok(())
}

pub async fn list_explicit() -> AnyhowResult<Vec<String>> {
    crate::package_managers::list_explicit_fast()
}

pub async fn is_installed(package: &str) -> bool {
    crate::package_managers::is_installed_fast(package).unwrap_or_default()
}
