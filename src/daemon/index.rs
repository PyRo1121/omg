use ahash::AHashMap;
use anyhow::Result;
use parking_lot::RwLock;

use crate::daemon::db::PersistentCache;
use crate::daemon::protocol::{DetailedPackageInfo, PackageInfo};

/// A highly-optimized string interner for reducing memory footprint
#[derive(Default)]
#[allow(dead_code)]
struct StringPool {
    pool: Vec<u8>,
    offsets: AHashMap<String, u32>,
}

impl StringPool {
    #[allow(dead_code)]
    fn intern(&mut self, s: &str) -> u32 {
        if let Some(&offset) = self.offsets.get(s) {
            return offset;
        }
        let offset = self.pool.len() as u32;
        self.pool.extend_from_slice(s.as_bytes());
        self.pool.push(0); // Null terminator
        self.offsets.insert(s.to_string(), offset);
        offset
    }

    fn get(&self, offset: u32) -> &str {
        let start = offset as usize;
        let mut end = start;
        while end < self.pool.len() && self.pool[end] != 0 {
            end += 1;
        }
        std::str::from_utf8(&self.pool[start..end]).unwrap_or("")
    }
}

pub struct PackageIndex {
    /// Internal compact representation
    items: Vec<CompactPackageInfo>,
    /// String pool for all metadata
    pool: StringPool,
    /// Maps package name to index in `items`
    name_to_idx: AHashMap<String, usize>,
    /// Contiguous lowercased search text for all packages (reserved for future use)
    #[allow(dead_code)]
    search_buffer: Vec<u8>,
    /// Starting offset of each package in `search_buffer` (reserved for future use)
    #[allow(dead_code)]
    package_offsets: Vec<u32>,
    /// Reader-writer lock for package lookups
    lock: RwLock<()>,
}

struct CompactPackageInfo {
    name_offset: u32,
    version_offset: u32,
    description_offset: u32,
    url_offset: u32,
    size: u64,
    download_size: u64,
    repo_offset: u32,
    source_offset: u32,
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

        let mut pool = StringPool::default();
        let mut items = Vec::new();
        let mut name_to_idx = AHashMap::default();
        let mut search_buffer = Vec::new();
        let mut package_offsets = Vec::new();

        let db_packages = debian_db::get_detailed_packages()?;
        for pkg in db_packages {
            let name_offset = pool.intern(&pkg.name);
            let idx = items.len();

            items.push(CompactPackageInfo {
                name_offset,
                version_offset: pool.intern(&pkg.version),
                description_offset: pool.intern(&pkg.description),
                url_offset: pool.intern(&pkg.homepage),
                size: pkg.installed_size,
                download_size: pkg.size,
                repo_offset: pool.intern(&pkg.section),
                source_offset: pool.intern("official"),
            });

            package_offsets.push(search_buffer.len() as u32);
            let search_str = format!("{} {}", pkg.name, pkg.description).to_ascii_lowercase();
            search_buffer.extend_from_slice(search_str.as_bytes());
            search_buffer.push(0);

            name_to_idx.insert(pkg.name.clone(), idx);
        }
        package_offsets.push(search_buffer.len() as u32);

