//! Package manager backends for system packages
//!
//! ## Feature Flags for Debian Support
//!
//! - `debian`: Adds rust-apt FFI for all operations (requires libapt-pkg-dev)

use std::sync::Arc;

#[cfg(feature = "arch")]
pub mod alpm_direct;
#[cfg(feature = "arch")]
pub mod alpm_ops;
#[cfg(feature = "arch")]
pub mod alpm_worker;
// apt module is available with debian feature
#[cfg(feature = "debian")]
pub mod apt;
#[cfg(feature = "arch")]
pub mod arch;
#[cfg(feature = "arch")]
pub mod aur;
#[cfg(feature = "arch")]
pub mod aur_deps;
#[cfg(feature = "arch")]
mod aur_index;
#[cfg(feature = "arch")]
pub mod aur_metadata;
#[cfg(feature = "arch")]
pub mod aur_sources;
#[cfg(any(feature = "debian", feature = "debian-pure"))]
pub mod debian_db;
#[cfg(feature = "debian-pure")]
pub mod debian_pure;
#[cfg(feature = "fedora")]
pub mod dnf;
// macOS Homebrew support - can be enabled via feature or auto-detected on macOS
#[cfg(any(feature = "macos", target_os = "macos"))]
pub mod homebrew;
pub mod mock;
#[cfg(feature = "arch")]
pub mod pacman_db;
#[cfg(feature = "arch")]
pub mod parallel_sync;
#[cfg(feature = "arch")]
pub mod pkgbuild;
mod traits;
pub mod types;
#[cfg(feature = "windows")]
pub mod windows;

pub use types::{parse_version_or_zero, zero_version};

#[cfg(feature = "arch")]
pub fn search_sync(query: &str) -> anyhow::Result<Vec<SyncPackage>> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return Ok(debian_db::search_fast(query)?
            .into_iter()
            .map(|pkg| SyncPackage {
                name: pkg.name,
                version: pkg.version,
                description: pkg.description,
                repo: "official".to_string(),
                download_size: 0,
                installed: pkg.installed,
            })
            .collect());
    }

    if crate::core::paths::test_mode() {
        let pm = get_package_manager()?;
        let results = futures::executor::block_on(pm.search(query))?;
        return Ok(results
            .into_iter()
            .map(|p| SyncPackage {
                name: p.name,
                version: p.version,
                description: p.description,
                repo: "official".to_string(),
                download_size: 0,
                installed: p.installed,
            })
            .collect());
    }
    alpm_direct::search_sync(query)
}

pub fn list_explicit_fast() -> anyhow::Result<Vec<String>> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return debian_db::list_explicit_fast();
    }

    #[cfg(feature = "arch")]
    {
        if crate::core::paths::test_mode() {
            let pm = get_package_manager()?;
            return futures::executor::block_on(pm.list_explicit());
        }
        return alpm_direct::list_explicit_fast();
    }

    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    return debian_db::list_explicit_fast();

    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    anyhow::bail!("No package manager backend enabled")
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn local_packages_from_debian_db() -> anyhow::Result<Vec<LocalPackage>> {
    Ok(debian_db::list_installed_fast()?
        .into_iter()
        .map(|pkg| LocalPackage {
            name: pkg.name,
            version: parse_version_or_zero(&pkg.version),
            description: pkg.description,
            install_size: 0,
            reason: if pkg.is_explicit {
                "explicit"
            } else {
                "dependency"
            },
        })
        .collect())
}

pub fn list_installed_fast() -> anyhow::Result<Vec<LocalPackage>> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return local_packages_from_debian_db();
    }

    #[cfg(feature = "arch")]
    return alpm_direct::list_installed_fast();

    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    return local_packages_from_debian_db();

    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    anyhow::bail!("No package manager backend enabled")
}

