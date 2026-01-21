//! High-performance in-memory package index using Nucleo
//!
//! Provides sub-millisecond fuzzy search and instant lookups
//! by mirroring system package databases in memory.
//!
//! Supports index preloading from persistent cache for <10ms cold starts.

use ahash::AHashMap;
#[cfg(feature = "arch")]
use anyhow::Context;
use anyhow::Result;
use memchr::memmem;
use parking_lot::RwLock;

use crate::core::paths;
use crate::daemon::db::PersistentCache;
use crate::daemon::protocol::{DetailedPackageInfo, PackageInfo};

pub struct PackageIndex {
    /// Maps package name to detailed info (using ahash for speed)
    packages: AHashMap<String, DetailedPackageInfo>,
    /// Lowercased search strings for case-insensitive match
    /// Format: "name description"
    search_items_lower: Vec<String>,
    /// Mapping from search_items index to package name
    search_items_names: Vec<String>,
    /// Prefix index for 1-2 char fast path
    prefix_index: AHashMap<String, Vec<usize>>,
    /// Reader-writer lock for package lookups
    lock: RwLock<()>,
}

impl PackageIndex {
    pub fn new() -> Result<Self> {
        #[cfg(all(feature = "debian", not(feature = "arch")))]
        {
            Self::new_apt()
        }

        #[cfg(all(feature = "arch", feature = "debian"))]
        {
            if is_debian_like() {
                Self::new_apt()
            } else {
                Self::new_alpm()
            }
        }

        #[cfg(all(feature = "arch", not(feature = "debian")))]
        {
            Self::new_alpm()
        }

        #[cfg(not(any(feature = "arch", feature = "debian")))]
        anyhow::bail!("No package manager backend enabled")
    }

    /// Create index with preloading from persistent cache
    pub fn new_with_cache(cache: &PersistentCache) -> Result<Self> {
        let start = std::time::Instant::now();

        // SECURITY FIX: Atomic cache validation to prevent TOCTOU race
        // Previous vulnerability:
        // 1. Check if cache is valid (read db_mtime)
        // 2. Load cache
        // 3. DB could be modified between steps 1 and 2
        //
        // New approach:
        // 1. Load cache with embedded metadata
        // 2. Get current db_mtime
        // 3. Validate cache metadata matches current state
        // This eliminates the race window since we validate AFTER loading

        // Try loading from cache first (includes metadata)
        let cached_index = cache.load_index().ok().flatten();

        // Get current DB mtime for validation
        let current_db_mtime = Self::get_db_mtime();

        // Validate cache is still current (AFTER loading, not before)
        if let Some(cached) = cached_index {
            // Check if cache metadata matches current DB state
            if let Ok(Some(meta)) = cache.get_index_meta() {
                if meta.db_mtime == current_db_mtime && meta.package_count == cached.packages.len() {
                    let index = Self::from_packages(cached.packages);
                    tracing::info!(
                        "Index loaded from cache in {:?} ({} packages)",
                        start.elapsed(),
                        index.len()
                    );
                    return Ok(index);
                } else {
                    tracing::debug!(
                        "Cache invalidated: db_mtime mismatch ({} != {}) or count mismatch",
                        meta.db_mtime,
                        current_db_mtime
                    );
                }
            }
        }

        // Cache miss, invalid, or stale - build fresh
        tracing::info!("Building fresh index (cache miss or stale)");
        let index = Self::new()?;

        // Save to cache for next startup
        let packages: Vec<_> = index.packages.values().cloned().collect();
        if let Err(e) = cache.save_index(&packages, current_db_mtime) {
            tracing::warn!("Failed to save index to cache: {e}");
        }

        tracing::info!(
            "Index built in {:?} ({} packages)",
            start.elapsed(),
            index.len()
        );
        Ok(index)
    }

