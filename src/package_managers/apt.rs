//! Debian/Ubuntu package manager backend
//!
//! This module provides full Debian/Ubuntu support using `rust-apt` FFI bindings.
//! It requires `libapt-pkg-dev` to be installed on the system.

use anyhow::{Context, Result};
use futures::future::{BoxFuture, FutureExt};

use crate::core::is_root;
use crate::core::{Package, PackageSource};
use crate::package_managers::types::{LocalPackage, PackageInfo, SyncPackage};

// Import rust-apt for full package management
use rust_apt::Cache;
use rust_apt::cache::{PackageSort, Upgrade};
use rust_apt::progress::{AcquireProgress, InstallProgress};

#[derive(Debug, Default)]
pub struct AptPackageManager;

impl AptPackageManager {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    pub async fn sync_databases(&self) -> Result<()> {
        if !is_root() {
            crate::core::privilege::run_self_sudo(&["sync"]).await?;
            return Ok(());
        }

        tokio::task::spawn_blocking(sync_databases_blocking)
            .await
            .context("APT sync task failed")??;
        Ok(())
    }
}

impl crate::package_managers::PackageManager for AptPackageManager {
    fn name(&self) -> &'static str {
        "apt"
    }

    fn search(&self, query: &str) -> BoxFuture<'static, Result<Vec<Package>>> {
        let query = query.to_string();
        async move {
            // Try fast path first
            let fast_results = super::debian_db::search_fast(&query);
            if let Ok(results) = fast_results {
                if !results.is_empty() {
                    return Ok(results);
                }
            }

            let results = tokio::task::spawn_blocking(move || search_sync(&query))
                .await
                .context("APT search task failed")??;

            Ok(sync_to_packages(results))
        }
        .boxed()
    }

    fn install(&self, packages: &[String]) -> BoxFuture<'static, Result<()>> {
        let packages = packages.to_vec();
        async move {
            // SECURITY: Validate package names
            crate::core::security::validate_package_names(&packages)?;

            if !is_root() {
                let mut args = vec!["install", "--"];
                let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
                args.extend_from_slice(&pkg_refs);
                crate::core::privilege::run_self_sudo(&args).await?;
                return Ok(());
            }

            tokio::task::spawn_blocking(move || install_blocking(&packages))
                .await
                .context("APT install task failed")??;
            Ok(())
        }
        .boxed()
    }

    fn remove(&self, packages: &[String]) -> BoxFuture<'static, Result<()>> {
        let packages = packages.to_vec();
        async move {
            // SECURITY: Validate package names
            crate::core::security::validate_package_names(&packages)?;

            if !is_root() {
                let mut args = vec!["remove", "--"];
                let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
                args.extend_from_slice(&pkg_refs);
                crate::core::privilege::run_self_sudo(&args).await?;
                return Ok(());
            }

            tokio::task::spawn_blocking(move || remove_blocking(&packages))
                .await
                .context("APT remove task failed")??;
            Ok(())
        }
        .boxed()
    }

    fn update(&self) -> BoxFuture<'static, Result<()>> {
        async move {
            if !is_root() {
                crate::core::privilege::run_self_sudo(&["update"]).await?;
                return Ok(());
            }

            tokio::task::spawn_blocking(update_blocking)
                .await
                .context("APT update task failed")??;
            Ok(())
        }
        .boxed()
    }

    fn sync(&self) -> BoxFuture<'static, Result<()>> {
        async move { AptPackageManager::new().sync_databases().await }.boxed()
    }

    fn info(&self, package: &str) -> BoxFuture<'static, Result<Option<Package>>> {
        let package = package.to_string();
        async move {
            // SECURITY: Validate package name
            crate::core::security::validate_package_name(&package)?;

            // Try fast path first
            if let Ok(Some(pkg)) = super::debian_db::get_info_fast(&package) {
                return Ok(Some(pkg));
            }

            let info = tokio::task::spawn_blocking(move || get_sync_pkg_info(&package))
                .await
                .context("APT info task failed")??;
            Ok(info.map(|info| Package {
                name: info.name,
                version: info.version,
                description: info.description,
                source: PackageSource::Official,
                installed: info.installed,
            }))
        }
        .boxed()
    }

    fn list_installed(&self) -> BoxFuture<'static, Result<Vec<Package>>> {
        async move {
            // Try fast path first
            if let Ok(installed) = super::debian_db::list_installed_fast() {
                return Ok(installed
                    .into_iter()
                    .map(|p| Package {
                        name: p.name,
                        version: p.version,
                        description: p.description,
                        source: PackageSource::Official,
                        installed: true,
                    })
                    .collect());
            }
            Ok(local_to_packages(list_installed_fast()?))
        }
        .boxed()
    }

    fn get_status(&self, fast: bool) -> BoxFuture<'static, Result<(usize, usize, usize, usize)>> {
        async move {
            // Try fast path for counts
            if let Ok((total, explicit, _, _)) = super::debian_db::get_counts_fast() {
                if fast {
                    return Ok((total, explicit, 0, 0));
                }

                // If not fast, we still want accuracy for orphans/updates
                if let Ok((_, _, orphans, updates)) = get_system_status() {
                    return Ok((total, explicit, orphans, updates));
                }
                return Ok((total, explicit, 0, 0));
            }

            get_system_status()
        }
        .boxed()
    }

    fn list_explicit(&self) -> BoxFuture<'static, Result<Vec<String>>> {
        async move {
            if let Ok(explicit) = super::debian_db::list_explicit_fast() {
                return Ok(explicit);
            }
            list_explicit()
        }
        .boxed()
    }

    fn list_updates(
        &self,
    ) -> BoxFuture<'static, Result<Vec<crate::package_managers::types::UpdateInfo>>> {
        async move {
            let updates = list_updates()?;
            Ok(updates
                .into_iter()
                .map(
                    |(name, old_ver, new_ver)| crate::package_managers::types::UpdateInfo {
                        name,
                        old_version: old_ver,
                        new_version: new_ver,
                        repo: "apt".to_string(),
                    },
                )
                .collect())
        }
        .boxed()
    }

    fn is_installed(&self, package: &str) -> BoxFuture<'static, bool> {
        let package = package.to_string();
        async move {
            if let Ok(installed) = super::debian_db::list_installed_fast() {
                return installed.iter().any(|p| p.name == package);
            }
            if let Ok(installed) = list_installed_fast() {
                installed.iter().any(|p| p.name == package)
            } else {
                false
            }
        }
        .boxed()
    }
}
pub fn search_sync(query: &str) -> Result<Vec<SyncPackage>> {
    let cache = open_cache(&[])?;
    let mut results = Vec::new();
    let query_lower = query.to_lowercase();

    for pkg in cache.packages(&PackageSort::default()) {
        let name = pkg.name();

        // Match name first (fast)
        let mut matched = name.contains(&query_lower);

        #[allow(clippy::collapsible_if)]
        // If name doesn't match, check summary (slower as it might load more data)
        let mut summary = None;
        if !matched {
            if let Some(s) = pkg.candidate().and_then(|c| c.summary()) {
                if s.to_lowercase().contains(&query_lower) {
                    matched = true;
                    summary = Some(s);
                }
            }
        }

        #[allow(clippy::redundant_closure_for_method_calls)]
        #[allow(clippy::unnecessary_map_or)]

        if matched {
            let candidate = pkg.candidate();
            let version: String = if let Some(ref c) = candidate {
                c.version().to_string()
            } else if let Some(ref i) = pkg.installed() {
                i.version().to_string()
            } else {
                "unknown".to_string()
            };

            let download_size: i64 = pkg
                .candidate()
                .map_or(0i64, |v| i64::try_from(v.size()).unwrap_or(i64::MAX));

            results.push(SyncPackage {
                name: name.to_string(),
                version,
                description: summary
                    .or_else(|| candidate.and_then(|c| c.summary()))
                    .unwrap_or_default(),
                repo: "apt".to_string(),
                download_size,
                installed: pkg.is_installed(),
            });
        }

        if results.len() >= 100 {
            break;
        }
    }

    Ok(results)
}

