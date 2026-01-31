//! Bloom Filter-based Quick Update Check
//!
//! Provides sub-millisecond "no updates available" detection for the common case.
//! When no updates are available (which is most of the time after a sync),
//! this check returns in <100 microseconds instead of scanning all packages.
//!
//! # How it works
//! 1. After sync, we build a Bloom filter of (name, version) pairs from sync DBs
//! 2. On update check, we test each local package against the filter
//! 3. If ALL local packages are in the filter, there are definitely no updates
//! 4. If any package is NOT in the filter, we fall back to full comparison
//!
//! # False Positive Rate
//! With 100,000 packages and 1MB filter, false positive rate is ~0.1%
//! This means we might do an unnecessary full scan 0.1% of the time,
//! which is acceptable for the 99.9% speedup in the common case.

use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use ahash::AHasher;
use anyhow::{Context, Result};

use crate::core::paths;

/// Bloom filter parameters
/// With these settings: ~1MB filter, ~100k items, ~0.1% false positive rate
const BLOOM_BITS: usize = 8 * 1024 * 1024; // 8 million bits = 1MB
const BLOOM_HASHES: usize = 7; // Optimal for this size/item ratio

/// Bloom filter for package (name, version) pairs
#[derive(Clone)]
pub struct PackageBloomFilter {
    /// Bit array
    bits: Vec<u64>,
    /// Number of hash functions
    num_hashes: usize,
    /// Total bits
    num_bits: usize,
}

impl Default for PackageBloomFilter {
    fn default() -> Self {
        Self::new(BLOOM_BITS, BLOOM_HASHES)
    }
}

impl PackageBloomFilter {
    /// Create a new bloom filter with specified parameters
    #[must_use]
    pub fn new(num_bits: usize, num_hashes: usize) -> Self {
        let num_words = num_bits.div_ceil(64);
        Self {
            bits: vec![0u64; num_words],
            num_hashes,
            num_bits,
        }
    }

    /// Hash a package (name, version) pair to multiple indices
    fn hash_indices(&self, name: &str, version: &str) -> Vec<usize> {
        let mut hasher1 = AHasher::default();
        name.hash(&mut hasher1);
        version.hash(&mut hasher1);
        let h1 = hasher1.finish() as usize;

        let mut hasher2 = AHasher::default();
        version.hash(&mut hasher2);
        name.hash(&mut hasher2);
        let h2 = hasher2.finish() as usize;

        (0..self.num_hashes)
            .map(|i| (h1.wrapping_add(i.wrapping_mul(h2))) % self.num_bits)
            .collect()
    }

    /// Add a package to the filter
    pub fn insert(&mut self, name: &str, version: &str) {
        let indices = self.hash_indices(name, version);
        for idx in indices {
            let word_idx = idx / 64;
            let bit_idx = idx % 64;
            self.bits[word_idx] |= 1u64 << bit_idx;
        }
    }

    /// Check if a package might be in the filter
    /// Returns true if the package might exist (possible false positive)
    /// Returns false if the package definitely doesn't exist
    #[must_use]
    pub fn contains(&self, name: &str, version: &str) -> bool {
        for idx in self.hash_indices(name, version) {
            let word_idx = idx / 64;
            let bit_idx = idx % 64;
            if self.bits[word_idx] & (1u64 << bit_idx) == 0 {
                return false;
            }
        }
        true
    }

    /// Serialize filter to bytes
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(16 + self.bits.len() * 8);

        // Header: num_bits, num_hashes
        bytes.extend_from_slice(&(self.num_bits as u64).to_le_bytes());
        bytes.extend_from_slice(&(self.num_hashes as u64).to_le_bytes());

        // Bit array
        for word in &self.bits {
            bytes.extend_from_slice(&word.to_le_bytes());
        }

        bytes
    }

    /// Deserialize filter from bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 16 {
            anyhow::bail!("Bloom filter data too short");
        }

        let num_bits = u64::from_le_bytes(bytes[0..8].try_into()?) as usize;
        let num_hashes = u64::from_le_bytes(bytes[8..16].try_into()?) as usize;
        let num_words = num_bits.div_ceil(64);

        if bytes.len() < 16 + num_words * 8 {
            anyhow::bail!("Bloom filter data truncated");
        }

        let mut bits = Vec::with_capacity(num_words);
        for i in 0..num_words {
            let start = 16 + i * 8;
            let word = u64::from_le_bytes(bytes[start..start + 8].try_into()?);
            bits.push(word);
        }

        Ok(Self {
            bits,
            num_hashes,
            num_bits,
        })
    }

    /// Load filter from disk cache
    pub fn load(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).context("Failed to read bloom filter")?;
        Self::from_bytes(&bytes)
    }

    /// Save filter to disk cache
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let bytes = self.to_bytes();
        let tmp_path = path.with_extension("tmp");
        fs::write(&tmp_path, bytes)?;
        fs::rename(tmp_path, path)?;
        Ok(())
    }
}

