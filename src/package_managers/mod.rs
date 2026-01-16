//! Package manager backends for system packages

#[cfg(feature = "arch")]
pub mod alpm_direct;
#[cfg(feature = "arch")]
pub mod alpm_ops;
#[cfg(feature = "arch")]
pub mod alpm_worker;
#[cfg(feature = "debian")]
pub mod apt;
#[cfg(feature = "arch")]
mod aur;
#[cfg(feature = "arch")]
mod official;
#[cfg(feature = "arch")]
pub mod pacman_db;
#[cfg(feature = "arch")]
pub mod parallel_sync;
#[cfg(feature = "arch")]
pub mod pkgbuild;
mod traits;
mod types;

pub use types::{parse_version_or_zero, zero_version};

#[cfg(feature = "arch")]
pub use alpm_direct::{
    get_counts, get_package_info, is_installed_fast, list_explicit_fast, list_installed_fast,
    list_orphans_fast, search_local, search_sync,
};
#[cfg(feature = "arch")]
pub use alpm_ops::DownloadInfo;
#[cfg(feature = "arch")]
pub use alpm_ops::{
    clean_cache, display_pkg_info, execute_transaction, get_sync_pkg_info, get_system_status,
    get_update_download_list, get_update_list, list_orphans_direct, sync_dbs,
};
#[cfg(feature = "arch")]
pub use aur::{AurClient, AurPackageDetail, search_detailed};
#[cfg(feature = "arch")]
pub use official::{
    OfficialPackageManager, is_installed, list_explicit, list_orphans, remove_orphans,
};
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

#[cfg(feature = "debian")]
pub use apt::{
    AptPackageManager, get_sync_pkg_info as apt_get_sync_pkg_info,
    get_system_status as apt_get_system_status,
    list_all_package_names as apt_list_all_package_names, list_explicit as apt_list_explicit,
    list_installed_fast as apt_list_installed_fast, list_orphans as apt_list_orphans,
    list_updates as apt_list_updates, remove_orphans as apt_remove_orphans,
    search_sync as apt_search_sync,
};
