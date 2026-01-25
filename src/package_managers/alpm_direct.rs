//! Direct ALPM (Arch Linux Package Manager) integration
//!
//! Uses libalpm directly for 10-100x faster queries compared to spawning pacman.

use alpm::{Alpm, PackageReason, SigLevel};
use anyhow::{Context, Result};

use std::cell::RefCell;

use crate::core::paths;
use crate::package_managers::pacman_db;
use crate::package_managers::types::{LocalPackage, PackageInfo, SyncPackage};

thread_local! {
    static ALPM_HANDLE: RefCell<Option<Alpm>> = const { RefCell::new(None) };
}

// Create a new Alpm handle.
fn create_alpm_handle() -> Result<Alpm> {
    let root = paths::pacman_root().to_string_lossy().into_owned();
    let db_path = paths::pacman_db_dir().to_string_lossy().into_owned();
    let alpm = Alpm::new(root, db_path).context("Failed to initialize ALPM handle")?;

    // Register sync databases
    for db_name in &["core", "extra", "multilib"] {
        let _ = alpm.register_syncdb(*db_name, SigLevel::USE_DEFAULT);
    }
    Ok(alpm)
}

/// Execute a function with a provided ALPM handle.
/// This is pub(crate) for testing purposes, allowing injection of a mock handle.
pub(crate) fn with_alpm_handle<F, R>(alpm: &Alpm, f: F) -> Result<R>
where
    F: FnOnce(&Alpm) -> Result<R>,
{
    f(alpm)
}

/// Get a cached ALPM handle or create a new one for this thread
#[allow(clippy::expect_used)]
pub fn with_handle<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&Alpm) -> Result<R>,
{
    ALPM_HANDLE.with(|cell| {
        let mut maybe_handle = cell.borrow_mut();
        if maybe_handle.is_none() {
            *maybe_handle = Some(create_alpm_handle()?);
        }

        with_alpm_handle(
            maybe_handle
                .as_ref()
                .expect("ALPM handle initialized above"),
            f,
        )
    })
}

/// Get a mutable cached ALPM handle
#[allow(clippy::expect_used)]
pub fn with_handle_mut<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut Alpm) -> Result<R>,
{
    ALPM_HANDLE.with(|cell| {
        let mut maybe_handle = cell.borrow_mut();
        if maybe_handle.is_none() {
            *maybe_handle = Some(create_alpm_handle()?);
        }

        f(maybe_handle
            .as_mut()
            .expect("ALPM handle initialized above"))
    })
}

/// Search local database (installed packages) - INSTANT
pub fn search_local(query: &str) -> Result<Vec<LocalPackage>> {
    with_handle(|handle| {
        let localdb = handle.localdb();
        let query_lower = query.to_lowercase();

        let results = localdb
            .pkgs()
            .iter()
            .filter(|pkg| {
                pkg.name().contains(&query_lower)
                    || pkg
                        .desc()
                        .is_some_and(|d| d.to_lowercase().contains(&query_lower))
            })
            .map(|pkg| LocalPackage {
                name: pkg.name().to_string(),
                version: super::types::parse_version_or_zero(pkg.version()),
                description: pkg.desc().unwrap_or("").to_string(),
                install_size: pkg.isize(),
                reason: match pkg.reason() {
                    PackageReason::Explicit => "explicit",
                    PackageReason::Depend => "dependency",
                },
            })
            .collect();

        Ok(results)
    })
}

/// Search sync databases (available packages) - FAST (<10ms)
pub fn search_sync(query: &str) -> Result<Vec<SyncPackage>> {
    with_handle(|handle| {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for db in handle.syncdbs() {
            for pkg in db.pkgs() {
                if pkg.name().contains(&query_lower)
                    || pkg
                        .desc()
                        .is_some_and(|d| d.to_lowercase().contains(&query_lower))
                {
                    let installed = handle.localdb().pkg(pkg.name()).is_ok();

                    results.push(SyncPackage {
                        name: pkg.name().to_string(),
                        version: super::types::parse_version_or_zero(pkg.version()),
                        description: pkg.desc().unwrap_or("").to_string(),
                        repo: db.name().to_string(),
                        download_size: pkg.download_size(),
                        installed,
                    });
                }
            }
        }

        Ok(results)
    })
}

