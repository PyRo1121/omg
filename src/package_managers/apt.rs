//! Debian/Ubuntu package manager backend
//!
//! This module provides full Debian/Ubuntu support using `rust-apt` FFI bindings.
//! It requires `libapt-pkg-dev` to be installed on the system.

use std::future::Future;
use std::pin::Pin;

use anyhow::{Context, Result};

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

    fn search(
        &self,
        query: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        let query = query.to_string();
        Box::pin(async move {
            // Try fast path first
            let fast_results = super::debian_db::search_fast(&query);
            if let Ok(results) = fast_results
                && !results.is_empty()
            {
                return Ok(results);
            }

            let results = tokio::task::spawn_blocking(move || search_sync(&query))
                .await
                .context("APT search task failed")??;

            Ok(sync_to_packages(results))
        })
    }

    fn install(
        &self,
        packages: &[String],
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
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
        })
    }

    fn remove(&self, packages: &[String]) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
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
        })
    }

    fn update(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            if !is_root() {
                crate::core::privilege::run_self_sudo(&["update"]).await?;
                return Ok(());
            }

            tokio::task::spawn_blocking(update_blocking)
                .await
                .context("APT update task failed")??;
            Ok(())
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move { AptPackageManager::new().sync_databases().await })
    }

    fn info(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Package>>> + Send + '_>> {
        let package = package.to_string();
        Box::pin(async move {
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
        })
    }

    fn list_installed(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        Box::pin(async move {
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
        })
    }

    fn get_status(
        &self,
        fast: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(usize, usize, usize, usize)>> + Send + '_>> {
        Box::pin(async move {
            super::debian_db::resolve_status_counts(
                fast,
                super::debian_db::get_counts_fast(),
                get_system_status,
            )
        })
    }

    fn list_explicit(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            if let Ok(explicit) = super::debian_db::list_explicit_fast() {
                return Ok(explicit);
            }
            list_explicit()
        })
    }

    fn list_updates(
        &self,
    ) -> Pin<
        Box<
            dyn Future<Output = Result<Vec<crate::package_managers::types::UpdateInfo>>>
                + Send
                + '_,
        >,
    > {
        Box::pin(async move {
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
        })
    }

    fn is_installed(&self, package: &str) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        let result = super::debian_db::is_installed_fast(package);
        Box::pin(async move { result })
    }
}
pub fn search_sync(query: &str) -> Result<Vec<SyncPackage>> {
    let cache = open_cache(&[])?;
    let mut results = Vec::with_capacity(64);
    let query_lower = query.to_lowercase();

    for pkg in cache.packages(&PackageSort::default()) {
        let name = pkg.name();
        let matched = name.contains(&query_lower)
            || pkg
                .candidate()
                .and_then(|c| c.summary())
                .is_some_and(|s| s.to_lowercase().contains(&query_lower));

        if matched {
            let candidate = pkg.candidate();
            let version = candidate
                .as_ref()
                .map(|c| c.version().to_string())
                .or_else(|| pkg.installed().map(|i| i.version().to_string()))
                .unwrap_or_else(|| "unknown".to_string());

            let download_size = candidate
                .as_ref()
                .map_or(0, |v| i64::try_from(v.size()).unwrap_or(i64::MAX));

            let description = candidate.and_then(|c| c.summary()).unwrap_or_default();

            results.push(SyncPackage {
                name: name.to_string(),
                version,
                description,
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
    let Some(pkg) = cache.get(name) else {
        return Ok(None);
    };

    let Some(version) = pkg.candidate().or_else(|| pkg.installed()) else {
        return Ok(None);
    };

    Ok(Some(PackageInfo {
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
    }))
}

pub fn list_installed_fast() -> Result<Vec<LocalPackage>> {
    let cache = open_cache(&[])?;
    let mut packages = Vec::with_capacity(512);

    for pkg in cache.packages(&PackageSort::default()) {
        if pkg.is_installed() {
            packages.push(map_local_package(&pkg));
        }
    }

    Ok(packages)
}

pub fn list_explicit() -> Result<Vec<String>> {
    let cache = open_cache(&[])?;
    let mut explicit = Vec::with_capacity(256);

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
    let mut names = Vec::with_capacity(4096);

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
    let mut orphans = Vec::with_capacity(32);
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
    let mut updates = Vec::with_capacity(64);

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

#[expect(clippy::cast_possible_wrap)] // Package sizes fit in i64; clamped to i64::MAX via unwrap_or
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
            name: pkg.name,
            version: pkg.version,
            description: pkg.description,
            source: PackageSource::Official,
            installed: true,
        })
        .collect()
}
