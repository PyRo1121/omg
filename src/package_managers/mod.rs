//! Package manager backends for system packages

pub mod alpm_direct;
pub mod alpm_ops;
pub mod alpm_worker;
mod arch;
mod aur;
pub mod pkgbuild;
mod traits;

pub use alpm_direct::{
    get_counts, get_package_info, is_installed_fast, list_explicit_fast, list_installed_fast,
    list_orphans_fast, search_local, search_sync, LocalPackage, SyncPackage,
};
pub use alpm_ops::PackageInfo as SyncPkgInfo;
pub use alpm_ops::{
    clean_cache, display_pkg_info, execute_transaction, get_sync_pkg_info, get_system_status,
    get_update_list, list_orphans_direct, sync_dbs,
};
pub use arch::{is_installed, list_explicit, list_orphans, remove_orphans, ArchPackageManager};
pub use aur::{search_detailed, AurClient, AurPackageDetail};
pub use traits::PackageManager;