pub fn get_sync_pkg_info(name: &str) -> Result<Option<PackageInfo>> {
    let cache = open_cache(&[])?;
    if let Some(pkg) = cache.get(name) {
        let version_to_use = pkg.candidate().or_else(|| pkg.installed());
        if let Some(version) = version_to_use {
            return Ok(Some(PackageInfo {
                name: pkg.name().to_string(),
                version: version.version().to_string(),
                description: version.summary().unwrap_or_default(),
                url: None,
                size: version.size(),
                install_size: Some(i64::try_from(version.installed_size()).unwrap_or(i64::MAX)),
                download_size: Some(version.size()),
                repo: "apt".to_string(),
                depends: collect_depends(&version),
                licenses: Vec::new(),
                installed: pkg.is_installed(),
            }));
        }
    }
    Ok(None)
}

pub fn list_installed_fast() -> Result<Vec<LocalPackage>> {
    let cache = open_cache(&[])?;
    let mut packages = Vec::new();

    for pkg in cache.packages(&PackageSort::default()) {
        if pkg.is_installed() {
            packages.push(map_local_package(&pkg));
        }
    }

    Ok(packages)
}

pub fn list_explicit() -> Result<Vec<String>> {
    let cache = open_cache(&[])?;
    let mut explicit = Vec::new();

    for pkg in cache.packages(&PackageSort::default()) {
        if pkg.is_installed() && !pkg.is_auto_installed() {
            explicit.push(pkg.name().to_string());
        }
    }

    explicit.sort();
    Ok(explicit)
}

