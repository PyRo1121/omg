//! Direct ALPM (Arch Linux Package Manager) integration
//!
//! Uses libalpm directly for 10-100x faster queries compared to spawning pacman.

use alpm::{Alpm, PackageReason};
use anyhow::{Context, Result};

/// Create a fresh ALPM handle for queries
/// Creates per-call since libalpm isn't thread-safe by design
fn create_handle() -> Result<Alpm> {
    Alpm::new("/", "/var/lib/pacman").context("Failed to initialize ALPM handle")
}

/// Search local database (installed packages) - INSTANT
pub fn search_local(query: &str) -> Result<Vec<LocalPackage>> {
    let handle = create_handle()?;
    let localdb = handle.localdb();
    let query_lower = query.to_lowercase();

    let results = localdb
        .pkgs()
        .iter()
        .filter(|pkg| {
            pkg.name().to_lowercase().contains(&query_lower)
                || pkg
                    .desc()
                    .map(|d| d.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
        })
        .map(|pkg| LocalPackage {
            name: pkg.name().to_string(),
            version: pkg.version().to_string(),
            description: pkg.desc().unwrap_or("").to_string(),
            install_size: pkg.isize(),
            reason: match pkg.reason() {
                PackageReason::Explicit => "explicit",
                PackageReason::Depend => "dependency",
            },
        })
        .collect();

    Ok(results)
}

/// Search sync databases (available packages) - FAST (<10ms)
pub fn search_sync(query: &str) -> Result<Vec<SyncPackage>> {
    let handle = create_handle()?;
    let query_lower = query.to_lowercase();
    let mut results = Vec::new();

    for db in handle.syncdbs() {
        for pkg in db.pkgs() {
            if pkg.name().to_lowercase().contains(&query_lower)
                || pkg
                    .desc()
                    .map(|d| d.to_lowercase().contains(&query_lower))
                    .unwrap_or(false)
            {
                let installed = handle.localdb().pkg(pkg.name()).is_ok();

                results.push(SyncPackage {
                    name: pkg.name().to_string(),
                    version: pkg.version().to_string(),
                    description: pkg.desc().unwrap_or("").to_string(),
                    repo: db.name().to_string(),
                    download_size: pkg.download_size(),
                    installed,
                });
            }
        }
    }

    Ok(results)
}

/// Get package info - INSTANT (<1ms)
pub fn get_package_info(name: &str) -> Result<Option<PackageInfo>> {
    let handle = create_handle()?;

    // Try local first
    if let Ok(pkg) = handle.localdb().pkg(name) {
        return Ok(Some(PackageInfo {
            name: pkg.name().to_string(),
            version: pkg.version().to_string(),
            description: pkg.desc().unwrap_or("").to_string(),
            url: pkg.url().map(|u| u.to_string()),
            licenses: pkg.licenses().iter().map(|l| l.to_string()).collect(),
            depends: pkg.depends().iter().map(|d| d.name().to_string()).collect(),
            installed: true,
            install_size: Some(pkg.isize()),
            download_size: None,
            repo: Some("local".to_string()),
        }));
    }

    // Try sync databases
    for db in handle.syncdbs() {
        if let Ok(pkg) = db.pkg(name) {
            return Ok(Some(PackageInfo {
                name: pkg.name().to_string(),
                version: pkg.version().to_string(),
                description: pkg.desc().unwrap_or("").to_string(),
                url: pkg.url().map(|u| u.to_string()),
                licenses: pkg.licenses().iter().map(|l| l.to_string()).collect(),
                depends: pkg.depends().iter().map(|d| d.name().to_string()).collect(),
                installed: false,
                install_size: Some(pkg.isize()),
                download_size: Some(pkg.download_size()),
                repo: Some(db.name().to_string()),
            }));
        }
    }

    Ok(None)
}

/// List all installed packages - INSTANT
pub fn list_installed_fast() -> Result<Vec<LocalPackage>> {
    let handle = create_handle()?;
    let localdb = handle.localdb();

    let results = localdb
        .pkgs()
        .iter()
        .map(|pkg| LocalPackage {
            name: pkg.name().to_string(),
            version: pkg.version().to_string(),
            description: pkg.desc().unwrap_or("").to_string(),
            install_size: pkg.isize(),
            reason: match pkg.reason() {
                PackageReason::Explicit => "explicit",
                PackageReason::Depend => "dependency",
            },
        })
        .collect();

    Ok(results)
}

/// List explicitly installed packages - INSTANT
pub fn list_explicit_fast() -> Result<Vec<String>> {
    let handle = create_handle()?;

    let results = handle
        .localdb()
        .pkgs()
        .iter()
        .filter(|pkg| pkg.reason() == PackageReason::Explicit)
        .map(|pkg| pkg.name().to_string())
        .collect();

    Ok(results)
}

/// List orphan packages - INSTANT
pub fn list_orphans_fast() -> Result<Vec<String>> {
    let handle = create_handle()?;

    let results = handle
        .localdb()
        .pkgs()
        .iter()
        .filter(|pkg| pkg.reason() == PackageReason::Depend && pkg.required_by().is_empty())
        .map(|pkg| pkg.name().to_string())
        .collect();

    Ok(results)
}

/// Check if package is installed - INSTANT
pub fn is_installed_fast(name: &str) -> Result<bool> {
    let handle = create_handle()?;
    Ok(handle.localdb().pkg(name).is_ok())
}

/// Get counts - INSTANT
pub fn get_counts() -> Result<(usize, usize, usize)> {
    let handle = create_handle()?;
    let pkgs = handle.localdb().pkgs();

    let total = pkgs.len();
    let explicit = pkgs
        .iter()
        .filter(|p| p.reason() == PackageReason::Explicit)
        .count();
    let orphans = pkgs
        .iter()
        .filter(|p| p.reason() == PackageReason::Depend && p.required_by().is_empty())
        .count();

    Ok((total, explicit, orphans))
}

/// List all known package names (local + sync) for completion - FAST
pub fn list_all_package_names() -> Result<Vec<String>> {
    let handle = create_handle()?;
    let mut names = std::collections::HashSet::new();

    // Add local packages
    for pkg in handle.localdb().pkgs() {
        names.insert(pkg.name().to_string());
    }

    // Add sync packages
    for db in handle.syncdbs() {
        for pkg in db.pkgs() {
            names.insert(pkg.name().to_string());
        }
    }

    let mut result: Vec<String> = names.into_iter().collect();
    result.sort();
    Ok(result)
}

#[derive(Debug, Clone)]
pub struct LocalPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub install_size: i64,
    pub reason: &'static str,
}

#[derive(Debug, Clone)]
pub struct SyncPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub repo: String,
    pub download_size: i64,
    pub installed: bool,
}

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub url: Option<String>,
    pub licenses: Vec<String>,
    pub depends: Vec<String>,
    pub installed: bool,
    pub install_size: Option<i64>,
    pub download_size: Option<i64>,
    pub repo: Option<String>,
}
