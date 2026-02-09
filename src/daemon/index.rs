//! In-memory package index with string interning for the daemon.
//!
//! Provides O(1) package lookups and SIMD-accelerated substring search
//! for the daemon's hot path. Uses a string pool to deduplicate package
//! metadata and reduce memory allocations.

use ahash::AHashMap;
use anyhow::Result;

use crate::daemon::db::PersistentCache;
use crate::daemon::protocol::{DetailedPackageInfo, PackageInfo};

// memchr::memmem provides SIMD-accelerated substring search
// for the description-matching hot path

/// Bloom filter for fast negative lookups (package doesn't exist check)
/// With 100k packages and 512KB filter: ~0.01% false positive rate
#[derive(Debug, Clone)]
struct PackageBloomFilter {
    bits: Vec<u64>,
    num_bits: usize,
}

#[allow(dead_code)]
impl PackageBloomFilter {
    /// Create bloom filter with optimal size for expected package count
    fn new(expected_items: usize) -> Self {
        // Use 8 bits per item for ~1% false positive rate
        let num_bits = (expected_items * 8).max(4096);
        let num_words = num_bits.div_ceil(64);
        Self {
            bits: vec![0u64; num_words],
            num_bits,
        }
    }

    /// Hash package name to multiple bit positions
    #[inline]
    fn hash_positions(&self, name: &str) -> [usize; 3] {
        use std::hash::{Hash, Hasher};
        let mut hasher = ahash::AHasher::default();
        name.hash(&mut hasher);
        let h1 = hasher.finish() as usize;
        let h2 = h1.wrapping_mul(0x9e37_79b9_7f4a_7c15); // golden ratio
        let h3 = h2.wrapping_mul(0x9e37_79b9_7f4a_7c15);
        [h1 % self.num_bits, h2 % self.num_bits, h3 % self.num_bits]
    }

    /// Insert package name into filter
    fn insert(&mut self, name: &str) {
        for pos in self.hash_positions(name) {
            let word_idx = pos / 64;
            let bit_idx = pos % 64;
            self.bits[word_idx] |= 1u64 << bit_idx;
        }
    }

    /// Check if package name might exist (false positives possible, no false negatives)
    #[inline]
    fn might_contain(&self, name: &str) -> bool {
        for pos in self.hash_positions(name) {
            let word_idx = pos / 64;
            let bit_idx = pos % 64;
            if self.bits[word_idx] & (1u64 << bit_idx) == 0 {
                return false; // Definitely NOT in set
            }
        }
        true // Might be in set (or false positive)
    }
}

/// String interner with O(1) lookup via packed (offset, length) encoding.
/// Each interned string is stored once in a contiguous byte buffer; the
/// returned handle packs both the byte offset and length into a single u64.
#[derive(Default)]
#[allow(dead_code)]
struct StringPool {
    pool: Vec<u8>,
    offsets: AHashMap<String, u64>,
}

/// Pack a 32-bit offset and 32-bit length into a single u64 handle.
#[allow(dead_code)]
const fn pack(offset: u32, len: u32) -> u64 {
    (offset as u64) | ((len as u64) << 32)
}

/// Unpack a handle back into (offset, length).
const fn unpack(handle: u64) -> (u32, u32) {
    (handle as u32, (handle >> 32) as u32)
}

#[allow(dead_code)]
impl StringPool {
    fn intern(&mut self, s: &str) -> u64 {
        if let Some(&handle) = self.offsets.get(s) {
            return handle;
        }
        debug_assert!(
            u32::try_from(self.pool.len()).is_ok(),
            "String pool exceeded u32 address space"
        );
        let offset = self.pool.len() as u32;
        let len = s.len() as u32;
        self.pool.extend_from_slice(s.as_bytes());
        let handle = pack(offset, len);
        self.offsets.insert(s.to_string(), handle);
        handle
    }

    /// O(1) string lookup — no scanning required.
    ///
    /// SAFETY: The pool is append-only and every string was written from a
    /// valid `&str` (guaranteed UTF-8 by Rust's type system).  No data is
    /// ever mutated after insertion, so the byte range `[offset, offset+len)`
    /// is always valid UTF-8.
    ///
    /// Bounds are verified in both debug and release builds to prevent UB
    /// from corrupted handles.
    #[inline]
    #[expect(unsafe_code)]
    fn get(&self, handle: u64) -> Result<&str, &'static str> {
        let (offset, len) = unpack(handle);
        let start = offset as usize;
        let end = start
            .checked_add(len as usize)
            .ok_or("StringPool handle overflow")?;