/// Get package info - INSTANT (<1ms)
pub fn get_package_info(name: &str) -> Result<Option<PackageInfo>> {
    with_handle(|handle| {
        // Try local first
        if let Ok(pkg) = handle.localdb().pkg(name) {
            return Ok(Some(PackageInfo {
                name: pkg.name().to_string(),
                version: super::types::parse_version_or_zero(pkg.version()),
                description: pkg.desc().unwrap_or("").to_string(),
                url: pkg.url().map(std::string::ToString::to_string),
                size: pkg.isize() as u64,
                install_size: Some(pkg.isize()),
                download_size: None,
                repo: "local".to_string(),
                depends: pkg.depends().iter().map(|d| d.name().to_string()).collect(),
                licenses: pkg
                    .licenses()
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect(),
                installed: true,
            }));
        }

        // Try sync databases
        for db in handle.syncdbs() {
            if let Ok(pkg) = db.pkg(name) {
                return Ok(Some(PackageInfo {
                    name: pkg.name().to_string(),
                    version: super::types::parse_version_or_zero(pkg.version()),
                    description: pkg.desc().unwrap_or("").to_string(),
                    url: pkg.url().map(std::string::ToString::to_string),
                    size: pkg.isize() as u64,
                    install_size: Some(pkg.isize()),
                    download_size: Some(pkg.download_size() as u64),
                    repo: db.name().to_string(),
                    depends: pkg.depends().iter().map(|d| d.name().to_string()).collect(),
                    licenses: pkg
                        .licenses()
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect(),
                    installed: false,
                }));
            }
        }

        Ok(None)
    })
}

/// List all installed packages - INSTANT
pub fn list_installed_fast() -> Result<Vec<LocalPackage>> {
    with_handle(|handle| {
        let localdb = handle.localdb();

        let results = localdb
            .pkgs()
            .iter()
            .map(|pkg| LocalPackage {
                name: pkg.name().to_string(),
                version: super::types::parse_version_or_zero(pkg.version()),
                description: pkg.desc().unwrap_or("").to_string(),
                install_size: pkg.isize(),
                reason: match pkg.reason() {
                    PackageReason::Explicit => "explicit",
                    PackageReason::Depend => "dependency",
                },
            })
            .collect();

        Ok(results)
    })
}

/// List explicitly installed packages - INSTANT
pub fn list_explicit_fast() -> Result<Vec<String>> {
    // Prefer cached local DB parsing for speed (works in normal mode too)
    if let Ok(packages) = pacman_db::list_local_cached() {
        let results: Vec<String> = packages
            .into_iter()
            .filter(|pkg| pkg.explicit)
            .map(|pkg| pkg.name)
            .collect();
        return Ok(results);
    }

    with_handle(|handle| {
        let results: Vec<String> = handle
            .localdb()
            .pkgs()
            .iter()
            .filter(|pkg| pkg.reason() == PackageReason::Explicit)
            .map(|pkg| pkg.name().to_string())
            .collect();

        Ok(results)
    })
}

/// List orphan packages - INSTANT
pub fn list_orphans_fast() -> Result<Vec<String>> {
    with_handle(|handle| {
        let results = handle
            .localdb()
            .pkgs()
            .iter()
            .filter(|pkg| pkg.reason() == PackageReason::Depend && pkg.required_by().is_empty())
            .map(|pkg| pkg.name().to_string())
            .collect();

        Ok(results)
    })
}

/// Check if package is installed - INSTANT
pub fn is_installed_fast(name: &str) -> Result<bool> {
    with_handle(|handle| Ok(handle.localdb().pkg(name).is_ok()))
}

/// Get counts - INSTANT
pub fn get_counts() -> Result<(usize, usize, usize)> {
    with_handle(|handle| {
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
    })
}

/// List all known package names (local + sync) for completion - FAST
pub fn list_all_package_names() -> Result<Vec<String>> {
    with_handle(|handle| {
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
    })
}