pub fn is_installed_fast(name: &str) -> anyhow::Result<bool> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return debian_db::is_installed_fast(name);
    }

    #[cfg(feature = "arch")]
    return alpm_direct::is_installed_fast(name);

    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    return debian_db::is_installed_fast(name);

    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    anyhow::bail!("No package manager backend enabled to query {name}")
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn package_info_from_debian_db(name: &str) -> anyhow::Result<Option<types::PackageInfo>> {
    Ok(
        debian_db::get_info_fast(name)?.map(|pkg| types::PackageInfo {
            name: pkg.name,
            version: pkg.version,
            description: pkg.description,
            url: None,
            size: 0,
            install_size: None,
            download_size: None,
            repo: if pkg.installed {
                "local".to_string()
            } else {
                "official".to_string()
            },
            depends: Vec::new(),
            licenses: Vec::new(),
            installed: pkg.installed,
        }),
    )
}

pub fn get_package_info(name: &str) -> anyhow::Result<Option<types::PackageInfo>> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return package_info_from_debian_db(name);
    }

    #[cfg(feature = "arch")]
    return alpm_direct::get_package_info(name);

    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    return package_info_from_debian_db(name);

    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    anyhow::bail!("No package manager backend enabled to query {name}")
}

pub fn list_orphans_fast() -> anyhow::Result<Vec<String>> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return debian_db::list_orphans_fast();
    }

    #[cfg(feature = "arch")]
    return alpm_direct::list_orphans_fast();

    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    return debian_db::list_orphans_fast();

    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    anyhow::bail!("No package manager backend enabled")
}

#[cfg(feature = "arch")]
pub use alpm_direct::{clear_alpm_cache, get_counts, search_local};
#[cfg(feature = "arch")]
pub use alpm_ops::DownloadInfo;
#[cfg(feature = "arch")]
pub use alpm_ops::{
    clean_cache, display_pkg_info, execute_transaction, get_sync_pkg_info, get_system_status,
    get_update_download_list, get_update_list, list_orphans_direct, sync_dbs,
};
#[cfg(feature = "arch")]
pub use arch::{ArchPackageManager, is_installed, list_explicit, list_orphans, remove_orphans};
#[cfg(feature = "arch")]
pub use aur::{AurClient, AurPackageDetail, search_detailed};
#[cfg(feature = "arch")]
pub use pacman_db::{
    check_updates_cached, get_local_package, get_potential_aur_packages, invalidate_caches,
    preload_caches,
};
#[cfg(feature = "arch")]
pub use parallel_sync::{
    DownloadJob, download_packages_parallel, select_fastest_mirrors, sync_databases_parallel,
};
pub use traits::PackageManager;
#[cfg(feature = "arch")]
pub use types::PackageInfo as SyncPkgInfo;
pub use types::{LocalPackage, SyncPackage};