/// List all available package names
pub fn list_all_package_names() -> Result<Vec<String>> {
    let cache = open_cache(&[])?;
    let mut names = Vec::new();

    for pkg in cache.packages(&PackageSort::default()) {
        names.push(pkg.name().to_string());
    }

    names.sort();
    names.dedup();
    Ok(names)
}

/// List orphaned packages
pub fn list_orphans() -> Result<Vec<String>> {
    let cache = open_cache(&[])?;
    let mut orphans = Vec::new();
    for pkg in cache.packages(&PackageSort::default()) {
        if pkg.is_auto_removable() {
            orphans.push(pkg.name().to_string());
        }
    }
    Ok(orphans)
}

pub fn remove_orphans() -> Result<()> {
    let orphans = list_orphans()?;
    if orphans.is_empty() {
        return Ok(());
    }
    remove_blocking(&orphans)
}

/// List packages with available updates
pub fn list_updates() -> Result<Vec<(String, String, String)>> {
    let cache = open_cache(&[])?;
    let mut updates = Vec::new();

    for pkg in cache.packages(&PackageSort::default()) {
        if pkg.is_upgradable() {
            let name = pkg.name().to_string();
            let old_version = pkg
                .installed()
                .map(|v| v.version().to_string())
                .unwrap_or_default();
            let new_version = pkg
                .candidate()
                .map(|v| v.version().to_string())
                .unwrap_or_default();
            updates.push((name, old_version, new_version));
        }
    }

    Ok(updates)
}

pub fn get_system_status() -> Result<(usize, usize, usize, usize)> {
    let cache = open_cache(&[])?;
    let mut installed_count = 0;
    let mut explicit_count = 0;
    let mut orphans_count = 0;
    let mut updates_count = 0;

    for pkg in cache.packages(&PackageSort::default()) {
        if pkg.is_installed() {
            installed_count += 1;
            if !pkg.is_auto_installed() {
                explicit_count += 1;
            }
        }

        if pkg.is_upgradable() {
            updates_count += 1;
        }

        if pkg.is_auto_removable() {
            orphans_count += 1;
        }
    }

    Ok((
        installed_count,
        explicit_count,
        orphans_count,
        updates_count,
    ))
}

fn open_cache(local_files: &[String]) -> Result<Cache> {
    let files: Vec<&str> = local_files.iter().map(String::as_str).collect();
    Cache::new(&files).map_err(|e| anyhow::anyhow!(format!("APT cache error: {e:?}")))
}

