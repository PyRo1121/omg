//! Debian/Ubuntu package manager backend (APT via rust-apt)

use anyhow::{Context, Result};
use rust_apt::Cache;
use rust_apt::cache::{PackageSort, Upgrade};
use rust_apt::progress::{AcquireProgress, InstallProgress};

use crate::core::{Package, PackageSource, is_root};
use crate::package_managers::types::{LocalPackage, SyncPackage, PackageInfo};

#[derive(Debug, Default)]
pub struct AptPackageManager;

impl AptPackageManager {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
    pub async fn sync_databases(&self) -> Result<()> {
        if !is_root() {
            let exe = std::env::current_exe()?;
            let status = tokio::process::Command::new("sudo")
                .arg("--")
                .arg(exe)
                .arg("sync")
                .status()
                .await?;
            if !status.success() {
                anyhow::bail!("Database synchronization failed");
            }
            return Ok(());
        }

        tokio::task::spawn_blocking(move || sync_databases_blocking())
            .await
            .context("APT sync task failed")??;
        Ok(())
    }
}

pub fn list_updates() -> Result<Vec<(String, String, String)>> {
    let cache = open_cache(&[])?;
    let sort = PackageSort::default().upgradable();
    let mut updates = Vec::new();
    for pkg in cache.packages(&sort) {
        let installed = pkg.installed();
        let candidate = pkg.candidate();
        if let (Some(installed), Some(candidate)) = (installed, candidate) {
            updates.push((
                pkg.name().to_string(),
                installed.version().to_string(),
                candidate.version().to_string(),
            ));
        }
    }
    Ok(updates)
}

pub fn get_system_status() -> Result<(usize, usize, usize, usize)> {
    let installed = list_installed_fast()?;
    let explicit = list_explicit()?;
    let orphans = list_orphans()?;
    let updates = list_updates()?;
    Ok((
        installed.len(),
        explicit.len(),
        orphans.len(),
        updates.len(),
    ))
}

impl crate::package_managers::PackageManager for AptPackageManager {
    fn name(&self) -> &'static str {
        "apt"
    }

    fn search(
        &self,
        query: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Package>>> + Send {
        let query = query.to_string();
        async move {
            tokio::task::spawn_blocking(move || search_sync(&query))
                .await
                .context("APT search task failed")?
                .map(sync_to_packages)
        }
    }

    fn install(&self, packages: &[String]) -> impl std::future::Future<Output = Result<()>> + Send {
        let packages = packages.to_vec();
        async move {
            if !is_root() {
                let exe = std::env::current_exe()?;
                let status = tokio::process::Command::new("sudo")
                    .arg("--")
                    .arg(exe)
                    .arg("install")
                    .args(&packages)
                    .status()
                    .await?;
                if !status.success() {
                    anyhow::bail!("Installation failed");
                }
                return Ok(());
            }

            tokio::task::spawn_blocking(move || install_blocking(&packages))
                .await
                .context("APT install task failed")??;
            Ok(())
        }
    }

    fn remove(&self, packages: &[String]) -> impl std::future::Future<Output = Result<()>> + Send {
        let packages = packages.to_vec();
        async move {
            if !is_root() {
                let exe = std::env::current_exe()?;
                let status = tokio::process::Command::new("sudo")
                    .arg("--")
                    .arg(exe)
                    .arg("remove")
                    .args(&packages)
                    .status()
                    .await?;
                if !status.success() {
                    anyhow::bail!("Removal failed");
                }
                return Ok(());
            }

            tokio::task::spawn_blocking(move || remove_blocking(&packages))
                .await
                .context("APT remove task failed")??;
            Ok(())
        }
    }

    fn update(&self) -> impl std::future::Future<Output = Result<()>> + Send {
        async move {
            if !is_root() {
                let exe = std::env::current_exe()?;
                let status = tokio::process::Command::new("sudo")
                    .arg("--")
                    .arg(exe)
                    .arg("update")
                    .status()
                    .await?;
                if !status.success() {
                    anyhow::bail!("Update failed");
                }
                return Ok(());
            }

            tokio::task::spawn_blocking(update_blocking)
                .await
                .context("APT update task failed")??;
            Ok(())
        }
    }

    fn info(
        &self,
        package: &str,
    ) -> impl std::future::Future<Output = Result<Option<Package>>> + Send {
        let package = package.to_string();
        async move {
            let info = tokio::task::spawn_blocking(move || get_sync_pkg_info(&package))
                .await
                .context("APT info task failed")??;
            Ok(info.map(|info| Package {
                name: info.name,
                version: info.version,
                description: info.description,
                source: PackageSource::Official,
                installed: false,
            }))
        }
    }

    fn list_installed(&self) -> impl std::future::Future<Output = Result<Vec<Package>>> + Send {
        async move { list_installed_fast().map(local_to_packages) }
    }
}

