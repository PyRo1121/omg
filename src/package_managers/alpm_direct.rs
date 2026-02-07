//! Direct ALPM (Arch Linux Package Manager) integration
//!
//! Uses libalpm directly for 10-100x faster queries compared to spawning pacman.

use alpm::{Alpm, PackageReason, SigLevel};
use anyhow::{Context, Result};

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::paths;
use crate::package_managers::pacman_db;
use crate::package_managers::types::{LocalPackage, PackageInfo, SyncPackage};

/// Zero-allocation case-insensitive substring search (ASCII-only)
#[inline]
fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.len() > haystack.len() {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle.len())
        .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
}

static CACHE_EPOCH: AtomicU64 = AtomicU64::new(0);

thread_local! {
    static ALPM_HANDLE: RefCell<Option<(Alpm, u64)>> = const { RefCell::new(None) };
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
#[expect(clippy::expect_used)] // ALPM handle initialization; failure indicates system misconfiguration
pub fn with_handle<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&Alpm) -> Result<R>,
{
    ALPM_HANDLE.with(|cell| {
        let current_epoch = CACHE_EPOCH.load(Ordering::Acquire);
        let mut maybe_handle = cell.borrow_mut();

        if !matches!(&*maybe_handle, Some((_, epoch)) if *epoch == current_epoch) {
            *maybe_handle = Some((create_alpm_handle()?, current_epoch));
        }

        // Get reference to handle
        let (handle_ref, _) = maybe_handle
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
#[expect(clippy::expect_used)] // ALPM handle initialization; failure indicates system misconfiguration
pub fn with_handle_mut<F, R>(f: F) -> Result<R>
where
    F: FnOnce(&mut Alpm) -> Result<R>,
{
    ALPM_HANDLE.with(|cell| {
        let current_epoch = CACHE_EPOCH.load(Ordering::Acquire);
        let mut maybe_handle = cell.borrow_mut();

        if !matches!(&*maybe_handle, Some((_, epoch)) if *epoch == current_epoch) {
            *maybe_handle = Some((create_alpm_handle()?, current_epoch));
        }

        let (handle_ref, _) = maybe_handle
            .as_mut()
            .expect("ALPM handle initialized above");

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(handle_ref)));

        drop(maybe_handle);

        match result {
            Ok(r) => r,
            Err(panic_payload) => std::panic::resume_unwind(panic_payload),
        }
    })
}

/// Invalidate ALPM handles across all threads.
///
/// Increments the global cache epoch, causing all threads to create fresh
/// ALPM handles on their next operation. This is necessary after sync
/// operations that run in a different process (via sudo).
pub fn clear_alpm_cache() {
    CACHE_EPOCH.fetch_add(1, Ordering::Release);
}

/// Search local database (installed packages) - INSTANT
#[inline]
pub fn search_local(query: &str) -> Result<Vec<LocalPackage>> {
    with_handle(|handle| {
        let localdb = handle.localdb();
        let query_lower = query.to_ascii_lowercase();
        let mut results = Vec::with_capacity(64);

        for pkg in localdb.pkgs() {
            if pkg.name().contains(&query_lower)
                || pkg
                    .desc()
                    .is_some_and(|d| contains_ignore_ascii_case(d, &query_lower))
            {
                results.push(LocalPackage {
                    name: pkg.name().to_string(),
                    version: super::types::parse_version_or_zero(pkg.version()),
                    description: pkg.desc().unwrap_or("").to_string(),
                    install_size: pkg.isize(),
                    reason: match pkg.reason() {
                        PackageReason::Explicit => "explicit",
                        PackageReason::Depend => "dependency",
                    },
                });
            }
        }

        Ok(results)
    })
}

/// Search sync databases (available packages) - FAST (<10ms)
#[inline]
pub fn search_sync(query: &str) -> Result<Vec<SyncPackage>> {
    with_handle(|handle| {
        let query_lower = query.to_ascii_lowercase();
        let mut results = Vec::with_capacity(64);

        for db in handle.syncdbs() {
            for pkg in db.pkgs() {
                if pkg.name().contains(&query_lower)
                    || pkg
                        .desc()
                        .is_some_and(|d| contains_ignore_ascii_case(d, &query_lower))
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
#[inline]
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

/// Batch get package info for multiple packages - 10-50x faster than individual lookups
/// Single ALPM handle call amortizes overhead across all packages
#[inline]
pub fn get_package_info_batch(names: &[&str]) -> Result<Vec<Option<PackageInfo>>> {
    with_handle(|handle| {
        let localdb = handle.localdb();
        let syncdbs: Vec<_> = handle.syncdbs().iter().collect();

        let results = names
            .iter()
            .map(|name| {
                if let Ok(pkg) = localdb.pkg(*name) {
                    return Some(PackageInfo {
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
                    });
                }

                for db in &syncdbs {
                    if let Ok(pkg) = db.pkg(*name) {
                        return Some(PackageInfo {
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
                        });
                    }
                }

                None
            })
            .collect();

        Ok(results)
    })
}

/// List all installed packages - INSTANT
pub fn list_installed_fast() -> Result<Vec<LocalPackage>> {
    with_handle(|handle| {
        let localdb = handle.localdb();
        let pkg_count = localdb.pkgs().len();

        let mut results = Vec::with_capacity(pkg_count);
        results.extend(localdb.pkgs().iter().map(|pkg| LocalPackage {
            name: pkg.name().to_string(),
            version: super::types::parse_version_or_zero(pkg.version()),
            description: pkg.desc().unwrap_or("").to_string(),
            install_size: pkg.isize(),
            reason: match pkg.reason() {
                PackageReason::Explicit => "explicit",
                PackageReason::Depend => "dependency",
            },
        }));

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

/// List installed packages with license information - for compliance scanning
pub fn list_installed_with_licenses() -> Result<Vec<(String, String, String)>> {
    with_handle(|handle| {
        let pkgs = handle.localdb().pkgs();
        let mut results = Vec::with_capacity(pkgs.len());
        results.extend(pkgs.iter().map(|pkg| {
            let licenses = pkg.licenses();
            let license_str = if licenses.is_empty() {
                "Unknown".to_string()
            } else {
                licenses
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            (
                pkg.name().to_string(),
                license_str,
                pkg.version().to_string(),
            )
        }));

        Ok(results)
    })
}

/// Check if a package has an available update
pub fn has_update(package: &str) -> Result<bool> {
    with_handle(|handle| {
        // Get local version
        let localdb = handle.localdb();
        let local_pkg = localdb.pkg(package)?;
        let local_ver = local_pkg.version();

        // Check sync databases for newer version
        for db in handle.syncdbs() {
            if let Ok(sync_pkg) = db.pkg(package) {
                let sync_ver = sync_pkg.version();
                if alpm::vercmp(sync_ver.as_str(), local_ver.as_str())
                    == std::cmp::Ordering::Greater
                {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    })
}

/// Check if package is installed - INSTANT
#[inline]
pub fn is_installed_fast(name: &str) -> Result<bool> {
    with_handle(|handle| Ok(handle.localdb().pkg(name).is_ok()))
}

/// Get counts - INSTANT (<1ms, single pass over packages)
#[inline]
pub fn get_counts() -> Result<(usize, usize, usize)> {
    with_handle(|handle| {
        let pkgs = handle.localdb().pkgs();
        let total = pkgs.len();

        // Single-pass counting for cache efficiency
        let (explicit, orphans) = pkgs.iter().fold((0, 0), |(mut exp, mut orp), pkg| {
            if pkg.reason() == PackageReason::Explicit {
                exp += 1;
            } else if pkg.required_by().is_empty() {
                orp += 1;
            }
            (exp, orp)
        });

        Ok((total, explicit, orphans))
    })
}

/// Get explicit count only - INSTANT (<500µs, optimized for count-only queries)
/// This is faster than `list_explicit_fast` when you only need the count
#[inline]
pub fn get_explicit_count_fast() -> Result<usize> {
    with_handle(|handle| {
        Ok(handle
            .localdb()
            .pkgs()
            .iter()
            .filter(|p| p.reason() == PackageReason::Explicit)
            .count())
    })
}

/// List all known package names (local + sync) for completion - FAST
#[inline]
pub fn list_all_package_names() -> Result<Vec<String>> {
    with_handle(|handle| {
        let localdb = handle.localdb();
        let sync_count: usize = handle.syncdbs().iter().map(|db| db.pkgs().len()).sum();
        let mut names = ahash::AHashSet::with_capacity(localdb.pkgs().len() + sync_count);

        for pkg in localdb.pkgs() {
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
