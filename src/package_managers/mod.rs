//! Package manager backends for system packages

pub mod alpm_direct;
pub mod alpm_ops;
pub mod alpm_worker;
mod aur;
mod official;
pub mod pacman_db;
pub mod parallel_sync;
pub mod pkgbuild;
mod traits;

pub use alpm_direct::{
    LocalPackage, SyncPackage, get_counts, get_package_info, is_installed_fast, list_explicit_fast,
    list_installed_fast, list_orphans_fast, search_local, search_sync,
};
pub use alpm_ops::DownloadInfo;
pub use alpm_ops::PackageInfo as SyncPkgInfo;
pub use alpm_ops::{
    clean_cache, display_pkg_info, execute_transaction, get_sync_pkg_info, get_system_status,
    get_update_download_list, get_update_list, list_orphans_direct, sync_dbs,
};
pub use aur::{AurClient, AurPackageDetail, search_detailed};
pub use official::{
    OfficialPackageManager, is_installed, list_explicit, list_orphans, remove_orphans,
};
pub use pacman_db::{
    check_updates_cached, get_local_package, get_potential_aur_packages, invalidate_caches,
    preload_caches,
};
pub use parallel_sync::{
    DownloadJob, download_packages_parallel, select_fastest_mirrors, sync_databases_parallel,
};
pub use traits::PackageManager;