/// Get the appropriate package manager for the current distribution
pub fn get_package_manager() -> anyhow::Result<Arc<dyn PackageManager>> {
    #[allow(unused_imports)]
    // Feature-gated re-exports; not all features compile the same subset
    use crate::core::env::distro::{Distro, detect_distro};

    if crate::core::paths::test_mode() {
        let distro = std::env::var("OMG_TEST_DISTRO").unwrap_or_else(|_| "arch".to_string());
        return Ok(Arc::new(mock::MockPackageManager::new(&distro)));
    }

    match detect_distro() {
        #[cfg(feature = "arch")]
        Distro::Arch => Ok(Arc::new(ArchPackageManager::new())),
        // debian provides AptPackageManager
        #[cfg(feature = "debian")]
        Distro::Debian | Distro::Ubuntu => Ok(Arc::new(AptPackageManager::new())),
        // debian-pure provides PureDebianPackageManager
        #[cfg(all(not(feature = "debian"), feature = "debian-pure"))]
        Distro::Debian | Distro::Ubuntu => {
            Ok(Arc::new(debian_pure::PureDebianPackageManager::new()))
        }
        // Fedora/RHEL provides DnfPackageManager (pure Rust)
        #[cfg(feature = "fedora")]
        Distro::Fedora => Ok(Arc::new(dnf::DnfPackageManager::new())),
        // macOS provides HomebrewPackageManager
        #[cfg(any(feature = "macos", target_os = "macos"))]
        Distro::MacOS => Ok(Arc::new(homebrew::HomebrewPackageManager::new())),
        // Windows provides WindowsPackageManager
        #[cfg(feature = "windows")]
        Distro::Windows => Ok(Arc::new(windows::WindowsPackageManager::new())),
        _ => {
            // Fallback or default
            #[cfg(feature = "arch")]
            return Ok(Arc::new(ArchPackageManager::new()));

            #[cfg(all(not(feature = "arch"), feature = "debian"))]
            return Ok(Arc::new(AptPackageManager::new()));

            #[cfg(all(
                not(feature = "arch"),
                not(feature = "debian"),
                feature = "debian-pure"
            ))]
            return Ok(Arc::new(debian_pure::PureDebianPackageManager::new()));

            #[cfg(all(
                not(feature = "arch"),
                not(feature = "debian"),
                not(feature = "debian-pure"),
                any(feature = "macos", target_os = "macos")
            ))]
            return Ok(Arc::new(homebrew::HomebrewPackageManager::new()));

            #[cfg(all(
                not(feature = "arch"),
                not(feature = "debian"),
                not(feature = "debian-pure"),
                not(any(feature = "macos", target_os = "macos")),
                feature = "fedora"
            ))]
            return Ok(Arc::new(dnf::DnfPackageManager::new()));

            #[cfg(all(
                not(feature = "arch"),
                not(feature = "debian"),
                not(feature = "debian-pure"),
                not(any(feature = "macos", target_os = "macos")),
                not(feature = "fedora"),
                feature = "windows"
            ))]
            return Ok(Arc::new(windows::WindowsPackageManager::new()));

            #[cfg(not(any(
                feature = "arch",
                feature = "debian",
                feature = "debian-pure",
                feature = "fedora",
                feature = "windows"
            )))]
            #[cfg(not(target_os = "macos"))]
            #[allow(unreachable_code)]
            {
                anyhow::bail!(
                    "No package manager backend enabled! Build with --features arch, debian, fedora, or windows"
                );
            }
        }
    }
}

// apt exports are available with debian feature
#[cfg(feature = "debian")]
pub fn apt_search_sync(query: &str) -> anyhow::Result<Vec<SyncPackage>> {
    if crate::core::paths::test_mode() {
        let pm = get_package_manager()?;
        let results = futures::executor::block_on(pm.search(query))?;
        return Ok(results
            .into_iter()
            .map(|p| SyncPackage {
                name: p.name,
                version: p.version,
                description: p.description,
                repo: "main".to_string(),
                download_size: 0,
                installed: p.installed,
            })
            .collect());
    }
    apt::search_sync(query)
}

#[cfg(feature = "debian")]
pub fn apt_list_explicit() -> anyhow::Result<Vec<String>> {
    if crate::core::paths::test_mode() {
        let pm = get_package_manager()?;
        return futures::executor::block_on(pm.list_explicit());
    }
    apt::list_explicit()
}

#[cfg(feature = "debian")]
pub use apt::{
    AptPackageManager, get_sync_pkg_info as apt_get_sync_pkg_info,
    get_system_status as apt_get_system_status,
    list_all_package_names as apt_list_all_package_names,
    list_installed_fast as apt_list_installed_fast, list_orphans as apt_list_orphans,
    list_updates as apt_list_updates, remove_orphans as apt_remove_orphans,
};
#[cfg(any(feature = "debian", feature = "debian-pure"))]
pub use debian_db::{
    get_counts_fast as apt_get_counts_fast, get_info_fast as apt_get_info_fast,
    list_explicit_fast as apt_list_explicit_fast, search_fast as apt_search_fast,
};

#[cfg(all(
    any(feature = "debian", feature = "debian-pure"),
    not(feature = "debian")
))]
pub use debian_db::list_installed_fast as apt_list_installed_fast;

#[cfg(all(feature = "debian", feature = "debian-pure"))]
pub use debian_db::list_installed_fast as apt_list_installed_db_fast;

// Homebrew exports are available on macOS
#[cfg(any(feature = "macos", target_os = "macos"))]
pub use homebrew::HomebrewPackageManager;

// DNF/RPM exports are available with fedora feature
#[cfg(feature = "fedora")]
pub use dnf::DnfPackageManager;

// Windows exports are available with windows feature
#[cfg(feature = "windows")]
pub use windows::WindowsPackageManager;
