//! In-memory package cache with LRU eviction

use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::protocol::{DetailedPackageInfo, PackageInfo, StatusResult};

/// Cache entry with timestamp
#[derive(Clone)]
struct CacheEntry {
    packages: Vec<PackageInfo>,
    timestamp: Instant,
}

/// Detailed info cache entry
#[derive(Clone)]
struct DetailedEntry {
    info: DetailedPackageInfo,
    timestamp: Instant,
}

/// LRU cache for package search results
pub struct PackageCache {
    /// Search results cache: query -> packages
    cache: DashMap<String, CacheEntry>,
    /// Detailed info cache: pkgname -> info
    detailed_cache: DashMap<String, DetailedEntry>,
    /// LRU order tracking
    lru_order: RwLock<VecDeque<String>>,
    /// Maximum cache size
    max_size: usize,
    /// Cache TTL
    ttl: Duration,
    /// System status cache
    system_status: RwLock<Option<(StatusResult, Instant)>>,
}

impl PackageCache {
    /// Create a new cache with given size and TTL
    pub fn new(max_size: usize, ttl_secs: u64) -> Self {
        PackageCache {
            cache: DashMap::new(),
            detailed_cache: DashMap::new(),
            lru_order: RwLock::new(VecDeque::with_capacity(max_size)),
            max_size,
            ttl: Duration::from_secs(ttl_secs),
            system_status: RwLock::new(None),
        }
    }

    /// Get cached system status
    pub fn get_status(&self) -> Option<StatusResult> {
        let status = self.system_status.read();
        if let Some((res, timestamp)) = status.as_ref() {
            if timestamp.elapsed() < self.ttl {
                return Some(res.clone());
            }
        }
        None
    }

    /// Update system status cache
    pub fn update_status(&self, result: StatusResult) {
        let mut status = self.system_status.write();
        *status = Some((result, Instant::now()));
    }

    /// Get cached results for a query
    pub fn get(&self, query: &str) -> Option<Vec<PackageInfo>> {
        let entry = self.cache.get(query)?;

        // Check if expired
        if entry.timestamp.elapsed() > self.ttl {
            drop(entry);
            self.cache.remove(query);
            return None;
        }

        // Update LRU order
        self.touch(query);

        Some(entry.packages.clone())
    }

    /// Store results in cache
    pub fn insert(&self, query: String, packages: Vec<PackageInfo>) {
        // Evict if at capacity
        while self.cache.len() >= self.max_size {
            self.evict_oldest();
        }

        self.cache.insert(
            query.clone(),
            CacheEntry {
                packages,
                timestamp: Instant::now(),
            },
        );

        // Add to LRU
        let mut lru = self.lru_order.write();
        lru.push_back(query);
    }

    /// Touch an entry to mark it as recently used
    fn touch(&self, query: &str) {
        let mut lru = self.lru_order.write();

        // Remove from current position
        if let Some(pos) = lru.iter().position(|k| k == query) {
            lru.remove(pos);
        }

        // Add to back (most recent)
        lru.push_back(query.to_string());
    }

    /// Evict the oldest entry
    fn evict_oldest(&self) {
        let mut lru = self.lru_order.write();
        if let Some(oldest) = lru.pop_front() {
            self.cache.remove(&oldest);
        }
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            size: self.cache.len(),
            max_size: self.max_size,
        }
    }

    /// Clear the entire cache
    pub fn clear(&self) {
        self.cache.clear();
        self.detailed_cache.clear();
        self.lru_order.write().clear();
    }

    /// Get detailed info from cache
    pub fn get_info(&self, name: &str) -> Option<DetailedPackageInfo> {
        let entry = self.detailed_cache.get(name)?;
        if entry.timestamp.elapsed() > self.ttl {
            drop(entry);
            self.detailed_cache.remove(name);
            return None;
        }
        Some(entry.info.clone())
    }

    /// Store detailed info in cache
    pub fn insert_info(&self, info: DetailedPackageInfo) {
        self.detailed_cache.insert(
            info.name.clone(),
            DetailedEntry {
                info,
                timestamp: Instant::now(),
            },
        );
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
        // 1000 entries, 5 minute TTL
        Self::new(1000, 300)
    }
}
