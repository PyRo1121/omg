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

/// Get a cached ALPM handle or create a new one for this thread
#[allow(clippy::expect_used)]
pub fn with_handle<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&Alpm) -> Result<R>,
{
    ALPM_HANDLE.with(|cell| {
        let mut maybe_handle = cell.borrow_mut();
        if maybe_handle.is_none() {
            let root = paths::pacman_root().to_string_lossy().into_owned();
            let db_path = paths::pacman_db_dir().to_string_lossy().into_owned();
            let alpm = Alpm::new(root, db_path).context("Failed to initialize ALPM handle")?;

            // Register sync databases
            for db_name in &["core", "extra", "multilib"] {
                let _ = alpm.register_syncdb(*db_name, SigLevel::USE_DEFAULT);
            }
            *maybe_handle = Some(alpm);
        }

        f(maybe_handle.as_ref().expect("ALPM handle initialized above"))
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
            let root = paths::pacman_root().to_string_lossy().into_owned();
            let db_path = paths::pacman_db_dir().to_string_lossy().into_owned();
            let alpm = Alpm::new(root, db_path).context("Failed to initialize ALPM handle")?;

            // Register sync databases
            for db_name in &["core", "extra", "multilib"] {
                let _ = alpm.register_syncdb(*db_name, SigLevel::USE_DEFAULT);
            }
            *maybe_handle = Some(alpm);
        }

        f(maybe_handle.as_mut().expect("ALPM handle initialized above"))
    })
}

/// Search local database (installed packages) - INSTANT
pub fn search_local(query: &str) -> Result<Vec<LocalPackage>> {
    if paths::test_mode() {
        let query_lower = query.to_lowercase();
        // Use cached search for test mode - much faster than re-parsing
        let packages = pacman_db::search_local_cached(&query_lower)?;
        let results = packages
            .into_iter()
            .map(|pkg| LocalPackage {
                name: pkg.name,
                version: pkg.version,
                description: pkg.desc,
                install_size: 0,
                reason: if pkg.explicit {
                    "explicit"
                } else {
                    "dependency"
                },
            })
            .collect();
        return Ok(results);
    }

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
    if paths::test_mode() {
        // search_sync_fast already uses cache and filters
        let packages = pacman_db::search_sync_fast(query)?;
        let results = packages
            .into_iter()
            .map(|pkg| {
                let installed = pacman_db::is_installed_cached(&pkg.name);
                SyncPackage {
                    name: pkg.name,
                    version: pkg.version,
                    description: pkg.desc,
                    repo: pkg.repo,
                    download_size: i64::try_from(pkg.csize).unwrap_or(i64::MAX),
                    installed,
                }
            })
            .collect();
        return Ok(results);
    }

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
    if paths::test_mode() {
        // Use cached lookup instead of re-parsing
        if let Some(pkg) = pacman_db::get_local_package(name)? {
            return Ok(Some(PackageInfo {
                name: pkg.name,
                version: pkg.version.clone(),
                description: pkg.desc,
                url: None,
                size: 0,
                install_size: None,
                download_size: None,
                repo: "local".to_string(),
                depends: Vec::new(),
                licenses: Vec::new(),
                installed: true,
            }));
        }

        if let Some(pkg) = pacman_db::get_sync_package(name)? {
            return Ok(Some(PackageInfo {
                name: pkg.name,
                version: pkg.version.clone(),
                description: pkg.desc,
                url: Some(pkg.url),
                size: pkg.isize,
                install_size: Some(i64::try_from(pkg.isize).unwrap_or(i64::MAX)),
                download_size: Some(pkg.csize),
                repo: pkg.repo,
                depends: pkg.depends,
                licenses: Vec::new(),
                installed: false,
            }));
        }

        return Ok(None);
    }

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
    if paths::test_mode() {
        // Use cached list instead of re-parsing
        let packages = pacman_db::list_local_cached()?;
        let results = packages
            .into_iter()
            .map(|pkg| LocalPackage {
                name: pkg.name,
                version: pkg.version,
                description: pkg.desc,
                install_size: 0,
                reason: if pkg.explicit {
                    "explicit"
                } else {
                    "dependency"
                },
            })
            .collect();
        return Ok(results);
    }

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
    if paths::test_mode() {
        // Use cached list instead of re-parsing
        let packages = pacman_db::list_local_cached()?;
        let results = packages
            .into_iter()
            .filter(|pkg| pkg.explicit)
            .map(|pkg| pkg.name)
            .collect();
        return Ok(results);
    }

    with_handle(|handle| {
        let results = handle
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
    if paths::test_mode() {
        return Ok(Vec::new());
    }

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
    if paths::test_mode() {
        return Ok(pacman_db::is_installed_cached(name));
    }

    with_handle(|handle| Ok(handle.localdb().pkg(name).is_ok()))
}

/// Get counts - INSTANT
pub fn get_counts() -> Result<(usize, usize, usize)> {
    if paths::test_mode() {
        let (total, explicit, deps) = pacman_db::get_counts_fast()?;
        return Ok((total, explicit, deps));
    }

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
    if paths::test_mode() {
        return pacman_db::list_all_names_cached();
    }

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
