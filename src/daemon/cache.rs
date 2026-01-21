//! In-memory package cache with LRU eviction

use moka::sync::Cache;
use std::sync::LazyLock;
use std::time::Duration;

use super::protocol::{DetailedPackageInfo, PackageInfo, StatusResult};

// Static cache keys to avoid repeated allocations
static KEY_STATUS: LazyLock<String> = LazyLock::new(|| "status".to_string());
static KEY_EXPLICIT: LazyLock<String> = LazyLock::new(|| "explicit".to_string());
static KEY_EXPLICIT_COUNT: LazyLock<String> = LazyLock::new(|| "explicit_count".to_string());

/// LRU cache for package search results
pub struct PackageCache {
    /// Search results cache: query -> packages
    cache: Cache<String, Vec<PackageInfo>>,
    /// Detailed info cache: pkgname -> info
    detailed_cache: Cache<String, DetailedPackageInfo>,
    /// Negative cache for missing package info
    info_miss_cache: Cache<String, bool>,
    /// Maximum cache size
    max_size: usize,
    /// System status cache
    system_status: Cache<String, StatusResult>,
    /// Explicit package list cache
    explicit_packages: Cache<String, Vec<String>>,
    /// Explicit package count cache
    explicit_count: Cache<String, usize>,
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
            detailed_cache,
            info_miss_cache,
            max_size,
            system_status,
            explicit_packages,
            explicit_count: Cache::builder().max_capacity(1).time_to_live(ttl).build(),
        }
    }

    /// Get cached system status
    #[inline]
    #[must_use]
    pub fn get_status(&self) -> Option<StatusResult> {
        self.system_status.get(&*KEY_STATUS)
    }

    /// Update system status cache
    pub fn update_status(&self, result: StatusResult) {
        self.explicit_count
            .insert(KEY_EXPLICIT_COUNT.clone(), result.explicit_packages);
        self.system_status.insert(KEY_STATUS.clone(), result);
    }

    /// Get cached explicit packages
    pub fn get_explicit(&self) -> Option<Vec<String>> {
        self.explicit_packages.get(&*KEY_EXPLICIT)
    }

    /// Get cached explicit package count
    #[inline]
    #[must_use]
    pub fn get_explicit_count(&self) -> Option<usize> {
        self.explicit_count.get(&*KEY_EXPLICIT_COUNT)
    }

    /// Update explicit package cache
    pub fn update_explicit(&self, packages: Vec<String>) {
        self.explicit_count
            .insert(KEY_EXPLICIT_COUNT.clone(), packages.len());
        self.explicit_packages
            .insert(KEY_EXPLICIT.clone(), packages);
    }

    /// Update explicit package count cache
    pub fn update_explicit_count(&self, count: usize) {
        self.explicit_count
            .insert(KEY_EXPLICIT_COUNT.clone(), count);
    }

    /// Get cached results for a query
    #[inline]
    #[must_use]
    pub fn get(&self, query: &str) -> Option<Vec<PackageInfo>> {
        self.cache.get(query)
    }

    /// Store results in cache
    pub fn insert(&self, query: String, packages: Vec<PackageInfo>) {
        self.cache.insert(query, packages);
    }

    /// Get cache statistics
    #[must_use]
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            size: self.cache.entry_count() as usize,
            max_size: self.max_size,
        }
    }

    /// Clear the entire cache
    pub fn clear(&self) {
        self.cache.invalidate_all();
        self.detailed_cache.invalidate_all();
        self.info_miss_cache.invalidate_all();
        self.system_status.invalidate_all();
        self.explicit_packages.invalidate_all();
        self.explicit_count.invalidate_all();
    }

    /// Get detailed info from cache
    #[inline]
    #[must_use]
    pub fn get_info(&self, name: &str) -> Option<DetailedPackageInfo> {
        self.detailed_cache.get(name)
    }

    /// Check if package info is known to be missing
    pub fn is_info_miss(&self, name: &str) -> bool {
        self.info_miss_cache.get(name).unwrap_or(false)
    }

    /// Store detailed info in cache
    pub fn insert_info(&self, info: DetailedPackageInfo) {
        let name = info.name.clone();
        self.detailed_cache.insert(name.clone(), info);
        self.info_miss_cache.invalidate(&name);
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
        // 1000 entries, 5 minute TTL; status cache 30s
        Self::new_with_ttls(1000, 300, 30)
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
