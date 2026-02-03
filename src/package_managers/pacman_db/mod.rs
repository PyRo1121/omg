//! Pure Rust Pacman Database Parser
//!
//! Direct parsing of /var/lib/pacman/sync/*.db and /var/lib/pacman/local/
//! without libalpm. Provides <1ms cached lookups via rkyv memory-mapping.

mod db;

pub use db::{
    LocalDbPackage, PacmanMmapIndex, RkyvSyncIndex, RkyvSyncPackage, SyncDbPackage,
    check_updates_cached, check_updates_fast, compare_versions, get_counts_fast,
    get_detailed_packages, get_explicit_count, get_local_package, get_potential_aur_packages,
    get_sync_package, invalidate_caches, is_installed_cached, list_all_names_cached,
    list_local_cached, parse_local_db, parse_sync_db, preload_caches, search_local_cached,
    search_sync_fast, search_sync_mmap, verify_aur_packages,
};