fn install_blocking(packages: &[String]) -> Result<()> {
    let (local_files, names): (Vec<String>, Vec<String>) =
        packages.iter().cloned().partition(|pkg| {
            let path = std::path::Path::new(pkg);
            path.extension().is_some_and(|ext| {
                ext.eq_ignore_ascii_case("deb") || ext.eq_ignore_ascii_case("ddeb")
            })
        });

    let cache = open_cache(&local_files)?;
    for pkg_name in &names {
        let pkg = cache
            .get(pkg_name)
            .with_context(|| format!("Package not found: {pkg_name}"))?;
        pkg.mark_install(true, true);
        pkg.protect();
    }

    cache
        .resolve(true)
        .map_err(|e| anyhow::anyhow!(format!("APT resolve error: {e:?}")))?;

    let mut acquire_progress = AcquireProgress::apt();
    let mut install_progress = InstallProgress::apt();
    cache
        .commit(&mut acquire_progress, &mut install_progress)
        .map_err(|e| anyhow::anyhow!(format!("APT commit error: {e:?}")))?;

    Ok(())
}

fn remove_blocking(packages: &[String]) -> Result<()> {
    let cache = open_cache(&[])?;
    for pkg_name in packages {
        let pkg = cache
            .get(pkg_name)
            .with_context(|| format!("Package not found: {pkg_name}"))?;
        pkg.mark_delete(false);
    }

    cache
        .resolve(true)
        .map_err(|e| anyhow::anyhow!(format!("APT resolve error: {e:?}")))?;

    let mut acquire_progress = AcquireProgress::apt();
    let mut install_progress = InstallProgress::apt();
    cache
        .commit(&mut acquire_progress, &mut install_progress)
        .map_err(|e| anyhow::anyhow!(format!("APT commit error: {e:?}")))?;

    Ok(())
}

fn update_blocking() -> Result<()> {
    sync_databases_blocking()?;
    let cache = open_cache(&[])?;
    cache
        .upgrade(Upgrade::FullUpgrade)
        .map_err(|e| anyhow::anyhow!(format!("APT upgrade error: {e:?}")))?;
    cache
        .resolve(true)
        .map_err(|e| anyhow::anyhow!(format!("APT resolve error: {e:?}")))?;

    let mut acquire_progress = AcquireProgress::apt();
    let mut install_progress = InstallProgress::apt();
    cache
        .commit(&mut acquire_progress, &mut install_progress)
        .map_err(|e| anyhow::anyhow!(format!("APT commit error: {e:?}")))?;

    Ok(())
}

fn sync_databases_blocking() -> Result<()> {
    let cache = open_cache(&[])?;
    let mut progress = AcquireProgress::apt();
    cache
        .update(&mut progress)
        .map_err(|e| anyhow::anyhow!(format!("APT update error: {e:?}")))?;
    Ok(())
}

#[allow(clippy::cast_possible_wrap)]
fn map_local_package(pkg: &rust_apt::Package<'_>) -> LocalPackage {
    let version = pkg
        .installed()
        .or_else(|| pkg.candidate())
        .map_or_else(|| "unknown".to_string(), |ver| ver.version().to_string());
    let summary = pkg
        .installed()
        .and_then(|ver| ver.summary())
        .or_else(|| pkg.candidate().and_then(|v| v.summary()))
        .unwrap_or_default();
    let reason = if pkg.is_auto_installed() {
        "dependency"
    } else {
        "explicit"
    };
    LocalPackage {
        name: pkg.name().to_string(),
        version,
        description: summary,
        install_size: pkg.installed().map_or(0, |v| v.installed_size() as i64),
        reason,
    }
}

fn collect_depends(version: &rust_apt::Version<'_>) -> Vec<String> {
    let mut depends = Vec::new();
    if let Some(deps) = version.dependencies() {
        for dep in deps {
            if dep.is_or() {
                for base in dep.iter() {
                    depends.push(base.name().to_string());
                }
            } else {
                let base = dep.first();
                depends.push(base.name().to_string());
            }
        }
    }
    depends
}

fn sync_to_packages(sync_pkgs: Vec<SyncPackage>) -> Vec<Package> {
    sync_pkgs
        .into_iter()
        .map(|pkg| Package {
            name: pkg.name,
            version: pkg.version,
            description: pkg.description,
            source: PackageSource::Official,
            installed: pkg.installed,
        })
        .collect()
}

fn local_to_packages(local_pkgs: Vec<LocalPackage>) -> Vec<Package> {
    local_pkgs
        .into_iter()
        .map(|pkg| Package {
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            description: pkg.description.clone(),
            source: PackageSource::Official,
            installed: true,
        })
        .collect()
}
