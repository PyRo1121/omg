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

fn create_alpm_handle() -> Result<Alpm> {
    let root = paths::pacman_root().to_string_lossy().into_owned();
    let db_path = paths::pacman_db_dir().to_string_lossy().into_owned();

    let alpm = Alpm::new(root.as_str(), db_path.as_str()).with_context(|| {
        format!(
            "Failed to initialize ALPM handle.\n\
             Root: {root}\n\
             DB Path: {db_path}\n\
             Ensure pacman is installed and the database exists."
        )
    })?;

    let repos = crate::core::pacman_conf::get_configured_repos().unwrap_or_else(|e| {
        tracing::warn!("Failed to parse pacman.conf: {e}. Using default repos.");
        vec![
            "core".to_string(),
            "extra".to_string(),
            "multilib".to_string(),
        ]
    });

    let mut registered = 0;
    for db_name in &repos {
        match alpm.register_syncdb(db_name.as_str(), SigLevel::USE_DEFAULT) {
            Ok(_) => {
                registered += 1;
                tracing::trace!("Registered sync database: {db_name}");
            }
            Err(e) => {
                let sync_path = paths::pacman_sync_dir().join(format!("{db_name}.db"));
                if sync_path.exists() {
                    tracing::warn!("Failed to register repo '{db_name}': {e}");
                } else {
                    tracing::debug!(
                        "Repo '{db_name}' not synced yet (missing {sync_path:?}). Run 'omg sync' first."
                    );
                }
            }
        }
    }

    if registered == 0 {
        tracing::warn!(
            "No sync databases registered. Package search may return empty results. Run 'omg sync'."
        );
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
///
/// SAFETY: Uses `catch_unwind` to ensure `RefCell` is properly released even if
/// the closure panics, preventing the thread-local from becoming poisoned.
#[allow(clippy::expect_used)] // ALPM handle initialization; failure indicates system misconfiguration
pub fn with_handle<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&Alpm) -> Result<R>,
{
    ALPM_HANDLE.with(|cell| {
        // Borrow and initialize if needed
        let mut maybe_handle = cell.borrow_mut();
        if maybe_handle.is_none() {
            *maybe_handle = Some(create_alpm_handle()?);
        }

        // Get reference to handle
        let handle_ref = maybe_handle
            .as_ref()
            .expect("ALPM handle initialized above");

        // Execute user function with panic safety
        // SAFETY: We wrap in catch_unwind to ensure RefCell is properly released
        // even if f panics. This prevents the thread-local from becoming poisoned.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_alpm_handle(handle_ref, f)
        }));

        // Drop the borrow before handling panic
        drop(maybe_handle);

        match result {
            Ok(r) => r,
            Err(panic_payload) => {
                // Re-throw the panic after RefCell is released
                std::panic::resume_unwind(panic_payload)
            }
        }
    })
}

/// Get a mutable cached ALPM handle
///
/// SAFETY: Uses `catch_unwind` to ensure `RefCell` is properly released even if
/// the closure panics, preventing the thread-local from becoming poisoned.
#[allow(clippy::expect_used)] // ALPM handle initialization; failure indicates system misconfiguration
pub fn with_handle_mut<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut Alpm) -> Result<R>,
{
    ALPM_HANDLE.with(|cell| {
        // Borrow and initialize if needed
        let mut maybe_handle = cell.borrow_mut();
        if maybe_handle.is_none() {
            *maybe_handle = Some(create_alpm_handle()?);
        }

        // Get mutable reference to handle
        let handle_ref = maybe_handle
            .as_mut()
            .expect("ALPM handle initialized above");

        // Execute user function with panic safety
        // SAFETY: We wrap in catch_unwind to ensure RefCell is properly released
        // even if f panics. This prevents the thread-local from becoming poisoned.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f(handle_ref)
        }));

        // Drop the borrow before handling panic
        drop(maybe_handle);

        match result {
            Ok(r) => r,
            Err(panic_payload) => {
                // Re-throw the panic after RefCell is released
                std::panic::resume_unwind(panic_payload)
            }
        }
    })
}