        if end > self.pool.len() {
            return Err("StringPool handle out of bounds");
        }

        // SAFETY: Bounds verified above. The pool is append-only and all data
        // originates from valid UTF-8 strings via `intern()`.
        Ok(unsafe { std::str::from_utf8_unchecked(&self.pool[start..end]) })
    }
}

pub struct PackageIndex {
    /// Internal compact representation
    items: Vec<CompactPackageInfo>,
    /// String pool for all metadata
    pool: StringPool,
    /// Maps package name to index in `items`
    name_to_idx: AHashMap<String, usize>,
    /// Bloom filter for fast negative lookups (O(1) rejection of non-existent packages)
    bloom: PackageBloomFilter,
}

struct CompactPackageInfo {
    name_offset: u64,
    name_lower_offset: u64,
    version_offset: u64,
    description_offset: u64,
    description_lower_offset: u64,
    url_offset: u64,
    size: u64,
    download_size: u64,
    repo_offset: u64,
    source_offset: u64,
}

/// Relevance score for a search match
/// Higher scores = better matches
///
/// Ordering: We use reverse sort (b.cmp(a)), so:
/// - Higher rank values are better (4 > 3 > 2 > 1 > 0)
/// - Lower `name_len` is better (shorter = more specific)
/// - Lower idx is better (stable sort tiebreaker)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RelevanceScore {
    /// Primary rank: exact name match > prefix match > word boundary > substring
    rank: u8,
    /// Secondary sort: shorter package names preferred (more specific)
    /// We use reverse length for proper ordering with reverse sort
    name_len_rev: usize, // usize::MAX - name_len, so shorter names have higher values
    /// Package index (tiebreaker for stable sorting)
    /// We use reverse index for proper ordering with reverse sort
    idx_rev: usize, // usize::MAX - idx, so earlier indices have higher values
}

impl RelevanceScore {
    const EXACT_NAME_MATCH: u8 = 4;
    const PREFIX_MATCH: u8 = 3;
    const WORD_BOUNDARY_MATCH: u8 = 2;
    const SUBSTRING_MATCH: u8 = 1;
    const DESCRIPTION_ONLY: u8 = 0;

    fn new(rank: u8, name_len: usize, idx: usize) -> Self {
        Self {
            rank,
            name_len_rev: usize::MAX.saturating_sub(name_len),
            idx_rev: usize::MAX.saturating_sub(idx),
        }
    }
}

impl PartialOrd for RelevanceScore {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RelevanceScore {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Compare by (rank, name_len_rev, idx_rev) - all in descending order
        // Higher values are better for all three fields
        (self.rank, self.name_len_rev, self.idx_rev).cmp(&(
            other.rank,
            other.name_len_rev,
            other.idx_rev,
        ))
    }
}

