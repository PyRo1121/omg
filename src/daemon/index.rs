//! High-performance in-memory package index using Nucleo
//!
//! Provides sub-millisecond fuzzy search and instant lookups
//! by mirroring libalpm databases in memory.

use ahash::AHashMap;
use anyhow::{Context, Result};
use nucleo_matcher::{Config, Matcher, Utf32String};
use parking_lot::RwLock;
use rayon::prelude::*;

use crate::core::paths;
use crate::daemon::protocol::{DetailedPackageInfo, PackageInfo};

pub struct PackageIndex {
    /// Maps package name to detailed info (using ahash for speed)
    packages: AHashMap<String, DetailedPackageInfo>,
    /// Search items for Nucleo
    search_items: Vec<(String, Utf32String)>,
    /// Lowercased search strings for case-insensitive match
    search_items_lower: Vec<Utf32String>,
    /// Prefix index for 1-2 char fast path
    prefix_index: AHashMap<String, Vec<usize>>,
    /// Reader-writer lock for package lookups
    lock: RwLock<()>,
}

impl PackageIndex {
    pub fn new() -> Result<Self> {
        let mut packages = AHashMap::default();
        let mut search_items = Vec::new();
        let mut search_items_lower = Vec::new();
        let mut prefix_index: AHashMap<String, Vec<usize>> = AHashMap::new();

        // Initialize ALPM and read all databases
        let root = paths::pacman_root().to_string_lossy().into_owned();
        let db_path = paths::pacman_db_dir().to_string_lossy().into_owned();
        let alpm =
            alpm::Alpm::new(root, db_path).context("Failed to initialize ALPM for indexing")?;

        for db_name in ["core", "extra", "multilib"] {
            let db = alpm.register_syncdb(db_name, alpm::SigLevel::USE_DEFAULT)?;
            for pkg in db.pkgs() {
                let info = DetailedPackageInfo {
                    name: pkg.name().to_string(),
                    version: pkg.version().to_string(),
                    description: pkg.desc().unwrap_or("").to_string(),
                    url: pkg.url().unwrap_or("").to_string(),
                    size: pkg.isize() as u64,
                    download_size: pkg.size() as u64,
                    repo: db.name().to_string(),
                    depends: pkg
                        .depends()
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect(),
                    licenses: pkg
                        .licenses()
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect(),
                    source: "official".to_string(),
                };

                let search_str = format!("{} {}", info.name, info.description);
                let search_lower = search_str.to_lowercase();
                let idx = search_items.len();
                search_items.push((info.name.clone(), Utf32String::from(search_str.as_str())));
                search_items_lower.push(Utf32String::from(search_lower.as_str()));
                let name_lower = info.name.to_lowercase();
                for len in 1..=2 {
                    if name_lower.len() >= len {
                        let prefix = name_lower[..len].to_string();
                        prefix_index.entry(prefix).or_default().push(idx);
                    }
                }
                packages.insert(info.name.clone(), info);
            }
        }

        tracing::info!("Indexed {} packages from official repos", packages.len());

        Ok(Self {
            packages,
            search_items,
            search_items_lower,
            prefix_index,
            lock: RwLock::new(()),
        })
    }

    /// Fuzzy search for packages
    pub fn search(&self, query: &str, limit: usize) -> Vec<PackageInfo> {
        if query.is_empty() {
            return Vec::new();
        }
        let query_lower = query.to_lowercase();

        // FAST PASS: Prefix match for short queries (1-2 chars)
        // Users typing 'f' or 'fi' usually want things starting with those letters.
        if query_lower.len() < 3 {
            let matches: Vec<_> = self
                .prefix_index
                .get(&query_lower)
                .into_iter()
                .flatten()
                .take(limit)
                .filter_map(|idx| {
                    let name = &self.search_items[*idx].0;
                    self.packages.get(name).map(|p| PackageInfo {
                        name: p.name.clone(),
                        version: p.version.clone(),
                        description: p.description.clone(),
                        source: p.source.clone(),
                    })
                })
                .collect();

            if !matches.is_empty() {
                return matches;
            }
        }

        let query_utf32 = Utf32String::from(query_lower.as_str());
        let query_slice = query_utf32.slice(..);

        // 1. Parallel search using rayon (one matcher per thread)
        let mut matches: Vec<(u16, usize)> = self
            .search_items_lower
            .par_iter()
            .enumerate()
            .map_init(
                || Matcher::new(Config::DEFAULT),
                |matcher, (idx, search_str)| {
                    matcher
                        .fuzzy_match(query_slice, search_str.slice(..))
                        .map(|score| (score, idx))
                },
            )
            .flatten()
            .collect();

        // 2. Optimized sorting
        if matches.len() > limit {
            matches.select_nth_unstable_by(limit, |a, b| b.0.cmp(&a.0));
            matches.truncate(limit);
        }

        matches.sort_unstable_by(|a, b| b.0.cmp(&a.0));

        // 3. Map back to info (One clone per result only)
        // Pre-reserve capacity to avoid reallocations
        let mut results = Vec::with_capacity(matches.len());
        for (_, idx) in matches {
            let name = &self.search_items[idx].0;
            if let Some(p) = self.packages.get(name) {
                results.push(PackageInfo {
                    name: p.name.clone(),
                    version: p.version.clone(),
                    description: p.description.clone(),
                    source: p.source.clone(),
                });
            }
        }
        results
    }

    /// Get detailed package info by name (instant)
    pub fn get(&self, name: &str) -> Option<DetailedPackageInfo> {
        let _read_guard = self.lock.read();
        self.packages.get(name).cloned()
    }

    /// Total number of indexed packages
    pub fn len(&self) -> usize {
        self.packages.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }
}
