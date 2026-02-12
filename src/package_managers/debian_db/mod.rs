//! Pure Rust Debian/APT Package Parser
//!
//! Direct parsing of /var/lib/dpkg/status and /var/lib/apt/lists/*_Packages
//! without apt-cache. Provides <15ms cached lookups via rkyv memory-mapping.
//!
//! ## Modules
//!
//! - [`db`]: Core package database parsing and caching
//! - [`sources`]: APT sources.list and deb822 sources parser
//! - [`parallel_sync`]: Parallel repository synchronization
//! - [`resolver`]: Dependency resolution with version comparison
//! - [`transaction`]: Pure Rust transaction engine for .deb installation
//! - [`validation`]: Pre-flight checks and error validation

pub mod content_store;
pub mod db;
pub mod parallel_sync;
pub mod resolver;
#[cfg(feature = "debian-resolvo")]
pub mod resolvo_adapter;
pub mod sources;
pub mod transaction;
pub mod validation;

pub use content_store::ContentStore;
pub use db::{
    DebianMmapIndex, DebianPackage, DebianPackageIndex, LocalPackage, clean_package_cache,
    cleanup_expired_mmaps, ensure_index_loaded, ensure_mmap_loaded, get_all_packages_with_sizes,
    get_counts_fast, get_detailed_packages, get_info_fast, get_installed_info_fast,
    get_package_dependencies, get_package_size, get_package_version, get_updates_from_mmap,
    is_installed_fast, is_mmap_available, is_package_auto_installed, list_explicit_fast,
    list_installed_fast, list_orphans_fast, search_fast,
};

pub use parallel_sync::{force_sync_all, needs_sync, sync_all_repositories};
pub use resolver::{DependencyResolver, ResolutionResult, compare_versions};
#[cfg(feature = "debian-resolvo")]
pub use resolvo_adapter::ResolvoAdapter;
pub use sources::{
    RepoType, Repository, get_enabled_binary_repos, parse_all_sources, parse_deb822_content,
    parse_sources_list_content,
};
pub use transaction::{Transaction, TransactionState, dry_run};
pub use validation::{
    check_disk_space, check_mirror_availability, estimate_time_remaining, format_bytes,
    format_speed, validate_deb_archive, verify_package_hash,
};
