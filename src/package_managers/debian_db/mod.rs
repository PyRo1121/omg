//! Pure Rust Debian/APT Package Parser
//!
//! Direct parsing of /var/lib/dpkg/status and /var/lib/apt/lists/*_Packages
//! without apt-cache. Provides <15ms cached lookups via rkyv memory-mapping.

mod db;

pub use db::{
    DebianMmapIndex, DebianPackage, DebianPackageIndex, LocalPackage, cleanup_expired_mmaps,
    ensure_index_loaded, get_all_packages_with_sizes, get_counts_fast, get_detailed_packages,
    get_info_fast, get_installed_info_fast, get_package_dependencies, get_package_size,
    get_package_version, is_installed_fast, is_package_auto_installed, list_explicit_fast,
    list_installed_fast, search_fast,
};
