use ahash::AHashMap;
use anyhow::Result;

use crate::daemon::db::PersistentCache;
use crate::daemon::protocol::{DetailedPackageInfo, PackageInfo};

// memchr::memmem provides SIMD-accelerated substring search
// for the description-matching hot path

/// String interner with O(1) lookup via packed (offset, length) encoding.
/// Each interned string is stored once in a contiguous byte buffer; the
/// returned handle packs both the byte offset and length into a single u64.
#[derive(Default)]
struct StringPool {
    pool: Vec<u8>,
    offsets: AHashMap<String, u64>,
}

/// Pack a 32-bit offset and 32-bit length into a single u64 handle.
const fn pack(offset: u32, len: u32) -> u64 {
    (offset as u64) | ((len as u64) << 32)
}

/// Unpack a handle back into (offset, length).
const fn unpack(handle: u64) -> (u32, u32) {
    (handle as u32, (handle >> 32) as u32)
}

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
    #[inline]
    fn get(&self, handle: u64) -> &str {
        let (offset, len) = unpack(handle);
        let start = offset as usize;
        let end = start + len as usize;
        debug_assert!(end <= self.pool.len(), "StringPool handle out of bounds");
        // SAFETY: see doc comment above
        unsafe { std::str::from_utf8_unchecked(&self.pool[start..end]) }
    }
}

pub struct PackageIndex {
    /// Internal compact representation
    items: Vec<CompactPackageInfo>,
    /// String pool for all metadata
    pool: StringPool,
    /// Maps package name to index in `items`
    name_to_idx: AHashMap<String, usize>,
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
        // Compare by rank first (higher is better)
        match self.rank.cmp(&other.rank) {
            std::cmp::Ordering::Equal => {
                // Then by name_len_rev (higher = shorter name = better)
                match self.name_len_rev.cmp(&other.name_len_rev) {
                    std::cmp::Ordering::Equal => {
                        // Finally by idx_rev (higher = earlier index = better)
                        self.idx_rev.cmp(&other.idx_rev)
                    }
                    other => other,
                }
            }
            other => other,
        }
    }
}

impl PackageIndex {
    pub fn new() -> Result<Self> {
        #[cfg(any(feature = "arch", feature = "debian", feature = "debian-pure"))]
        use crate::core::env::distro::{detect_distro, Distro};
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

        let mut pool = StringPool::default();
        let mut items = Vec::new();
        let mut name_to_idx = AHashMap::default();

        let db_packages = debian_db::get_detailed_packages()?;
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
        }

        Ok(Self {
            items,
            pool,
            name_to_idx,
        })
    }

    #[cfg(feature = "arch")]
    fn new_alpm() -> Result<Self> {
        use crate::package_managers::pacman_db;
        let mut pool = StringPool::default();
        let mut items = Vec::new();
        let mut name_to_idx = AHashMap::default();

        let db_packages = pacman_db::get_detailed_packages()?;
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
        }

        Ok(Self {
            items,
            pool,
            name_to_idx,
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

        // Capacity hint: ~4% match rate on typical repos
        let mut scored_matches: Vec<(RelevanceScore, usize)> =
            Vec::with_capacity(self.items.len() / 25);

        let mut name_match_count: usize = 0;

        for (idx, item) in self.items.iter().enumerate() {
            // Zero-allocation: pre-lowercased slices from the string pool
            let name_lower = self.pool.get(item.name_lower_offset);

            if let Some(name_score) = Self::score_name_match(&query_lower, name_lower, idx) {
                scored_matches.push((name_score, idx));
                name_match_count += 1;
            } else if name_match_count < limit {
                // Only scan descriptions while we still need more results;
                // description-only matches are lowest priority and cannot
                // displace name matches in the top-K output.
                let desc_lower = self.pool.get(item.description_lower_offset);
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
        scored_matches.sort_unstable_by(|a, b| b.0.cmp(&a.0));

        scored_matches
            .into_iter()
            .filter_map(|(_, idx)| {
                let item = self.items.get(idx)?;
                Some(PackageInfo {
                    name: self.pool.get(item.name_offset).to_string(),
                    version: self.pool.get(item.version_offset).to_string(),
                    description: self.pool.get(item.description_offset).to_string(),
                    source: self.pool.get(item.source_offset).to_string(),
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

        if found_substring {
            Some(RelevanceScore::new(
                RelevanceScore::SUBSTRING_MATCH,
                name_lower.len(),
                idx,
            ))
        } else {
            None
        }
    }

    pub fn get(&self, name: &str) -> Option<DetailedPackageInfo> {
        let &idx = self.name_to_idx.get(name)?;
        let item = &self.items[idx];

        Some(DetailedPackageInfo {
            name: self.pool.get(item.name_offset).to_string(),
            version: self.pool.get(item.version_offset).to_string(),
            description: self.pool.get(item.description_offset).to_string(),
            url: self.pool.get(item.url_offset).to_string(),
            size: item.size,
            download_size: item.download_size,
            repo: self.pool.get(item.repo_offset).to_string(),
            depends: Vec::new(),
            licenses: Vec::new(),
            source: self.pool.get(item.source_offset).to_string(),
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
        self.name_to_idx
            .keys()
            .filter(|name| name.to_ascii_lowercase().starts_with(&query_lower))
            .take(limit)
            .cloned()
            .collect()
    }

    pub fn all_packages(&self) -> Vec<DetailedPackageInfo> {
        self.items
            .iter()
            .map(|item| DetailedPackageInfo {
                name: self.pool.get(item.name_offset).to_string(),
                version: self.pool.get(item.version_offset).to_string(),
                description: self.pool.get(item.description_offset).to_string(),
                url: self.pool.get(item.url_offset).to_string(),
                size: item.size,
                download_size: item.download_size,
                repo: self.pool.get(item.repo_offset).to_string(),
                depends: Vec::new(),
                licenses: Vec::new(),
                source: self.pool.get(item.source_offset).to_string(),
            })
            .collect()
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
        assert_eq!(pool.get(off1), "hello");
        assert_eq!(pool.get(off2), "world");
    }

    #[test]
    fn test_string_pool_empty_and_special() {
        let mut pool = StringPool::default();
        let off_empty = pool.intern("");
        let off_space = pool.intern(" ");
        let off_unicode = pool.intern("🦀");

        assert_eq!(pool.get(off_empty), "");
        assert_eq!(pool.get(off_space), " ");
        assert_eq!(pool.get(off_unicode), "🦀");
    }

    #[test]
    fn test_string_pool_large() {
        let mut pool = StringPool::default();
        for i in 0..1000 {
            let s = format!("string-{i}");
            let off = pool.intern(&s);
            assert_eq!(pool.get(off), s);
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