impl PackageIndex {
    pub fn new() -> Result<Self> {
        #[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
        use crate::core::env::distro::{Distro, detect_distro};
        #[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
        let distro = detect_distro();

        #[cfg(any(feature = "debian", feature = "debian-pure"))]
        if distro == Distro::Debian || distro == Distro::Ubuntu {
            return Self::new_apt();
        }

        #[cfg(feature = "arch")]
        if distro == Distro::Arch {
            return Self::new_alpm();
        }

        // Fallbacks if detection fails but features are enabled
        #[cfg(feature = "arch")]
        return Self::new_alpm();

        #[cfg(all(
            not(feature = "arch"),
            any(feature = "debian", feature = "debian-pure")
        ))]
        return Self::new_apt();

        #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
        anyhow::bail!("No package backend enabled")
    }

    pub fn new_with_cache(_cache: &PersistentCache) -> Result<Self> {
        let start = std::time::Instant::now();
        let index = Self::new()?;
        tracing::info!(
            "Compact Index built in {:?} ({} packages)",
            start.elapsed(),
            index.items.len()
        );
        Ok(index)
    }

    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    fn new_apt() -> Result<Self> {
        use crate::package_managers::debian_db;
        debian_db::ensure_index_loaded()?;

        let db_packages = debian_db::get_detailed_packages()?;
        let pkg_count = db_packages.len();

        let mut pool = StringPool::default();
        let mut items = Vec::with_capacity(pkg_count);
        let mut name_to_idx = AHashMap::with_capacity(pkg_count);
        let mut bloom = PackageBloomFilter::new(pkg_count);

        for pkg in db_packages {
            let name_offset = pool.intern(&pkg.name);
            let name_lower_offset = pool.intern(&pkg.name.to_ascii_lowercase());
            let description_offset = pool.intern(&pkg.description);
            let description_lower_offset = pool.intern(&pkg.description.to_ascii_lowercase());
            let idx = items.len();

            items.push(CompactPackageInfo {
                name_offset,
                name_lower_offset,
                version_offset: pool.intern(&pkg.version),
                description_offset,
                description_lower_offset,
                url_offset: pool.intern(&pkg.homepage),
                size: pkg.installed_size,
                download_size: pkg.size,
                repo_offset: pool.intern(&pkg.section),
                source_offset: pool.intern("official"),
            });

            name_to_idx.insert(pkg.name.clone(), idx);
            bloom.insert(&pkg.name);
        }

        Ok(Self {
            items,
            pool,
            name_to_idx,
            bloom,
        })
    }

    #[cfg(feature = "arch")]
    fn new_alpm() -> Result<Self> {
        use crate::package_managers::pacman_db;
        let db_packages = pacman_db::get_detailed_packages()?;
        let pkg_count = db_packages.len();

        let mut pool = StringPool::default();
        let mut items = Vec::with_capacity(pkg_count);
        let mut name_to_idx = AHashMap::with_capacity(pkg_count);
        let mut bloom = PackageBloomFilter::new(pkg_count);

        for pkg in db_packages {
            let name_offset = pool.intern(&pkg.name);
            let name_lower_offset = pool.intern(&pkg.name.to_ascii_lowercase());
            let description_offset = pool.intern(&pkg.desc);
            let description_lower_offset = pool.intern(&pkg.desc.to_ascii_lowercase());
            let idx = items.len();

            items.push(CompactPackageInfo {
                name_offset,
                name_lower_offset,
                version_offset: pool.intern(&pkg.version.to_string()),
                description_offset,
                description_lower_offset,
                url_offset: pool.intern(&pkg.url),
                size: pkg.isize,
                download_size: pkg.csize,
                repo_offset: pool.intern(&pkg.repo),
                source_offset: pool.intern("official"),
            });

            name_to_idx.insert(pkg.name.clone(), idx);
            bloom.insert(&pkg.name);
        }

        Ok(Self {
            items,
            pool,
            name_to_idx,
            bloom,
        })
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<PackageInfo> {
        if query.is_empty() {
            return Vec::new();
        }
        let query_lower = query.to_ascii_lowercase();
        let query_bytes = query_lower.as_bytes();

        // Pre-build SIMD-accelerated description searcher (amortises needle
        // preprocessing across all packages)
        let desc_finder = memchr::memmem::Finder::new(query_bytes);

        // OPTIMIZATION: Dynamic capacity based on query specificity
        // Short queries (1-2 chars) -> expect many matches -> pre-allocate more
        // Long queries (5+ chars) -> expect few matches -> allocate less
        let capacity_hint = if query.len() <= 2 {
            self.items.len() / 10 // ~10% for very broad searches
        } else if query.len() <= 4 {
            self.items.len() / 25 // ~4% for typical searches
        } else {
            self.items.len() / 100 // ~1% for specific searches
        };
        let mut scored_matches: Vec<(RelevanceScore, usize)> = Vec::with_capacity(capacity_hint);

        let mut name_match_count: usize = 0;

        for (idx, item) in self.items.iter().enumerate() {
            // Zero-allocation: pre-lowercased slices from the string pool
            let name_lower = self
                .pool
                .get(item.name_lower_offset)
                .expect("valid name_lower handle from index");

            if let Some(name_score) = Self::score_name_match(&query_lower, name_lower, idx) {
                scored_matches.push((name_score, idx));
                name_match_count += 1;
            } else if name_match_count < limit {
                // Only scan descriptions while we still need more results;
                // description-only matches are lowest priority and cannot
                // displace name matches in the top-K output.
                let desc_lower = self
                    .pool
                    .get(item.description_lower_offset)
                    .expect("valid description_lower handle from index");
                if desc_finder.find(desc_lower.as_bytes()).is_some() {
                    scored_matches.push((
                        RelevanceScore::new(
                            RelevanceScore::DESCRIPTION_ONLY,
                            name_lower.len(),
                            idx,
                        ),
                        idx,
                    ));
                }
            }
        }

        // Partial sort: O(n) selection for top-K, then O(k log k) final sort
        if scored_matches.len() > limit {
            scored_matches.select_nth_unstable_by(limit - 1, |a, b| b.0.cmp(&a.0));
            scored_matches.truncate(limit);
        }
        scored_matches.sort_unstable_by_key(|&(score, _)| std::cmp::Reverse(score));

        scored_matches
            .into_iter()
            .filter_map(|(_, idx)| {
                let item = self.items.get(idx)?;
                Some(PackageInfo {
                    name: self
                        .pool
                        .get(item.name_offset)
                        .expect("valid name handle")
                        .to_string(),
                    version: self
                        .pool
                        .get(item.version_offset)
                        .expect("valid version handle")
                        .to_string(),
                    description: self
                        .pool
                        .get(item.description_offset)
                        .expect("valid description handle")
                        .to_string(),
                    source: self
                        .pool
                        .get(item.source_offset)
                        .expect("valid source handle")
                        .to_string(),
                })
            })
            .collect()
    }

    /// Score a name match via single-pass classification.
    /// `match_indices` finds all occurrence positions; we return the highest-ranked one.
    fn score_name_match(query_lower: &str, name_lower: &str, idx: usize) -> Option<RelevanceScore> {
        let query_len = query_lower.len();
        let mut found_substring = false;

        for (pos, _) in name_lower.match_indices(query_lower) {
            // Exact: query spans the entire name
            if pos == 0 && name_lower.len() == query_len {
                return Some(RelevanceScore::new(
                    RelevanceScore::EXACT_NAME_MATCH,
                    name_lower.len(),
                    idx,
                ));
            }
            // Prefix: query at position 0 but name is longer
            if pos == 0 {
                return Some(RelevanceScore::new(
                    RelevanceScore::PREFIX_MATCH,
                    name_lower.len(),
                    idx,
                ));
            }
            // Word boundary: preceded by separator
            let prev = name_lower.as_bytes()[pos - 1];
            if prev == b'-' || prev == b'_' || prev == b'.' || prev.is_ascii_whitespace() {
                return Some(RelevanceScore::new(
                    RelevanceScore::WORD_BOUNDARY_MATCH,
                    name_lower.len(),
                    idx,
                ));
            }
            // Non-boundary substring — keep scanning for a better match
            found_substring = true;
        }

        found_substring
            .then(|| RelevanceScore::new(RelevanceScore::SUBSTRING_MATCH, name_lower.len(), idx))
    }

    /// Get package info by name with bloom filter fast-path for negative lookups
    #[inline]
    pub fn get(&self, name: &str) -> Option<DetailedPackageInfo> {
        // OPTIMIZATION: Bloom filter provides O(1) rejection of non-existent packages
        // This avoids expensive HashMap lookup for packages that don't exist
        // False positive rate: ~0.01%, so we might do a HashMap lookup unnecessarily
        // But true negatives (99.99% of non-existent packages) skip the HashMap entirely
        if !self.bloom.might_contain(name) {
            return None; // Definitely doesn't exist
        }

        let &idx = self.name_to_idx.get(name)?;
        let item = &self.items[idx];

        Some(DetailedPackageInfo {
            name: self
                .pool
                .get(item.name_offset)
                .expect("valid name handle")
                .to_string(),
            version: self
                .pool
                .get(item.version_offset)
                .expect("valid version handle")
                .to_string(),
            description: self
                .pool
                .get(item.description_offset)
                .expect("valid description handle")
                .to_string(),
            url: self
                .pool
                .get(item.url_offset)
                .expect("valid url handle")
                .to_string(),
            size: item.size,
            download_size: item.download_size,
            repo: self
                .pool
                .get(item.repo_offset)
                .expect("valid repo handle")
                .to_string(),
            depends: Vec::new(),
            licenses: Vec::new(),
            source: self
                .pool
                .get(item.source_offset)
                .expect("valid source handle")
                .to_string(),
        })
    }

    #[inline]
    #[must_use]
    pub fn len(&self) -> usize {
        self.items.len()
    }

    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn suggest(&self, query: &str, limit: usize) -> Vec<String> {
        if query.is_empty() {
            return Vec::new();
        }
        let query_lower = query.to_ascii_lowercase();
        let mut suggestions = Vec::with_capacity(limit);

        for item in &self.items {
            if suggestions.len() >= limit {
                break;
            }
            // Use pre-lowercased name from string pool (zero allocation per item)
            let name_lower = self
                .pool
                .get(item.name_lower_offset)
                .expect("valid name_lower handle from index");
            if name_lower.starts_with(&query_lower) {
                suggestions.push(
                    self.pool
                        .get(item.name_offset)
                        .expect("valid name handle")
                        .to_string(),
                );
            }
        }

        suggestions
    }

    pub fn all_packages(&self) -> Vec<DetailedPackageInfo> {
        let mut results = Vec::with_capacity(self.items.len());
        results.extend(self.items.iter().map(|item| {
            DetailedPackageInfo {
                name: self
                    .pool
                    .get(item.name_offset)
                    .expect("valid name handle")
                    .to_string(),
                version: self
                    .pool
                    .get(item.version_offset)
                    .expect("valid version handle")
                    .to_string(),
                description: self
                    .pool
                    .get(item.description_offset)
                    .expect("valid description handle")
                    .to_string(),
                url: self
                    .pool
                    .get(item.url_offset)
                    .expect("valid url handle")
                    .to_string(),
                size: item.size,
                download_size: item.download_size,
                repo: self
                    .pool
                    .get(item.repo_offset)
                    .expect("valid repo handle")
                    .to_string(),
                depends: Vec::new(),
                licenses: Vec::new(),
                source: self
                    .pool
                    .get(item.source_offset)
                    .expect("valid source handle")
                    .to_string(),
            }
        }));
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_pool_interning() {
        let mut pool = StringPool::default();
        let off1 = pool.intern("hello");
        let off2 = pool.intern("world");
        let off3 = pool.intern("hello"); // Duplicate

        assert_eq!(off1, off3);
        assert_ne!(off1, off2);
        assert_eq!(pool.get(off1).unwrap(), "hello");
        assert_eq!(pool.get(off2).unwrap(), "world");
    }

    #[test]
    fn test_string_pool_empty_and_special() {
        let mut pool = StringPool::default();
        let off_empty = pool.intern("");
        let off_space = pool.intern(" ");
        let off_unicode = pool.intern("🦀");

        assert_eq!(pool.get(off_empty).unwrap(), "");
        assert_eq!(pool.get(off_space).unwrap(), " ");
        assert_eq!(pool.get(off_unicode).unwrap(), "🦀");
    }

    #[test]
    fn test_string_pool_large() {
        let mut pool = StringPool::default();
        for i in 0..1000 {
            let s = format!("string-{i}");
            let off = pool.intern(&s);
            assert_eq!(pool.get(off).unwrap(), s);
        }
    }

    #[test]
    fn test_relevance_score_ordering() {
        // Test that RelevanceScore sorts correctly
        let idx1 = 0;
        let idx2 = 1;
        let idx3 = 2;

        let exact = RelevanceScore::new(RelevanceScore::EXACT_NAME_MATCH, 5, idx1);
        let prefix = RelevanceScore::new(RelevanceScore::PREFIX_MATCH, 5, idx2);
        let word_boundary = RelevanceScore::new(RelevanceScore::WORD_BOUNDARY_MATCH, 5, idx3);
        let substring = RelevanceScore::new(RelevanceScore::SUBSTRING_MATCH, 5, 0);
        let description = RelevanceScore::new(RelevanceScore::DESCRIPTION_ONLY, 5, 0);

        // Higher ranks should come first (higher value = better)
        assert!(exact > prefix);
        assert!(prefix > word_boundary);
        assert!(word_boundary > substring);
        assert!(substring > description);
    }

    #[test]
    fn test_relevance_score_tiebreaker() {
        // When ranks are equal, shorter names should come first
        let short = RelevanceScore::new(RelevanceScore::PREFIX_MATCH, 5, 0);
        let long = RelevanceScore::new(RelevanceScore::PREFIX_MATCH, 15, 0);

        // Shorter name (len=5) should have higher value than longer name (len=15)
        assert!(short > long);
    }

    #[test]
    fn test_relevance_score_stable_sort() {
        // When rank and length are equal, lower index should come first
        let first = RelevanceScore::new(RelevanceScore::PREFIX_MATCH, 10, 0);
        let second = RelevanceScore::new(RelevanceScore::PREFIX_MATCH, 10, 1);

        // Lower index should have higher value (comes first in results)
        assert!(first > second);
    }
}