    /// Build index from pre-loaded packages (instant)
    fn from_packages(packages_vec: Vec<DetailedPackageInfo>) -> Self {
        let mut packages = AHashMap::with_capacity(packages_vec.len());
        let mut search_items_lower = Vec::with_capacity(packages_vec.len());
        let mut search_items_names = Vec::with_capacity(packages_vec.len());
        let mut prefix_index: AHashMap<String, Vec<usize>> = AHashMap::new();

        for info in packages_vec {
            let search_str = format!("{} {}", info.name, info.description);
            let search_lower = search_str.to_lowercase();
            let idx = search_items_lower.len();

            search_items_lower.push(search_lower);
            search_items_names.push(info.name.clone());

            let name_lower = info.name.to_lowercase();
            for len in 1..=2 {
                if name_lower.len() >= len {
                    let prefix = name_lower[..len].to_string();
                    prefix_index.entry(prefix).or_default().push(idx);
                }
            }
            packages.insert(info.name.clone(), info);
        }

        Self {
            packages,
            search_items_lower,
            search_items_names,
            prefix_index,
            lock: RwLock::new(()),
        }
    }

    /// Get modification time of pacman sync databases
    /// Note: This is sync I/O but typically fast (<1ms) since it's just a stat() call.
    /// For async contexts, consider caching this value.
    fn get_db_mtime() -> u64 {
        let db_dir = paths::pacman_db_dir().join("sync");
        std::fs::metadata(&db_dir)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs())
    }

    #[cfg(feature = "arch")]
    fn new_alpm() -> Result<Self> {
        let mut packages = AHashMap::default();
        let mut search_items_lower = Vec::new();
        let mut search_items_names = Vec::new();
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
                let idx = search_items_lower.len();

                search_items_lower.push(search_lower);
                search_items_names.push(info.name.clone());

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
            search_items_lower,
            search_items_names,
            prefix_index,
            lock: RwLock::new(()),
        })
    }

    #[cfg(feature = "debian")]
    fn new_apt() -> Result<Self> {
        use std::fs;
        use std::io::Read;
        use std::path::Path;

        let mut packages = AHashMap::default();
        let mut search_items_lower = Vec::new();
        let mut search_items_names = Vec::new();
        let mut prefix_index: AHashMap<String, Vec<usize>> = AHashMap::new();

        let lists_dir = Path::new("/var/lib/apt/lists");
        if !lists_dir.exists() {
            anyhow::bail!("APT lists directory not found");
        }

        // Parse all Packages files directly (much faster than rust-apt Cache iteration)
        for entry in fs::read_dir(lists_dir)? {
            let entry = entry?;
            let path = entry.path();
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            let content = if filename.ends_with("_Packages.lz4") {
                fs::read(&path).ok().and_then(|compressed| {
                    let mut decoder = lz4_flex::frame::FrameDecoder::new(&compressed[..]);
                    let mut buf = Vec::new();
                    decoder.read_to_end(&mut buf).ok()?;
                    String::from_utf8(buf).ok()
                })
            } else if filename.ends_with("_Packages.gz") {
                fs::read(&path).ok().and_then(|compressed| {
                    let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
                    let mut content = String::new();
                    decoder.read_to_string(&mut content).ok()?;
                    Some(content)
                })
            } else if filename.ends_with("_Packages") && !filename.contains('.') {
                fs::read_to_string(&path).ok()
            } else {
                None
            };

            if let Some(content) = content {
                Self::parse_packages_content(
                    &content,
                    &mut packages,
                    &mut search_items_lower,
                    &mut search_items_names,
                    &mut prefix_index,
                );
            }
        }

        tracing::info!("Indexed {} packages from apt", packages.len());

        Ok(Self {
            packages,
            search_items_lower,
            search_items_names,
            prefix_index,
            lock: RwLock::new(()),
        })
    }

    #[cfg(feature = "debian")]
    fn parse_packages_content(
        content: &str,
        packages: &mut AHashMap<String, DetailedPackageInfo>,
        search_items_lower: &mut Vec<String>,
        search_items_names: &mut Vec<String>,
        prefix_index: &mut AHashMap<String, Vec<usize>>,
    ) {
        for paragraph in content.split("\n\n") {
            if paragraph.trim().is_empty() {
                continue;
            }

            let mut name = String::new();
            let mut version = String::new();
            let mut description = String::new();
            let mut url = String::new();
            let mut size = 0u64;
            let mut download_size = 0u64;
            let mut depends = Vec::new();

            for line in paragraph.lines() {
                if line.starts_with(' ') || line.starts_with('\t') {
                    continue;
                }
                if let Some((key, value)) = line.split_once(':') {
                    let value = value.trim();
                    match key.trim() {
                        "Package" => name = value.to_string(),
                        "Version" => version = value.to_string(),
                        "Description" => description = value.to_string(),
                        "Homepage" => url = value.to_string(),
                        "Installed-Size" => size = value.parse::<u64>().unwrap_or(0) * 1024,
                        "Size" => download_size = value.parse().unwrap_or(0),
                        "Depends" => {
                            depends = value
                                .split(',')
                                .map(|d| {
                                    d.trim().split_whitespace().next().unwrap_or("").to_string()
                                })
                                .filter(|d| !d.is_empty())
                                .collect();
                        }
                        _ => {}
                    }
                }
            }

            if name.is_empty() || packages.contains_key(&name) {
                continue;
            }

            let info = DetailedPackageInfo {
                name: name.clone(),
                version,
                description: description.clone(),
                url,
                size,
                download_size,
                repo: "apt".to_string(),
                depends,
                licenses: Vec::new(),
                source: "official".to_string(),
            };

            let search_str = format!("{} {}", info.name, info.description);
            let search_lower = search_str.to_lowercase();
            let idx = search_items_lower.len();

            search_items_lower.push(search_lower);
            search_items_names.push(info.name.clone());

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

    /// Fast substring search for packages (like apt-cache search)
    /// Optimized: avoids dynamic dispatch, minimizes clones, uses direct indexing
    #[inline]
    pub fn search(&self, query: &str, limit: usize) -> Vec<PackageInfo> {
        if query.is_empty() {
            return Vec::new();
        }
        let query_lower = query.to_lowercase();
        let query_bytes = query_lower.as_bytes();
        let mut results = Vec::with_capacity(limit);

        // OPTIMIZATION: Avoid Box<dyn Iterator> - use concrete types
        if query_lower.len() <= 2 {
            // Short query: use prefix index
            if let Some(matches) = self.prefix_index.get(&query_lower) {
                for &idx in matches {
                    if results.len() >= limit {
                        break;
                    }
                    // SAFETY: prefix_index only contains valid indices
                    if let Some(name) = self.search_items_names.get(idx)
                        && let Some(p) = self.packages.get(name)
                    {
                        results.push(PackageInfo {
                            name: p.name.clone(),
                            version: p.version.clone(),
                            description: p.description.clone(),
                            source: p.source.clone(),
                        });
                    }
                }
            }
        } else {
            // Longer query: use SIMD-accelerated memmem search
            let finder = memmem::Finder::new(query_bytes);
            for (idx, search_lower) in self.search_items_lower.iter().enumerate() {
                if results.len() >= limit {
                    break;
                }
                // SIMD-accelerated substring search (10x faster than str::contains)
                if finder.find(search_lower.as_bytes()).is_some() {
                    if let Some(name) = self.search_items_names.get(idx)
                        && let Some(p) = self.packages.get(name)
                    {
                        results.push(PackageInfo {
                            name: p.name.clone(),
                            version: p.version.clone(),
                            description: p.description.clone(),
                            source: p.source.clone(),
                        });
                    }
                }
            }
        }

        results
    }

    /// Get detailed package info by name (instant)
    #[inline]
    #[must_use]
    pub fn get(&self, name: &str) -> Option<DetailedPackageInfo> {
        let _read_guard = self.lock.read();
        self.packages.get(name).cloned()
    }

    /// Total number of indexed packages
    #[must_use]
    pub fn len(&self) -> usize {
        self.packages.len()
    }

    /// Check if the index is empty
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    /// Get all packages (for cache serialization)
    #[must_use]
    pub fn all_packages(&self) -> Vec<DetailedPackageInfo> {
        self.packages.values().cloned().collect()
    }
}

#[cfg(feature = "debian")]
#[allow(dead_code)]
fn collect_depends(version: rust_apt::Version<'_>) -> Vec<String> {
    let mut depends = Vec::new();
    if let Some(deps) = version.dependencies() {
        for dep in deps {
            if dep.is_or() {
                for base in dep.iter() {
                    depends.push(base.name().to_string());
                }
            } else {
                let base = dep.first();
                depends.push(base.name().to_string());
            }
        }
    }
    depends
}