/// Clear the thread-local ALPM handle cache.
///
/// This should be called when paths change (e.g., in tests that set different
/// `OMG_DATA_DIR` or `OMG_PACMAN_DB_DIR` environment variables) to avoid
/// memory corruption from using handles that reference deleted directories.
///
/// # Safety
/// This function is safe but must be called before any other ALPM operations
/// when environment paths have changed.
pub fn clear_alpm_cache() {
    ALPM_HANDLE.with(|cell| {
        let _ = cell.borrow_mut().take();
    });
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
                size: pkg.isize().try_into().unwrap_or(0),
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
                    size: pkg.isize().try_into().unwrap_or(0),
                    install_size: Some(pkg.isize()),
                    download_size: Some(pkg.download_size().try_into().unwrap_or(0)),
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

        for pkg in handle.localdb().pkgs() {
            names.insert(pkg.name().to_string());
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_local_returns_results() {
        let result = search_local("pacman");
        assert!(result.is_ok());
    }

    #[test]
    fn test_search_local_empty_query() {
        let result = search_local("");
        assert!(result.is_ok());
        let packages = result.unwrap();
        assert!(
            !packages.is_empty(),
            "Empty query should return all packages"
        );
    }

    #[test]
    fn test_search_sync_returns_results() {
        let result = search_sync("linux");
        assert!(result.is_ok());
    }

    #[test]
    fn test_get_package_info_existing() {
        let result = get_package_info("pacman");
        assert!(result.is_ok());
        let info = result.unwrap();
        assert!(info.is_some(), "pacman should be installed");
        let pkg = info.unwrap();
        assert_eq!(pkg.name, "pacman");
    }

    #[test]
    fn test_get_package_info_nonexistent() {
        let result = get_package_info("this-package-definitely-does-not-exist-12345");
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_list_installed_fast() {
        let result = list_installed_fast();
        assert!(result.is_ok());
        let packages = result.unwrap();
        assert!(!packages.is_empty(), "Should have installed packages");
        assert!(
            packages.iter().any(|p| p.name == "pacman"),
            "pacman should be installed"
        );
    }

    #[test]
    fn test_list_explicit_fast() {
        let result = list_explicit_fast();
        assert!(result.is_ok());
        let packages = result.unwrap();
        assert!(!packages.is_empty(), "Should have explicit packages");
    }

    #[test]
    fn test_list_orphans_fast() {
        let result = list_orphans_fast();
        assert!(result.is_ok());
    }

    #[test]
    fn test_is_installed_fast_pacman() {
        let result = is_installed_fast("pacman");
        assert!(result.is_ok());
        assert!(result.unwrap(), "pacman should be installed");
    }

    #[test]
    fn test_is_installed_fast_nonexistent() {
        let result = is_installed_fast("this-package-definitely-does-not-exist-12345");
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[test]
    fn test_get_counts() {
        let result = get_counts();
        assert!(result.is_ok());
        let (total, explicit, _orphans) = result.unwrap();
        assert!(total > 0, "Should have installed packages");
        assert!(explicit > 0, "Should have explicit packages");
        assert!(explicit <= total, "Explicit should be <= total");
    }

    #[test]
    fn test_list_all_package_names() {
        let result = list_all_package_names();
        assert!(result.is_ok());
        let names = result.unwrap();
        assert!(!names.is_empty());
        assert!(names.contains(&"pacman".to_string()));
        let is_sorted = names.windows(2).all(|w| w[0] <= w[1]);
        assert!(is_sorted, "Package names should be sorted");
    }

    #[test]
    fn test_local_package_has_valid_fields() {
        let result = list_installed_fast();
        assert!(result.is_ok());
        let packages = result.unwrap();

        for pkg in packages.iter().take(5) {
            assert!(!pkg.name.is_empty(), "Package name should not be empty");
            assert!(
                pkg.reason == "explicit" || pkg.reason == "dependency",
                "Reason should be explicit or dependency"
            );
        }
    }

    #[test]
    fn test_sync_package_has_repo() {
        let result = search_sync("linux");
        assert!(result.is_ok());
        let packages = result.unwrap();

        for pkg in packages.iter().take(5) {
            assert!(!pkg.repo.is_empty(), "Repo should not be empty");
        }
    }
}