        Ok(Self {
            items,
            pool,
            name_to_idx,
            search_buffer,
            package_offsets,
            lock: RwLock::new(()),
        })
    }

    #[cfg(feature = "arch")]
    fn new_alpm() -> Result<Self> {
        use crate::package_managers::pacman_db;
        let mut pool = StringPool::default();
        let mut items = Vec::new();
        let mut name_to_idx = AHashMap::default();
        let mut search_buffer = Vec::new();
        let mut package_offsets = Vec::new();

        let db_packages = pacman_db::get_detailed_packages()?;
        for pkg in db_packages {
            let name_offset = pool.intern(&pkg.name);
            let idx = items.len();

            items.push(CompactPackageInfo {
                name_offset,
                version_offset: pool.intern(&pkg.version.to_string()),
                description_offset: pool.intern(&pkg.desc),
                url_offset: pool.intern(&pkg.url),
                size: pkg.isize,
                download_size: pkg.csize,
                repo_offset: pool.intern(&pkg.repo),
                source_offset: pool.intern("official"),
            });

            package_offsets.push(search_buffer.len() as u32);
            let search_str = format!("{} {}", pkg.name, pkg.desc).to_ascii_lowercase();
            search_buffer.extend_from_slice(search_str.as_bytes());
            search_buffer.push(0);

            name_to_idx.insert(pkg.name.clone(), idx);
        }
        package_offsets.push(search_buffer.len() as u32);

        Ok(Self {
            items,
            pool,
            name_to_idx,
            search_buffer,
            package_offsets,
            lock: RwLock::new(()),
        })
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<PackageInfo> {
        if query.is_empty() {
            return Vec::new();
        }
        let query_lower = query.to_ascii_lowercase();

        // Collect all matches with relevance scores
        let mut scored_matches: Vec<(RelevanceScore, usize)> = Vec::new();

        // Check each package for matches
        for (idx, item) in self.items.iter().enumerate() {
            let name = self.pool.get(item.name_offset);
            let description = self.pool.get(item.description_offset);
            let name_lower = name.to_ascii_lowercase();

            // Score this package
            let score =
                if let Some(name_score) = Self::score_name_match(&query_lower, &name_lower, idx) {
                    name_score
                } else if description.to_ascii_lowercase().contains(&query_lower) {
                    // Match in description only (lowest priority)
                    RelevanceScore::new(RelevanceScore::DESCRIPTION_ONLY, name.len(), idx)
                } else {
                    continue; // No match at all
                };

            scored_matches.push((score, idx));
        }

        // Sort by relevance (higher scores first)
        scored_matches.sort_by(|a, b| b.0.cmp(&a.0));

        // Convert to PackageInfo, respecting limit
        scored_matches
            .into_iter()
            .take(limit)
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

    /// Score a name match, returning Some(score) if there's a match, None otherwise
    fn score_name_match(query_lower: &str, name_lower: &str, idx: usize) -> Option<RelevanceScore> {
        // 1. Exact match (highest priority)
        if query_lower == name_lower {
            return Some(RelevanceScore::new(
                RelevanceScore::EXACT_NAME_MATCH,
                name_lower.len(),
                idx,
            ));
        }

        // 2. Prefix match (e.g., "brave" matches "brave-browser")
        if name_lower.starts_with(query_lower) {
            return Some(RelevanceScore::new(
                RelevanceScore::PREFIX_MATCH,
                name_lower.len(),
                idx,
            ));
        }

        // 3. Word boundary match (e.g., "brave" matches "brave-browser" or "my-brave-app")
        // Check if query appears at start of name or after a separator (-, _, ., space)
        for (pos, _) in name_lower.match_indices(query_lower) {
            // At word boundary if at position 0 or preceded by separator
            if pos == 0
                || name_lower.as_bytes()[pos - 1].is_ascii_whitespace()
                || name_lower.as_bytes()[pos - 1] == b'-'
                || name_lower.as_bytes()[pos - 1] == b'_'
                || name_lower.as_bytes()[pos - 1] == b'.'
            {
                return Some(RelevanceScore::new(
                    RelevanceScore::WORD_BOUNDARY_MATCH,
                    name_lower.len(),
                    idx,
                ));
            }
        }

        // 4. Simple substring match (e.g., "rav" matches "brave")
        if name_lower.contains(query_lower) {
            return Some(RelevanceScore::new(
                RelevanceScore::SUBSTRING_MATCH,
                name_lower.len(),
                idx,
            ));
        }

        None
    }

    pub fn get(&self, name: &str) -> Option<DetailedPackageInfo> {
        let _read_guard = self.lock.read();
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

    pub fn len(&self) -> usize {
        self.items.len()
    }
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn suggest(&self, query: &str, limit: usize) -> Vec<String> {
        if query.is_empty() {
            return Vec::new();
        }
        let query_lower = query.to_lowercase();
        self.name_to_idx
            .keys()
            .filter(|name| name.to_lowercase().starts_with(&query_lower))
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
