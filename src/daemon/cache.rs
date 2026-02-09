//! In-memory package cache with LRU eviction

use moka::sync::Cache;
use std::sync::Arc;
use std::time::Duration;

use super::protocol::{DetailedPackageInfo, PackageInfo, StatusResult};

/// Static cache keys (avoids String allocation on every cache access)
const KEY_STATUS: &str = "status";
const KEY_EXPLICIT: &str = "explicit";
const KEY_EXPLICIT_COUNT: &str = "explicit_count";

/// LRU cache for package search results
pub struct PackageCache {
    /// Search results cache: query -> packages (Arc for cheap cloning)
    cache: Cache<String, Arc<Vec<PackageInfo>>>,
    /// Debian search results cache: query -> package info (Arc for cheap cloning)
    debian_cache: Cache<String, Arc<Vec<PackageInfo>>>,
    /// Detailed info cache: pkgname -> info (Arc for cheap cloning)
    detailed_cache: Cache<String, Arc<DetailedPackageInfo>>,
    /// Negative cache for missing package info
    info_miss_cache: Cache<String, bool>,
    /// Maximum cache size
    max_size: usize,
    /// System status cache - uses &'static str keys to avoid allocation
    system_status: Cache<&'static str, Arc<StatusResult>>,
    /// Explicit package list cache - uses &'static str keys to avoid allocation
    explicit_packages: Cache<&'static str, Arc<Vec<String>>>,
    /// Explicit package count cache - uses &'static str keys to avoid allocation
    explicit_count: Cache<&'static str, usize>,
}

impl PackageCache {
    /// Create a new cache with given size and TTL
    #[must_use]
    pub fn new(max_size: usize, ttl_secs: u64) -> Self {
        Self::new_with_ttls(max_size, ttl_secs, ttl_secs)
    }

    /// Create a new cache with separate TTLs for search and status
    #[must_use]
    pub fn new_with_ttls(max_size: usize, ttl_secs: u64, status_ttl_secs: u64) -> Self {
        let ttl = Duration::from_secs(ttl_secs);
        let status_ttl = Duration::from_secs(status_ttl_secs);
        let cache = Cache::builder()
            .max_capacity(max_size as u64)
            .time_to_live(ttl)
            .build();
        let debian_cache = Cache::builder()
            .max_capacity(max_size as u64)
            .time_to_live(ttl)
            .build();
        let detailed_cache = Cache::builder()
            .max_capacity(max_size as u64)
            .time_to_live(ttl)
            .build();
        let info_miss_cache = Cache::builder()
            .max_capacity(max_size as u64)
            .time_to_live(ttl)
            .build();
        let system_status = Cache::builder()
            .max_capacity(1)
            .time_to_live(status_ttl)
            .build();
        let explicit_packages = Cache::builder()
            .max_capacity(1)
            .time_to_live(status_ttl)
            .build();

        Self {
            cache,
            debian_cache,
            detailed_cache,
            info_miss_cache,
            max_size,
            system_status,
            explicit_packages,
            explicit_count: Cache::builder().max_capacity(1).time_to_live(ttl).build(),
        }
    }

    /// Get cached system status (Arc clone is cheap - just pointer copy)
    /// Inlined for hot-path performance (called on every `omg status`)
    #[inline]
    #[must_use]
    pub fn get_status(&self) -> Option<Arc<StatusResult>> {
        self.system_status.get(KEY_STATUS)
    }

    /// Update system status cache (accepts Arc to avoid double-wrapping)
    pub fn update_status(&self, result: Arc<StatusResult>) {
        self.explicit_count
            .insert(KEY_EXPLICIT_COUNT, result.explicit_packages);
        self.system_status.insert(KEY_STATUS, result);
    }

    /// Get cached explicit packages (Arc clone is cheap - just pointer copy)
    #[inline]
    #[must_use]
    pub fn get_explicit(&self) -> Option<Arc<Vec<String>>> {
        self.explicit_packages.get(KEY_EXPLICIT)
    }

    /// Get cached explicit package count
    #[inline]
    #[must_use]
    pub fn get_explicit_count(&self) -> Option<usize> {
        self.explicit_count.get(KEY_EXPLICIT_COUNT)
    }

    /// Update explicit package cache
    pub fn update_explicit(&self, packages: Vec<String>) {
        self.explicit_count
            .insert(KEY_EXPLICIT_COUNT, packages.len());
        self.explicit_packages
            .insert(KEY_EXPLICIT, Arc::new(packages));
    }