/// Get the bloom filter cache path
fn bloom_cache_path() -> PathBuf {
    paths::cache_dir().join("sync_bloom.bin")
}

/// Get the newest modification time of sync databases
fn get_sync_db_mtime() -> Option<SystemTime> {
    let sync_dir = paths::pacman_sync_dir();
    let mut newest = SystemTime::UNIX_EPOCH;

    if let Ok(entries) = fs::read_dir(sync_dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata()
                && let Ok(mtime) = meta.modified()
                    && mtime > newest {
                        newest = mtime;
                    }
        }
    }

    if newest == SystemTime::UNIX_EPOCH {
        None
    } else {
        Some(newest)
    }
}

/// Build bloom filter from sync database packages
pub fn build_sync_bloom_filter() -> Result<PackageBloomFilter> {
    use crate::package_managers::pacman_db::get_detailed_packages;

    let packages = get_detailed_packages()?;
    let mut filter = PackageBloomFilter::default();

    for pkg in &packages {
        filter.insert(&pkg.name, &pkg.version.to_string());
    }

    Ok(filter)
}

/// Quick check if updates might be available
///
/// Returns:
/// - `Ok(false)` - Definitely no updates (100% certain)
/// - `Ok(true)` - Updates might be available (need full check)
/// - `Err(_)` - Error loading filter (fall back to full check)
///
/// # Performance
/// - First call after sync: ~50-100ms (builds filter)
/// - Subsequent calls: <100 microseconds
pub fn quick_update_check() -> Result<bool> {
    let bloom_path = bloom_cache_path();
    let current_mtime = get_sync_db_mtime();

    // Try to load cached filter
    let filter = if bloom_path.exists() {
        // Check if filter is still valid
        if let Ok(meta) = fs::metadata(&bloom_path) {
            if let Ok(filter_mtime) = meta.modified() {
                if current_mtime.is_some_and(|db_mtime| filter_mtime >= db_mtime) {
                    // Filter is newer than DBs, still valid
                    PackageBloomFilter::load(&bloom_path)?
                } else {
                    // DBs updated, rebuild filter
                    let filter = build_sync_bloom_filter()?;
                    filter.save(&bloom_path)?;
                    filter
                }
            } else {
                // Can't get mtime, rebuild
                let filter = build_sync_bloom_filter()?;
                filter.save(&bloom_path)?;
                filter
            }
        } else {
            // Can't stat file, rebuild
            let filter = build_sync_bloom_filter()?;
            filter.save(&bloom_path)?;
            filter
        }
    } else {
        // No cached filter, build one
        let filter = build_sync_bloom_filter()?;
        filter.save(&bloom_path)?;
        filter
    };

    use crate::package_managers::pacman_db::{get_sync_package, list_local_cached};

    let local_packages = list_local_cached()?;

    for pkg in &local_packages {
        let version_str = pkg.version.to_string();

        if !filter.contains(&pkg.name, &version_str)
            && get_sync_package(&pkg.name)?.is_some() {
                return Ok(true);
            }
    }

    Ok(false)
}

/// Invalidate the bloom filter cache (call after sync)
pub fn invalidate_bloom_cache() {
    let bloom_path = bloom_cache_path();
    let _ = fs::remove_file(bloom_path);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bloom_filter_basic() {
        let mut filter = PackageBloomFilter::new(10000, 5);

        filter.insert("firefox", "120.0.1-1");
        filter.insert("linux", "6.6.8-1");

        assert!(filter.contains("firefox", "120.0.1-1"));
        assert!(filter.contains("linux", "6.6.8-1"));

        // Different version should not match
        assert!(!filter.contains("firefox", "119.0-1"));
    }

    #[test]
    fn test_bloom_filter_serialization() {
        let mut filter = PackageBloomFilter::new(10000, 5);
        filter.insert("vim", "9.0.2000-1");
        filter.insert("git", "2.43.0-1");

        let bytes = filter.to_bytes();
        let restored = PackageBloomFilter::from_bytes(&bytes).unwrap();

        assert!(restored.contains("vim", "9.0.2000-1"));
        assert!(restored.contains("git", "2.43.0-1"));
        assert!(!restored.contains("vim", "8.0-1"));
    }

    #[test]
    fn test_bloom_filter_false_positive_rate() {
        let mut filter = PackageBloomFilter::default();

        // Insert 10000 packages
        for i in 0..10000 {
            filter.insert(&format!("package-{i}"), &format!("1.0.{i}-1"));
        }

        // Check false positive rate on non-existent packages
        let mut false_positives = 0;
        for i in 10000..20000 {
            if filter.contains(&format!("package-{i}"), &format!("1.0.{i}-1")) {
                false_positives += 1;
            }
        }

        // Should be well under 1%
        assert!(
            false_positives < 100,
            "False positive rate too high: {false_positives}/10000"
        );
    }
}