pub fn search_sync(query: &str) -> Result<Vec<SyncPackage>> {
    search_sync_blocking(query)
}

pub fn get_sync_pkg_info(name: &str) -> Result<Option<PackageInfo>> {
    let cache = open_cache(&[])?;
    let pkg = match cache.get(name) {
        Some(pkg) => pkg,
        None => return Ok(None),
    };
    let version = pkg.candidate().or_else(|| pkg.installed());
    let Some(version) = version else {
        return Ok(None);
    };

    let description = version.summary().unwrap_or_default();
    let long_description = version.description().unwrap_or_default();
    let url = version.get_record("Homepage").unwrap_or_default();
    let depends = collect_depends(&version);

    Ok(Some(PackageInfo {
        name: pkg.name().to_string(),
        version: version.version().to_string(),
        description: if description.is_empty() {
            long_description
        } else {
            description
        },
        url,
        size: version.installed_size(),
        download_size: version.size(),
        repo: "apt".to_string(),
        depends,
        licenses: Vec::new(),
    }))
}

pub fn list_installed_fast() -> Result<Vec<LocalPackage>> {
    let cache = open_cache(&[])?;
    let sort = PackageSort::default().installed();
    let packages = cache
        .packages(&sort)
        .map(|pkg| map_local_package(&pkg))
        .collect();
    Ok(packages)
}

pub fn list_explicit() -> Result<Vec<String>> {
    let cache = open_cache(&[])?;
    let sort = PackageSort::default().installed();
    let mut packages = Vec::new();
    for pkg in cache.packages(&sort) {
        if !pkg.is_auto_installed() {
            packages.push(pkg.name().to_string());
        }
    }
    Ok(packages)
}

pub fn list_orphans() -> Result<Vec<String>> {
    let cache = open_cache(&[])?;
    let sort = PackageSort::default().auto_removable();
    let packages = cache
        .packages(&sort)
        .map(|pkg| pkg.name().to_string())
        .collect();
    Ok(packages)
}

pub fn list_all_package_names() -> Result<Vec<String>> {
    let cache = open_cache(&[])?;
    let sort = PackageSort::default().names();
    let mut names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for pkg in cache.packages(&sort) {
        names.insert(pkg.name().to_string());
    }
    let mut result: Vec<String> = names.into_iter().collect();
    result.sort();
    Ok(result)
}

pub fn remove_orphans() -> Result<()> {
    let orphans = list_orphans()?;
    if orphans.is_empty() {
        return Ok(());
    }
    remove_blocking(&orphans)
}

fn open_cache(local_files: &[String]) -> Result<Cache> {
    let files: Vec<&str> = local_files.iter().map(String::as_str).collect();
    Cache::new(&files).map_err(|e| anyhow::anyhow!(format!("APT cache error: {e:?}")))
}

fn search_sync_blocking(query: &str) -> Result<Vec<SyncPackage>> {
    let cache = open_cache(&[])?;
    let query_lower = query.to_lowercase();
    let sort = PackageSort::default();
    let mut results = Vec::new();
    for pkg in cache.packages(&sort) {
        let name = pkg.name();
        let mut matches = name.to_lowercase().contains(&query_lower);
        let candidate = pkg.candidate().or_else(|| pkg.installed());
        let summary = candidate
            .as_ref()
            .and_then(|ver| ver.summary())
            .unwrap_or_default();
        if !matches && !summary.is_empty() {
            matches = summary.to_lowercase().contains(&query_lower);
        }
        if !matches {
            continue;
        }
        results.push(map_sync_package_with_summary(&pkg, &summary));
    }
    Ok(results)
}

fn install_blocking(packages: &[String]) -> Result<()> {
    let (local_files, names): (Vec<String>, Vec<String>) = packages
        .iter()
        .cloned()
        .partition(|pkg| pkg.ends_with(".deb") || pkg.ends_with(".ddeb"));

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

fn map_sync_package(pkg: &rust_apt::Package<'_>) -> SyncPackage {
    let summary = pkg
        .candidate()
        .and_then(|ver| ver.summary())
        .unwrap_or_default();
    map_sync_package_with_summary(pkg, &summary)
}

fn map_sync_package_with_summary(pkg: &rust_apt::Package<'_>, summary: &str) -> SyncPackage {
    let version = pkg
        .candidate()
        .or_else(|| pkg.installed())
        .map(|ver| ver.version().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    SyncPackage {
        name: pkg.name().to_string(),
        version,
        description: summary.to_string(),
        repo: "apt".to_string(),
        download_size: 0,
        installed: pkg.is_installed(),
    }
}

fn map_local_package(pkg: &rust_apt::Package<'_>) -> LocalPackage {
    let version = pkg
        .installed()
        .or_else(|| pkg.candidate())
        .map(|ver| ver.version().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let summary = pkg
        .candidate()
        .and_then(|ver| ver.summary())
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
        install_size: 0,
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
            } else if let Some(base) = dep.first() {
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