    pub fn update_explicit_arc(&self, packages: Arc<Vec<String>>) {
        self.explicit_count
            .insert(KEY_EXPLICIT_COUNT, packages.len());
        self.explicit_packages.insert(KEY_EXPLICIT, packages);
    }

    /// Update explicit package count cache
    pub fn update_explicit_count(&self, count: usize) {
        self.explicit_count.insert(KEY_EXPLICIT_COUNT, count);
    }

    /// Get cached results for a query (Arc clone is cheap - just pointer copy)
    /// Inlined for hot-path performance (called on every search)
    #[inline]
    #[must_use]
    pub fn get(&self, query: &str) -> Option<Arc<Vec<PackageInfo>>> {
        self.cache.get(query)
    }

    /// Store results in cache
    pub fn insert(&self, query: String, packages: Vec<PackageInfo>) {
        self.cache.insert(query, Arc::new(packages));
    }

    /// Store Arc'd results in cache (avoids double-wrapping)
    pub fn insert_arc(&self, query: String, packages: Arc<Vec<PackageInfo>>) {
        self.cache.insert(query, packages);
    }

    /// Get cached Debian search results (Arc clone is cheap - just pointer copy)
    #[inline]
    #[must_use]
    pub fn get_debian(&self, query: &str) -> Option<Arc<Vec<PackageInfo>>> {
        self.debian_cache.get(query)
    }

    /// Store Debian search results in cache
    pub fn insert_debian(&self, query: String, packages: Vec<PackageInfo>) {
        self.debian_cache.insert(query, Arc::new(packages));
    }

    /// Store Arc'd Debian search results in cache (avoids double-wrapping)
    pub fn insert_debian_arc(&self, query: String, packages: Arc<Vec<PackageInfo>>) {
        self.debian_cache.insert(query, packages);
    }

    /// Get cache statistics
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            size: self.cache.entry_count().min(usize::MAX as u64) as usize,
            max_size: self.max_size,
        }
    }

    /// Clear the entire cache
    pub fn clear(&self) {
        self.cache.invalidate_all();
        self.debian_cache.invalidate_all();
        self.detailed_cache.invalidate_all();
        self.info_miss_cache.invalidate_all();
        self.system_status.invalidate_all();
        self.explicit_packages.invalidate_all();
        self.explicit_count.invalidate_all();
    }

    /// Sync pending cache operations
    /// Moka cache is eventually consistent, this ensures all pending operations complete.
    /// Primarily used in tests to ensure cache state is synchronized before assertions.
    pub fn sync(&self) {
        self.cache.run_pending_tasks();
        self.debian_cache.run_pending_tasks();
        self.detailed_cache.run_pending_tasks();
        self.info_miss_cache.run_pending_tasks();
        self.system_status.run_pending_tasks();
        self.explicit_packages.run_pending_tasks();
        self.explicit_count.run_pending_tasks();
    }

    /// Get detailed info from cache (Arc clone is cheap - just pointer copy)
    /// Inlined for hot-path performance (called on every `omg info`)
    #[inline]
    #[must_use]
    pub fn get_info(&self, name: &str) -> Option<Arc<DetailedPackageInfo>> {
        self.detailed_cache.get(name)
    }

    /// Check if package info is known to be missing (negative cache)
    /// Inlined for hot-path performance (prevents unnecessary lookups)
    #[inline]
    #[must_use]
    pub fn is_info_miss(&self, name: &str) -> bool {
        self.info_miss_cache.get(name).is_some()
    }

    /// Store detailed info in cache (optimized to clone name once)
    pub fn insert_info(&self, info: DetailedPackageInfo) {
        let name = info.name.clone();
        self.info_miss_cache.invalidate(&name);
        self.detailed_cache.insert(name, Arc::new(info));
    }

    /// Store Arc'd detailed info in cache (avoids double-wrapping)
    pub fn insert_info_arc(&self, info: Arc<DetailedPackageInfo>) {
        let name = info.name.clone();
        self.info_miss_cache.invalidate(&name);
        self.detailed_cache.insert(name, info);
    }

    /// Record a missing package info lookup
    pub fn insert_info_miss(&self, name: &str) {
        self.info_miss_cache.insert(name.to_string(), true);
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
}

impl Default for PackageCache {
    fn default() -> Self {
        // 1000 entries, 5 minute TTL for search results
        // Status cache: 2 minutes (frequent operations are fast with ALPM caching)
        // This reduces unnecessary status refreshes while still feeling responsive
        Self::new_with_ttls(1000, 300, 120)
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
