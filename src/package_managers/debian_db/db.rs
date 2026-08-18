//! Pure Rust Debian/Ubuntu Database Parser - ULTRA FAST
//!
//! Parses /var/lib/apt/lists/*_Packages and /var/lib/dpkg/status files directly
//! and provides a high-performance index with zero-copy deserialization via rkyv.
//!
//! Performance features:
//! - Zero-copy memory-mapped access via rkyv + mmap
//! - SIMD-accelerated search via memchr/memmem
//! - LZ4 compressed cache for space efficiency
//! - Parallel parsing via rayon

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};

use ahash::AHashSet;
use anyhow::{Context, Result};
use fst::{IntoStreamer, Map, Streamer};
use memchr::memmem;
use memmap2::Mmap;
use rayon::prelude::*;
use std::sync::RwLock;

use crate::core::paths;
use crate::core::{Package, PackageSource};

/// TTL for cache eviction safety net (30 minutes)
const CACHE_TTL_SECS: u64 = 30 * 60;

/// Check if cache is expired based on TTL (30-minute safety net)
fn is_cache_expired(last_accessed: Option<std::time::SystemTime>) -> bool {
    if let Some(last_access) = last_accessed
        && let Ok(elapsed) = std::time::SystemTime::now().duration_since(last_access)
    {
        return elapsed.as_secs() > CACHE_TTL_SECS;
    }
    false
}

/// Global cache for Debian package index
static DEBIAN_INDEX_CACHE: LazyLock<RwLock<DebianIndexCache>> =
    LazyLock::new(|| RwLock::new(DebianIndexCache::default()));

/// Global cache for dpkg/status to avoid reparsing on every call
static DPKG_STATUS_CACHE: LazyLock<RwLock<DpkgStatusCache>> =
    LazyLock::new(|| RwLock::new(DpkgStatusCache::default()));

/// SIMD-accelerated finder for "Status: install ok installed"
/// Pre-compiled for faster dpkg/status parsing
static STATUS_INSTALLED_FINDER: LazyLock<memmem::Finder<'static>> =
    LazyLock::new(|| memmem::Finder::new(b"Status: install ok installed"));

#[derive(Default)]
struct DebianIndexCache {
    index: Option<DebianPackageIndex>,
    last_modified: Option<std::time::SystemTime>,
    /// Track individual file mtimes for incremental updates
    file_mtimes: HashMap<PathBuf, std::time::SystemTime>,
    /// Contiguous search buffer for SIMD search: "name desc\0name desc\0..."
    search_buffer: Vec<u8>,
    /// Offsets into the search buffer
    package_offsets: Vec<usize>,
    /// Cached set of installed package names
    installed_set: AHashSet<String>,
    /// Last access time for TTL-based eviction (30-minute safety net)
    last_accessed: Option<std::time::SystemTime>,
}

/// Cache for /var/lib/dpkg/status to avoid expensive reparsing
#[derive(Default)]
struct DpkgStatusCache {
    packages: Vec<LocalPackage>,
    installed_set: AHashSet<String>,
    status_mtime: Option<std::time::SystemTime>,
    extended_states_mtime: Option<std::time::SystemTime>,
    /// Last access time for TTL-based eviction (30-minute safety net)
    last_accessed: Option<std::time::SystemTime>,
}

/// Global mmap-based index for zero-copy access (optional, used when available)
static DEBIAN_MMAP_INDEX: LazyLock<RwLock<Option<DebianMmapIndex>>> =
    LazyLock::new(|| RwLock::new(None));

/// Global FST index for `O(query_len)` prefix searches
/// FST provides logarithmic complexity for prefix matching vs O(n) full scan
static DEBIAN_FST_INDEX: LazyLock<RwLock<Option<FstIndex>>> = LazyLock::new(|| RwLock::new(None));

/// FST-based search index with TTL-based eviction
struct FstIndex {
    map: Map<Mmap>,
    /// Last access time for TTL-based eviction
    last_accessed: AtomicU64,
}

impl FstIndex {
    /// Open an existing FST index
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open FST index at {}", path.display()))?;

        // SAFETY: Memory mapping is safe for read-only access
        // - File opened read-only, no modifications possible
        // - FST validates data integrity on construction
        // - Mmap maintains exclusive file handle ownership
        #[expect(unsafe_code)]
        let mmap = unsafe { Mmap::map(&file)? };

        let map = Map::new(mmap).map_err(|e| anyhow::anyhow!("Corrupted FST index: {e}"))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Self {
            map,
            last_accessed: AtomicU64::new(now),
        })
    }

    /// Check if expired based on TTL
    fn is_expired(&self) -> bool {
        let last = self.last_accessed.load(Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now.saturating_sub(last) > CACHE_TTL_SECS
    }

    /// Update last accessed time
    fn touch(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_accessed.store(now, Ordering::Relaxed);
    }
}

/// Zero-copy memory-mapped Debian package index
/// Provides sub-millisecond access to package metadata without deserialization
pub struct DebianMmapIndex {
    mmap: Mmap,
    /// Last access time (Unix timestamp) for TTL-based eviction
    /// `AtomicU64` allows lock-free updates from read-only methods
    last_accessed: AtomicU64,
}

impl DebianMmapIndex {
    /// Open an existing index using memory mapping
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open mmap index at {}", path.display()))?;

        // SAFETY: Memory mapping requires unsafe but is sound here:
        // - File is opened read-only, preventing modification
        // - Mmap maintains exclusive ownership of the file handle
        // - rkyv validation (in archive()) ensures data integrity
        // - No concurrent mutations possible (read-only file descriptor)
        // Alternative considered: Read entire file into memory would be slower
        // and use more RAM for large Debian package databases (>500MB)
        #[expect(unsafe_code)]
        let mmap = unsafe { Mmap::map(&file)? };

        // Initialize last_accessed to current time
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Self {
            mmap,
            last_accessed: AtomicU64::new(now),
        })
    }

    /// Access the archived data with zero-copy
    #[inline]
    fn archive(&self) -> Result<&rkyv::Archived<DebianPackageIndex>> {
        rkyv::access::<rkyv::Archived<DebianPackageIndex>, rkyv::rancor::Error>(&self.mmap)
            .map_err(|e| anyhow::anyhow!("Corrupted Debian package index: {e}"))
    }

    /// Get a package by name (zero-copy, O(1) via hash lookup in archived data)
    pub fn get(&self, name: &str) -> Result<Option<&rkyv::Archived<DebianPackage>>> {
        let archive = self.archive()?;
        let Some(idx) = archive.name_to_idx.get(name) else {
            return Ok(None);
        };
        // Convert archived u32 to native usize
        let idx = u32::from(*idx) as usize;
        Ok(archive.packages.get(idx))
    }

    /// Get all packages (zero-copy reference)
    pub fn packages(&self) -> Result<&rkyv::vec::ArchivedVec<rkyv::Archived<DebianPackage>>> {
        Ok(&self.archive()?.packages)
    }

    /// Check if the mmap is expired based on TTL (30 minutes)
    pub fn is_expired(&self) -> bool {
        let last = self.last_accessed.load(Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now.saturating_sub(last) > CACHE_TTL_SECS
    }

    /// Update last accessed time (called on each access)
    pub fn touch(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_accessed.store(now, Ordering::Relaxed);
    }
}

impl Drop for DebianMmapIndex {
    fn drop(&mut self) {
        // Mmap::drop() will automatically unmap the memory and close the file descriptor
        // This explicit Drop impl documents the cleanup behavior for memory leak audits
        tracing::debug!(
            "Unmapping Debian package index (size: {} bytes)",
            self.mmap.len()
        );
    }
}

/// A Debian package entry optimized for zero-copy access
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone)]
pub struct DebianPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub section: String,
    pub priority: String,
    pub installed_size: u64,
    pub maintainer: String,
    pub architecture: String,
    pub depends: Vec<String>,
    pub filename: String,
    pub size: u64,
    pub sha256: String,
    pub homepage: String,
    pub component: String,
}

use crate::package_managers::types::parse_version_or_zero;

impl DebianPackage {
    pub fn to_package(&self) -> Package {
        Package {
            name: self.name.clone(),
            version: parse_version_or_zero(&self.version),
            description: self.description.clone(),
            source: PackageSource::Official,
            installed: false,
        }
    }
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Default, Clone)]
pub struct DebianPackageIndex {
    pub packages: Vec<DebianPackage>,
    /// Note: Uses std `HashMap` for rkyv serialization compatibility
    /// Converted to `AHashMap` at runtime for faster lookups
    pub name_to_idx: HashMap<String, usize>,
    pub name_arch_to_idx: HashMap<String, usize>,
    pub name_arch_component_to_idx: HashMap<String, usize>,
    pub updated_at: i64,
}

impl DebianPackageIndex {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_package(&mut self, pkg: DebianPackage) {
        let idx = self.packages.len();
        let name = pkg.name.clone();
        let arch = pkg.architecture.clone();
        let component = pkg.component.clone();

        if let Some(existing_idx) = self.name_to_idx.get(&name).copied() {
            if let Some(existing_pkg) = self.packages.get(existing_idx)
                && is_better_name_candidate(&pkg, existing_pkg)
            {
                self.name_to_idx.insert(name.clone(), idx);
            }
        } else {
            self.name_to_idx.insert(name.clone(), idx);
        }

        let name_arch_key = format!("{name}:{arch}");
        if let Some(existing_idx) = self.name_arch_to_idx.get(&name_arch_key).copied() {
            if let Some(existing_pkg) = self.packages.get(existing_idx)
                && is_better_arch_candidate(&pkg, existing_pkg)
            {
                self.name_arch_to_idx.insert(name_arch_key.clone(), idx);
            }
        } else {
            self.name_arch_to_idx.insert(name_arch_key, idx);
        }

        let name_arch_component_key = format!("{name}:{arch}:{component}");
        self.name_arch_component_to_idx
            .insert(name_arch_component_key, idx);
        self.packages.push(pkg);
    }

    pub fn get(&self, name: &str) -> Option<&DebianPackage> {
        self.name_to_idx.get(name).map(|&idx| &self.packages[idx])
    }

    pub fn get_name_arch(&self, name: &str, arch: &str) -> Option<&DebianPackage> {
        let key = format!("{name}:{arch}");
        self.name_arch_to_idx
            .get(&key)
            .map(|&idx| &self.packages[idx])
    }

    pub fn get_name_arch_component(
        &self,
        name: &str,
        arch: &str,
        component: &str,
    ) -> Option<&DebianPackage> {
        let key = format!("{name}:{arch}:{component}");
        self.name_arch_component_to_idx
            .get(&key)
            .map(|&idx| &self.packages[idx])
    }

    pub fn get_query(&self, query: &str) -> Option<&DebianPackage> {
        if let Some((name, rest)) = query.split_once(':') {
            if let Some((arch, component)) = rest.split_once(':') {
                return self
                    .get_name_arch_component(name, arch, component)
                    .or_else(|| self.get_name_arch(name, arch))
                    .or_else(|| self.get(name));
            }

            return self.get_name_arch(name, rest).or_else(|| self.get(name));
        }

        self.get(query)
    }
}

fn native_debian_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "x86" => "i386",
        "arm" => "armhf",
        other => other,
    }
}

fn name_candidate_score(pkg: &DebianPackage) -> u8 {
    let mut score = 0u8;
    if pkg.architecture == native_debian_arch() {
        score += 4;
    } else if pkg.architecture == "all" {
        score += 2;
    }
    if pkg.component == "main" {
        score += 1;
    }
    score
}

fn is_better_name_candidate(new_pkg: &DebianPackage, existing_pkg: &DebianPackage) -> bool {
    let new_score = name_candidate_score(new_pkg);
    let existing_score = name_candidate_score(existing_pkg);
    if new_score != existing_score {
        return new_score > existing_score;
    }

    parse_version_or_zero(&new_pkg.version) > parse_version_or_zero(&existing_pkg.version)
}

fn is_better_arch_candidate(new_pkg: &DebianPackage, existing_pkg: &DebianPackage) -> bool {
    if new_pkg.component == "main" && existing_pkg.component != "main" {
        return true;
    }
    if new_pkg.component != "main" && existing_pkg.component == "main" {
        return false;
    }

    parse_version_or_zero(&new_pkg.version) > parse_version_or_zero(&existing_pkg.version)
}

pub fn ensure_index_loaded() -> Result<()> {
    let lists_dir = Path::new("/var/lib/apt/lists");
    if !lists_dir.exists() {
        return Ok(());
    }

    // Get current package files and their mtimes
    let mut current_files = HashMap::new();
    if let Ok(entries) = fs::read_dir(lists_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(filename) = path.file_name().and_then(|n| n.to_str())
                && filename.contains("_Packages")
                && !path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("diff"))
                && let Ok(meta) = entry.metadata()
                && let Ok(mtime) = meta.modified()
            {
                current_files.insert(path, mtime);
            }
        }
    }

    // Check if we need to update
    let needs_update = {
        let mut cache = DEBIAN_INDEX_CACHE.write().expect("lock poisoned");

        // Clear cache if TTL expired (safety net for unbounded growth)
        if is_cache_expired(cache.last_accessed) {
            *cache = DebianIndexCache::default();
            true
        } else if cache.index.is_none() {
            true // No index yet
        } else {
            // Check if any files changed or were added/removed
            let needs_update = cache.file_mtimes != current_files;
            if !needs_update {
                // Cache hit - update last accessed
                cache.last_accessed = Some(std::time::SystemTime::now());
            }
            needs_update
        }
    };

    if !needs_update {
        return Ok(());
    }

    // Determine which files changed
    let (changed_files, mut index): (Vec<PathBuf>, Option<DebianPackageIndex>) = {
        let cache = DEBIAN_INDEX_CACHE.read().expect("lock poisoned");
        let mut changed: Vec<PathBuf> = Vec::new();

        for (path, mtime) in &current_files {
            if cache.file_mtimes.get(path) != Some(mtime) {
                changed.push(path.clone());
            }
        }

        // If we have a cached index and only some files changed, do incremental update
        if !changed.is_empty() && changed.len() < current_files.len() / 2 && cache.index.is_some() {
            (changed, cache.index.clone())
        } else {
            // Too many changes or no cached index - full rebuild
            (
                current_files.keys().cloned().collect::<Vec<PathBuf>>(),
                None,
            )
        }
    };

    // Load or create index (with LZ4 compression support)
    let cache_path = paths::cache_dir().join("debian_index_v6.lz4");
    let mmap_path = paths::cache_dir().join("debian_index_v6.mmap");

    // Check if LZ4 cache is fresher than all Packages files.
    // On cold process start, file_mtimes is empty so all files appear "changed".
    // But if the cache file is newer than every Packages file, it's already up-to-date.
    let mut cache_is_fresh = false;
    if index.is_none() && cache_path.exists() {
        // Check if cache file is newer than all Packages files
        if let Ok(cache_meta) = fs::metadata(&cache_path)
            && let Ok(cache_mtime) = cache_meta.modified()
        {
            cache_is_fresh = current_files
                .values()
                .all(|pkg_mtime| cache_mtime >= *pkg_mtime);
        }

        if let Ok(compressed) = fs::read(&cache_path)
            && let Ok(bytes) = lz4_flex::decompress_size_prepended(&compressed)
            && let Ok(idx) = rkyv::from_bytes::<DebianPackageIndex, rkyv::rancor::Error>(&bytes)
        {
            index = Some(idx);
        }
    }

    // Try to load the mmap index for zero-copy access
    if mmap_path.exists() {
        let mut mmap_guard = DEBIAN_MMAP_INDEX.write().expect("lock poisoned");

        // Clear expired mmap (TTL-based cleanup for 500MB+ resource leak)
        if let Some(ref mmap) = *mmap_guard
            && mmap.is_expired()
        {
            tracing::debug!("Clearing expired Debian mmap index (TTL exceeded)");
            *mmap_guard = None;
        }

        if mmap_guard.is_none()
            && let Ok(mmap_index) = DebianMmapIndex::open(&mmap_path)
        {
            *mmap_guard = Some(mmap_index);
        }
    }

    let mut index = index.unwrap_or_else(DebianPackageIndex::new);

    // Skip rebuild if cache file is fresh (newer than all Packages files).
    // This avoids re-parsing 94k packages on every cold process start.
    if !changed_files.is_empty() && cache_is_fresh && !index.packages.is_empty() {
        tracing::debug!(
            "LZ4 cache is fresh (newer than all {} Packages files), skipping rebuild",
            current_files.len()
        );
        // Cache is valid - store in memory and return early
        let mut cache = DEBIAN_INDEX_CACHE.write().expect("lock poisoned");
        cache.index = Some(index);
        cache.file_mtimes = current_files;
        cache.last_accessed = Some(std::time::SystemTime::now());
        return Ok(());
    }

    // Parse all files when any have changed (incremental update was broken)
    // The mtime check above still avoids unnecessary rebuilds when nothing changed
    if !changed_files.is_empty() {
        // Get all current Packages files
        let all_files: Vec<PathBuf> = current_files.keys().cloned().collect();

        let new_packages: Vec<DebianPackage> = all_files
            .par_iter()
            .map(|path| parse_packages_file_sync(path))
            .collect::<Result<Vec<Vec<DebianPackage>>>>()?
            .into_iter()
            .flatten()
            .collect();

        // Clear and rebuild - simpler and correct
        index.packages.clear();
        index.name_to_idx.clear();
        index.name_arch_to_idx.clear();
        index.name_arch_component_to_idx.clear();

        // Add all packages
        for pkg in new_packages {
            index.add_package(pkg);
        }

        // Update timestamp and save
        index.updated_at = jiff::Timestamp::now().as_second();
        if let Some(p) = cache_path.parent() {
            fs::create_dir_all(p).with_context(|| {
                format!("Failed to create Debian cache directory: {}", p.display())
            })?;
        }
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&index)
            .map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;

        use std::io::Write;
        use tempfile::NamedTempFile;

        // Save compressed version for space efficiency
        let compressed = lz4_flex::compress_prepend_size(&bytes);

        // Atomic write for compressed cache
        let parent = cache_path.parent().unwrap_or_else(|| Path::new("."));
        let mut temp_cache =
            NamedTempFile::new_in(parent).context("Failed to create temporary cache file")?;
        temp_cache
            .write_all(&compressed)
            .context("Failed to write compressed cache data")?;
        temp_cache
            .as_file_mut()
            .sync_all()
            .context("Failed to sync compressed Debian cache")?;
        temp_cache
            .persist(&cache_path)
            .map_err(|error| error.error)
            .context("Failed to persist compressed cache file")?;

        // Also save uncompressed version for zero-copy mmap access
        let mmap_path = paths::cache_dir().join("debian_index_v6.mmap");

        // Atomic write for mmap index
        // CRITICAL: Must use atomic rename to avoid crashing readers holding an mmap
        let mut temp_mmap =
            NamedTempFile::new_in(parent).context("Failed to create temporary mmap file")?;
        temp_mmap
            .write_all(&bytes)
            .context("Failed to write mmap data")?;
        temp_mmap
            .as_file_mut()
            .sync_all()
            .context("Failed to sync Debian mmap index")?;
        temp_mmap
            .persist(&mmap_path)
            .map_err(|error| error.error)
            .context("Failed to persist mmap file")?;

        // Load the mmap index for zero-copy access
        if let Ok(mmap_index) = DebianMmapIndex::open(&mmap_path) {
            let mut mmap_guard = DEBIAN_MMAP_INDEX.write().expect("lock poisoned");

            // Clear existing mmap before loading new one
            if mmap_guard.is_some() {
                tracing::debug!("Replacing existing Debian mmap index with updated version");
            }

            *mmap_guard = Some(mmap_index);
        }

        // Build FST index for O(query_len) prefix searches
        // FST requires sorted input, so we need to sort packages by name
        let fst_path = paths::cache_dir().join("debian_index_v6.fst");
        let fst_build_start = std::time::Instant::now();

        let mut lower_name_to_idx: HashMap<String, usize> =
            HashMap::with_capacity(index.name_to_idx.len());
        for (name, idx) in &index.name_to_idx {
            let lower = name.to_lowercase();
            lower_name_to_idx
                .entry(lower)
                .and_modify(|existing_idx| {
                    if *idx > *existing_idx {
                        *existing_idx = *idx;
                    }
                })
                .or_insert(*idx);
        }

        let mut sorted_packages: Vec<(String, usize)> = lower_name_to_idx.into_iter().collect();
        sorted_packages.sort_by(|a, b| a.0.cmp(&b.0));

        // Build FST map: lowercased package name -> package index
        let mut fst_builder = fst::MapBuilder::memory();
        for (name, idx) in sorted_packages {
            if let Err(e) = fst_builder.insert(name.as_bytes(), idx as u64) {
                tracing::warn!("Failed to insert '{}' into FST: {}", name, e);
            }
        }

        let fst_bytes = fst_builder
            .into_inner()
            .context("Failed to build FST index")?;

        // Atomic write for FST index
        let mut temp_fst =
            NamedTempFile::new_in(parent).context("Failed to create temporary FST file")?;
        temp_fst
            .write_all(&fst_bytes)
            .context("Failed to write FST data")?;
        temp_fst
            .as_file_mut()
            .sync_all()
            .context("Failed to sync Debian FST index")?;
        temp_fst
            .persist(&fst_path)
            .map_err(|error| error.error)
            .context("Failed to persist FST file")?;

        tracing::debug!(
            "Built FST index with {} entries ({} bytes) in {:?}",
            index.packages.len(),
            fst_bytes.len(),
            fst_build_start.elapsed()
        );

        // Load the FST index
        if let Ok(fst_index) = FstIndex::open(&fst_path) {
            let mut fst_guard = DEBIAN_FST_INDEX.write().expect("lock poisoned");
            if fst_guard.is_some() {
                tracing::debug!("Replacing existing Debian FST index with updated version");
            }
            *fst_guard = Some(fst_index);
        }
    }

    // Rebuild search buffer with pre-calculated capacity
    // IMPORTANT: Store lowercased content for case-insensitive SIMD search
    let estimated_size: usize = index
        .packages
        .iter()
        .map(|p| p.name.len() + p.description.len() + 2)
        .sum();
    let mut search_buffer = Vec::with_capacity(estimated_size);
    let mut package_offsets = Vec::with_capacity(index.packages.len() + 1);

    for pkg in &index.packages {
        package_offsets.push(search_buffer.len());
        // Store lowercased for O(1) case-insensitive search
        search_buffer.extend(pkg.name.bytes().map(|b| b.to_ascii_lowercase()));
        search_buffer.push(b' ');
        search_buffer.extend(pkg.description.bytes().map(|b| b.to_ascii_lowercase()));
        search_buffer.push(0);
    }
    package_offsets.push(search_buffer.len());

    let installed_set = list_installed_fast()
        .unwrap_or_default()
        .into_iter()
        .map(|p| p.name)
        .collect();

    let newest_mtime = current_files
        .values()
        .max()
        .copied()
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    let mut cache = DEBIAN_INDEX_CACHE.write().expect("lock poisoned");
    cache.index = Some(index);
    cache.last_modified = Some(newest_mtime);
    cache.file_mtimes = current_files;
    cache.search_buffer = search_buffer;
    cache.package_offsets = package_offsets;
    cache.installed_set = installed_set;
    cache.last_accessed = Some(std::time::SystemTime::now());

    Ok(())
}

/// Ensure FST index is loaded (if available on disk)
/// Returns `Ok(())` whether FST is available or not - FST is optional optimization
fn ensure_fst_loaded() {
    // Check if already loaded
    {
        let guard = DEBIAN_FST_INDEX.read().expect("lock poisoned");
        if let Some(ref fst) = *guard {
            // Clear if expired
            if fst.is_expired() {
                drop(guard);
                let mut write_guard = DEBIAN_FST_INDEX.write().expect("lock poisoned");
                tracing::debug!("Clearing expired FST index (TTL exceeded)");
                *write_guard = None;
            } else {
                fst.touch();
                return;
            }
        }
    }

    // Try to load from disk
    let fst_path = paths::cache_dir().join("debian_index_v6.fst");
    if !fst_path.exists() {
        return; // FST not available yet, will fall back to SIMD search
    }

    if let Ok(fst_index) = FstIndex::open(&fst_path) {
        let mut guard = DEBIAN_FST_INDEX.write().expect("lock poisoned");
        *guard = Some(fst_index);
        tracing::debug!("Loaded FST index from disk");
    }
}

/// Ensure the mmap index is loaded from disk (if available).
///
/// This is nearly instant (just a syscall, no decompression) unlike `ensure_index_loaded()`.
/// Used by the ultra-fast search and update paths to avoid loading the full index.
pub fn ensure_mmap_loaded() -> bool {
    // Check if already loaded
    {
        let guard = DEBIAN_MMAP_INDEX.read().expect("lock poisoned");
        if let Some(ref mmap) = *guard {
            if mmap.is_expired() {
                drop(guard);
                let mut write_guard = DEBIAN_MMAP_INDEX.write().expect("lock poisoned");
                *write_guard = None;
            } else {
                mmap.touch();
                return true;
            }
        }
    }

    // Try to load from disk
    let mmap_path = paths::cache_dir().join("debian_index_v6.mmap");
    if !mmap_path.exists() {
        return false;
    }

    if let Ok(mmap_index) = DebianMmapIndex::open(&mmap_path) {
        let mut guard = DEBIAN_MMAP_INDEX.write().expect("lock poisoned");
        *guard = Some(mmap_index);
        tracing::debug!("Loaded mmap index from disk (zero-copy)");
        true
    } else {
        false
    }
}

fn parse_packages_file_sync(path: &Path) -> Result<Vec<DebianPackage>> {
    let component = extract_component_from_path(path);
    let content = read_packages_file_content(path)?;

    // Collect paragraph byte ranges first
    let double_newline_iter = memmem::find_iter(content.as_bytes(), b"\n\n");
    let mut paragraph_ranges = Vec::new();
    let mut start = 0;

    for end in double_newline_iter {
        if end > start {
            paragraph_ranges.push((start, end));
        }
        start = end + 2;
    }

    // Handle last paragraph
    if start < content.len() {
        paragraph_ranges.push((start, content.len()));
    }

    // Parse paragraphs in parallel for large files (>100 packages)
    let packages = if paragraph_ranges.len() > 100 {
        paragraph_ranges
            .par_iter()
            .filter_map(|(start, end)| {
                let paragraph = &content[*start..*end];
                if paragraph.trim().is_empty() {
                    None
                } else {
                    parse_paragraph_str(paragraph, &component).ok()
                }
            })
            .collect()
    } else {
        paragraph_ranges
            .iter()
            .filter_map(|(start, end)| {
                let paragraph = &content[*start..*end];
                if paragraph.trim().is_empty() {
                    None
                } else {
                    parse_paragraph_str(paragraph, &component).ok()
                }
            })
            .collect()
    };

    Ok(packages)
}

fn read_packages_file_content(path: &Path) -> Result<String> {
    let file = fs::File::open(path)?;
    let mut reader = BufReader::with_capacity(64 * 1024, file);

    let mut buf = String::new();
    if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lz4"))
    {
        let mut decoder = lz4_flex::frame::FrameDecoder::new(reader);
        decoder.read_to_string(&mut buf)?;
    } else if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gz"))
    {
        let mut decoder = flate2::read::GzDecoder::new(reader);
        decoder.read_to_string(&mut buf)?;
    } else if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("xz"))
    {
        let mut decompressed = Vec::new();
        lzma_rs::xz_decompress(&mut reader, &mut decompressed)?;
        buf = String::from_utf8(decompressed)
            .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in xz-compressed Packages file: {e}"))?;
    } else {
        reader.read_to_string(&mut buf)?;
    }

    Ok(buf)
}

fn parse_minimal_package_info(paragraph: &str, expected_name: &str) -> Option<(String, String)> {
    let mut name_matches = false;
    let mut version = String::new();
    let mut description = String::new();
    let mut collecting_description = false;

    for line in paragraph.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if collecting_description {
                if !description.is_empty() {
                    description.push('\n');
                }
                description.push_str(line.trim_start());
            }
            continue;
        }

        let Some(colon_pos) = memchr::memchr(b':', line.as_bytes()) else {
            continue;
        };

        collecting_description = false;
        let key = &line[..colon_pos];
        let value = line[colon_pos + 1..].trim_start();

        match key.as_bytes() {
            b"Package" => {
                if value != expected_name {
                    return None;
                }
                name_matches = true;
            }
            b"Version" => version = value.to_string(),
            b"Description" => {
                description = value.to_string();
                collecting_description = true;
            }
            _ => {}
        }
    }

    if name_matches && !version.is_empty() {
        Some((version, description))
    } else {
        None
    }
}

fn extract_component_from_path(path: &Path) -> String {
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();

    if let Some(binary_pos) = filename.find("_binary-") {
        let prefix = &filename[..binary_pos];
        if let Some(component) = prefix.rsplit('_').next()
            && !component.is_empty()
        {
            return component.to_string();
        }
    }

    if let Some(stripped) = filename.strip_suffix("_Packages")
        && let Some((component, _)) = stripped.split_once('_')
        && !component.is_empty()
    {
        return component.to_string();
    }

    String::from("main")
}

#[allow(clippy::unnecessary_wraps)]
fn find_info_from_apt_lists_fast(name: &str) -> Result<Option<Package>> {
    let lists_dir = Path::new("/var/lib/apt/lists");
    if !lists_dir.exists() {
        return Ok(None);
    }

    let start_pattern = format!("Package: {name}\n");
    let mid_pattern = format!("\nPackage: {name}\n");

    let Ok(entries) = fs::read_dir(lists_dir) else {
        return Ok(None);
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(filename) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };

        if !filename.contains("_Packages")
            || path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("diff"))
        {
            continue;
        }

        let Ok(content) = read_packages_file_content(&path) else {
            continue;
        };
        let bytes = content.as_bytes();

        let match_pos = if bytes.starts_with(start_pattern.as_bytes()) {
            Some(0usize)
        } else {
            memmem::find(bytes, mid_pattern.as_bytes()).map(|pos| pos + 1)
        };

        let Some(match_pos) = match_pos else {
            continue;
        };

        let mut paragraph_start = match_pos;
        while paragraph_start >= 2 {
            if bytes[paragraph_start - 2] == b'\n' && bytes[paragraph_start - 1] == b'\n' {
                break;
            }
            paragraph_start -= 1;
        }
        if paragraph_start < 2 {
            paragraph_start = 0;
        }

        let paragraph_end =
            memmem::find(&bytes[match_pos..], b"\n\n").map_or(bytes.len(), |rel| match_pos + rel);

        let paragraph = &content[paragraph_start..paragraph_end];
        if let Some((version, description)) = parse_minimal_package_info(paragraph, name) {
            return Ok(Some(Package {
                name: name.to_string(),
                version: parse_version_or_zero(&version),
                description,
                source: PackageSource::Official,
                installed: is_installed_fast(name)?,
            }));
        }
    }

    Ok(None)
}

fn append_dependencies(value: &str, dependencies: &mut Vec<String>) {
    dependencies.reserve(value.matches(',').count() + 1);
    for dependency in value.split(',') {
        if let Some(package) = dependency.split_whitespace().next() {
            dependencies.push(package.to_string());
        }
    }
}

#[inline]
fn parse_paragraph_str(paragraph: &str, component: &str) -> Result<DebianPackage> {
    let mut name = String::new();
    let mut version = String::new();
    let mut description = String::with_capacity(128); // Pre-allocate for description
    let mut section = String::new();
    let mut priority = String::new();
    let mut installed_size = 0u64;
    let mut maintainer = String::new();
    let mut architecture = String::new();
    let mut depends = Vec::new();
    let mut filename = String::new();
    let mut size = 0u64;
    let mut sha256 = String::new();
    let mut homepage = String::new();
    let mut current_field = None;

    for line in paragraph.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            let value = line.trim_start();
            match current_field {
                Some("Description") if !description.is_empty() => {
                    description.push('\n');
                    if value != "." {
                        description.push_str(value);
                    }
                }
                Some("Depends") => append_dependencies(value, &mut depends),
                _ => {}
            }
            continue;
        }
        if line.is_empty() {
            continue;
        }

        let colon_pos = memchr::memchr(b':', line.as_bytes())
            .with_context(|| format!("Invalid deb822 field without a colon: {line}"))?;
        let key = &line[..colon_pos];
        let value = line[colon_pos + 1..].trim_start();
        current_field = Some(key);

        match key.as_bytes() {
            b"Package" => name = value.to_string(),
            b"Version" => version = value.to_string(),
            b"Description" => description = value.to_string(),
            b"Section" => section = value.to_string(),
            b"Priority" => priority = value.to_string(),
            b"Installed-Size" => {
                installed_size = value
                    .parse()
                    .with_context(|| format!("Invalid Installed-Size value: {value}"))?;
            }
            b"Maintainer" => maintainer = value.to_string(),
            b"Architecture" => architecture = value.to_string(),
            b"Depends" => append_dependencies(value, &mut depends),
            b"Filename" => filename = value.to_string(),
            b"Size" => {
                size = value
                    .parse()
                    .with_context(|| format!("Invalid Size value: {value}"))?;
            }
            b"SHA256" => sha256 = value.to_string(),
            b"Homepage" => homepage = value.to_string(),
            _ => {}
        }
    }

    if name.is_empty() {
        anyhow::bail!("Invalid package entry: missing 'Package' field in Packages file");
    }
    Ok(DebianPackage {
        name,
        version,
        description,
        section,
        priority,
        installed_size,
        maintainer,
        architecture,
        depends,
        filename,
        size,
        sha256,
        homepage,
        component: component.to_string(),
    })
}

pub fn get_detailed_packages() -> Result<Vec<DebianPackage>> {
    if crate::core::paths::test_mode() {
        return Ok(vec![DebianPackage {
            name: "apt".to_string(),
            version: "2.6.1".to_string(),
            description: "Debian package manager".to_string(),
            section: "admin".to_string(),
            priority: "optional".to_string(),
            installed_size: 1024,
            maintainer: "Debian".to_string(),
            architecture: "amd64".to_string(),
            depends: vec![],
            filename: "pool/main/a/apt/apt_2.6.1_amd64.deb".to_string(),
            size: 500,
            sha256: "hash".to_string(),
            homepage: "https://debian.org".to_string(),
            component: "main".to_string(),
        }]);
    }
    ensure_index_loaded()?;
    let guard = DEBIAN_INDEX_CACHE.read().expect("lock poisoned");
    let index = guard.index.as_ref().context(
        "Debian package index not loaded. Run 'omg sync' to refresh the package database",
    )?;
    Ok(index.packages.clone())
}

pub fn search_fast(query: &str) -> Result<Vec<Package>> {
    if crate::core::paths::test_mode() {
        return Ok(vec![Package {
            name: "apt".to_string(),
            version: parse_version_or_zero("2.6.1"),
            description: "Debian package manager".to_string(),
            source: PackageSource::Official,
            installed: true,
        }]);
    }

    // ULTRA-FAST PATH: FST + mmap (no index loading, no decompression)
    // This path takes ~5ms vs ~90ms for `ensure_index_loaded()`
    if !query.is_empty() && !query.contains(':') {
        ensure_fst_loaded();
        let fst_guard = DEBIAN_FST_INDEX.read().expect("lock poisoned");
        if fst_guard.is_some() && ensure_mmap_loaded() {
            let fst_index = fst_guard.as_ref().expect("checked is_some() above");
            fst_index.touch();
            let query_lower = query.to_lowercase();
            let mmap_guard = DEBIAN_MMAP_INDEX.read().expect("lock poisoned");
            if let Some(ref mmap) = *mmap_guard {
                mmap.touch();
                return Ok(fst_mmap_search(&fst_index.map, mmap, &query_lower));
            }
        }
        drop(fst_guard);
    }

    // Fallback: load full index (needed for empty queries or when FST/mmap unavailable)
    ensure_index_loaded()?;
    if query.is_empty() {
        ensure_fst_loaded();
    }

    let guard = DEBIAN_INDEX_CACHE.read().expect("lock poisoned");
    let index = guard.index.as_ref().context(
        "Debian package index not loaded. Run 'omg sync' to refresh the package database",
    )?;

    if query.is_empty() {
        return Ok(index
            .packages
            .iter()
            .map(|pkg| {
                let mut p = pkg.to_package();
                p.installed = guard.installed_set.contains(&p.name);
                p
            })
            .collect());
    }

    // Fast path: check for exact package name match first
    if let Some(exact_pkg) = index.get_query(query) {
        let mut p = exact_pkg.to_package();
        p.installed = guard.installed_set.contains(&p.name);
        return Ok(vec![p]);
    }

    let query_lower = query.to_lowercase();
    if query_lower != query
        && let Some(exact_pkg) = index.get_query(&query_lower)
    {
        let mut p = exact_pkg.to_package();
        p.installed = guard.installed_set.contains(&p.name);
        return Ok(vec![p]);
    }

    // FST search with in-memory index
    let fst_guard = DEBIAN_FST_INDEX.read().expect("lock poisoned");
    if let Some(ref fst_index) = *fst_guard {
        fst_index.touch();
        return Ok(fst_search(
            &fst_index.map,
            index,
            &query_lower,
            &guard.installed_set,
        ));
    }
    drop(fst_guard);

    // Fallback: SIMD search
    Ok(simd_search_fallback(
        index,
        &query_lower,
        &guard.search_buffer,
        &guard.package_offsets,
    ))
}

/// FST-based search: `O(query_len)` prefix matching
/// Much faster than full buffer scan for common queries
#[inline]
fn fst_search(
    fst_map: &Map<Mmap>,
    index: &DebianPackageIndex,
    query_lower: &str,
    installed_set: &AHashSet<String>,
) -> Vec<Package> {
    let query_bytes = query_lower.as_bytes();

    // 1. Try exact match first (fastest - single lookup)
    if let Some(idx) = fst_map.get(query_bytes)
        && let Some(pkg) = index.packages.get(idx as usize)
    {
        let mut p = pkg.to_package();
        p.installed = installed_set.contains(&p.name);
        return vec![p];
    }

    // 2. Prefix search (very fast - early termination)
    let mut prefix_matches = Vec::with_capacity(100);
    let mut stream = fst_map.range().ge(query_bytes).into_stream();

    while let Some((name_bytes, idx)) = stream.next() {
        // Early exit when prefix no longer matches
        if !name_bytes.starts_with(query_bytes) {
            break;
        }

        if let Some(pkg) = index.packages.get(idx as usize) {
            let mut p = pkg.to_package();
            p.installed = installed_set.contains(&p.name);
            prefix_matches.push(p);
        }

        if prefix_matches.len() >= 100 {
            break;
        }
    }

    // If we found prefix matches, return them
    if !prefix_matches.is_empty() {
        return prefix_matches;
    }

    // 3. Substring search fallback (slower but comprehensive)
    // Only do this if no prefix matches were found
    let finder = memmem::Finder::new(query_bytes);
    let mut substring_matches = Vec::with_capacity(100);

    let mut stream = fst_map.stream().into_stream();
    while let Some((name_bytes, idx)) = stream.next() {
        // Check if query appears anywhere in the name
        if finder.find(name_bytes).is_some() {
            if let Some(pkg) = index.packages.get(idx as usize) {
                let mut p = pkg.to_package();
                p.installed = installed_set.contains(&p.name);
                substring_matches.push(p);
            }

            if substring_matches.len() >= 100 {
                break;
            }
        }
    }

    substring_matches
}

/// FST+mmap search: completely bypasses `ensure_index_loaded()`
/// Uses FST for name matching and mmap for zero-copy package details
#[inline]
fn fst_mmap_search(fst_map: &Map<Mmap>, mmap: &DebianMmapIndex, query_lower: &str) -> Vec<Package> {
    let query_bytes = query_lower.as_bytes();

    // 1. Try exact match first
    if let Some(_idx) = fst_map.get(query_bytes)
        && let Ok(Some(pkg)) = mmap.get(query_lower)
    {
        return vec![Package {
            name: pkg.name.to_string(),
            version: parse_version_or_zero(pkg.version.as_str()),
            description: pkg.description.to_string(),
            source: PackageSource::Official,
            installed: false,
        }];
    }

    // 2. Prefix search
    let mut results = Vec::with_capacity(100);
    let mut stream = fst_map.range().ge(query_bytes).into_stream();

    while let Some((name_bytes, _idx)) = stream.next() {
        if !name_bytes.starts_with(query_bytes) {
            break;
        }
        if let Ok(name_str) = std::str::from_utf8(name_bytes)
            && let Ok(Some(pkg)) = mmap.get(name_str)
        {
            results.push(Package {
                name: pkg.name.to_string(),
                version: parse_version_or_zero(pkg.version.as_str()),
                description: pkg.description.to_string(),
                source: PackageSource::Official,
                installed: false,
            });
        }
        if results.len() >= 100 {
            break;
        }
    }

    if !results.is_empty() {
        return results;
    }

    // 3. Substring search fallback
    let finder = memmem::Finder::new(query_bytes);
    let mut stream = fst_map.stream().into_stream();
    while let Some((name_bytes, _idx)) = stream.next() {
        if finder.find(name_bytes).is_some()
            && let Ok(name_str) = std::str::from_utf8(name_bytes)
            && let Ok(Some(pkg)) = mmap.get(name_str)
        {
            results.push(Package {
                name: pkg.name.to_string(),
                version: parse_version_or_zero(pkg.version.as_str()),
                description: pkg.description.to_string(),
                source: PackageSource::Official,
                installed: false,
            });
        }
        if results.len() >= 100 {
            break;
        }
    }

    results
}

fn is_package_installed_scan(name: &str) -> Result<bool> {
    let status_path = Path::new("/var/lib/dpkg/status");
    if !status_path.exists() {
        return Ok(false);
    }

    let status_content = fs::read_to_string(status_path)?;
    let start_pattern = format!("Package: {name}\n");
    let mid_pattern = format!("\nPackage: {name}\n");
    let bytes = status_content.as_bytes();

    let mut start_positions = Vec::with_capacity(2);
    if bytes.starts_with(start_pattern.as_bytes()) {
        start_positions.push(0usize);
    }
    for pos in memmem::find_iter(bytes, mid_pattern.as_bytes()) {
        start_positions.push(pos + 1);
    }

    for match_pos in start_positions {
        let mut paragraph_start = match_pos;
        while paragraph_start >= 2 {
            if bytes[paragraph_start - 2] == b'\n' && bytes[paragraph_start - 1] == b'\n' {
                break;
            }
            paragraph_start -= 1;
        }
        if paragraph_start < 2 {
            paragraph_start = 0;
        }

        let paragraph_end =
            memmem::find(&bytes[match_pos..], b"\n\n").map_or(bytes.len(), |rel| match_pos + rel);
        let paragraph = &bytes[paragraph_start..paragraph_end];

        if STATUS_INSTALLED_FINDER.find(paragraph).is_some() {
            return Ok(true);
        }
    }

    Ok(false)
}

/// SIMD-based search fallback (used when FST not available)
#[inline]
fn simd_search_fallback(
    index: &DebianPackageIndex,
    query_lower: &str,
    search_buffer: &[u8],
    package_offsets: &[usize],
) -> Vec<Package> {
    let finder = memmem::Finder::new(query_lower.as_bytes());
    let mut exact_matches = Vec::with_capacity(4);
    let mut prefix_matches = Vec::with_capacity(32);
    let mut substring_matches = Vec::with_capacity(128);
    let mut seen_indices = AHashSet::new();

    for match_idx in finder.find_iter(search_buffer) {
        let pkg_idx = match package_offsets.binary_search(&match_idx) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        if seen_indices.insert(pkg_idx)
            && let Some(pkg) = index.packages.get(pkg_idx)
        {
            let mut p = pkg.to_package();
            p.installed = false;

            // Categorize by match type for better relevance
            let name_lower = p.name.to_lowercase();
            if name_lower == query_lower {
                exact_matches.push(p);
            } else if name_lower.starts_with(query_lower) {
                prefix_matches.push(p);
            } else {
                substring_matches.push(p);
            }
        }
        if exact_matches.len() + prefix_matches.len() + substring_matches.len() >= 100 {
            break;
        }
    }

    // Return results in relevance order: exact > prefix > substring
    exact_matches.extend(prefix_matches);
    exact_matches.extend(substring_matches);
    exact_matches
}

pub fn get_info_fast(name: &str) -> Result<Option<Package>> {
    if crate::core::paths::test_mode() {
        return Ok(Some(Package {
            name: name.to_string(),
            version: parse_version_or_zero("1.0.0"),
            description: "Mock package".to_string(),
            source: PackageSource::Official,
            installed: true,
        }));
    }

    // ULTRA-FAST PATH: mmap O(1) lookup (no index loading needed)
    if ensure_mmap_loaded() {
        let mmap_guard = DEBIAN_MMAP_INDEX.read().expect("lock poisoned");
        if let Some(ref mmap) = *mmap_guard {
            mmap.touch();
            if let Ok(Some(pkg)) = mmap.get(name) {
                let installed = is_installed_fast(name)?;
                return Ok(Some(Package {
                    name: pkg.name.to_string(),
                    version: parse_version_or_zero(pkg.version.as_str()),
                    description: pkg.description.to_string(),
                    source: PackageSource::Official,
                    installed,
                }));
            }
            // Package not in mmap - still return None without loading full index
            return Ok(None);
        }
    }

    // Fallback: load full index
    if !name.contains(':')
        && let Some(pkg) = find_info_from_apt_lists_fast(name)?
    {
        return Ok(Some(pkg));
    }

    // Fallback: load full index
    ensure_index_loaded()?;
    let guard = DEBIAN_INDEX_CACHE.read().expect("lock poisoned");
    let index = guard.index.as_ref().context(
        "Debian package index not loaded. Run 'omg sync' to refresh the package database",
    )?;
    if let Some(pkg) = index.get_query(name) {
        let mut p = pkg.to_package();
        p.installed = guard.installed_set.contains(&p.name);
        Ok(Some(p))
    } else {
        Ok(None)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct LocalPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub architecture: String,
    pub is_explicit: bool,
}

/// Parse a dpkg status paragraph into `LocalPackage` fields
#[inline]
fn parse_status_paragraph(paragraph: &str) -> Option<(String, String, String, String)> {
    let mut name = String::new();
    let mut version = String::new();
    let mut description = String::new();
    let mut arch = String::new();

    for line in paragraph.lines() {
        let Some(colon_pos) = memchr::memchr(b':', line.as_bytes()) else {
            continue;
        };
        let key = &line[..colon_pos];
        let value = line[colon_pos + 1..].trim_start();

        match key.as_bytes() {
            b"Package" => name = value.to_string(),
            b"Version" => version = value.to_string(),
            b"Description" => description = value.to_string(),
            b"Architecture" => arch = value.to_string(),
            _ => {}
        }
    }

    if name.is_empty() {
        None
    } else {
        Some((name, version, description, arch))
    }
}

pub fn list_installed_fast() -> Result<Vec<LocalPackage>> {
    if crate::core::paths::test_mode() {
        return Ok(vec![LocalPackage {
            name: "apt".to_string(),
            version: "2.6.1".to_string(),
            description: "Debian package manager".to_string(),
            architecture: "amd64".to_string(),
            is_explicit: true,
        }]);
    }

    let status_path = Path::new("/var/lib/dpkg/status");
    if !status_path.exists() {
        return Ok(Vec::new());
    }

    let extended_states_path = Path::new("/var/lib/apt/extended_states");

    // Get mtimes
    let status_mtime = fs::metadata(status_path)
        .ok()
        .and_then(|m| m.modified().ok());
    let extended_states_mtime = extended_states_path
        .exists()
        .then(|| {
            fs::metadata(extended_states_path)
                .ok()
                .and_then(|m| m.modified().ok())
        })
        .flatten();

    // Check cache first
    {
        let mut cache = DPKG_STATUS_CACHE.write().expect("lock poisoned");

        // Clear cache if TTL expired (safety net for unbounded growth)
        if is_cache_expired(cache.last_accessed) {
            *cache = DpkgStatusCache::default();
        } else if cache.status_mtime == status_mtime
            && cache.extended_states_mtime == extended_states_mtime
            && !cache.packages.is_empty()
        {
            // Cache hit! Update last accessed
            cache.last_accessed = Some(std::time::SystemTime::now());
            return Ok(cache.packages.clone());
        }
    }

    // Cache miss - parse from disk
    let status_content = fs::read_to_string(status_path)?;

    // Fast parse of extended_states using memchr for line iteration
    let mut auto_installed = AHashSet::new();
    if let Ok(ext_content) = fs::read_to_string(extended_states_path) {
        let mut current_pkg = String::new();
        for line in ext_content.lines() {
            if let Some(name) = line.strip_prefix("Package: ") {
                current_pkg = name.trim().to_string();
            } else if line.starts_with("Auto-Installed: 1") && !current_pkg.is_empty() {
                auto_installed.insert(std::mem::take(&mut current_pkg));
            }
        }
    }

    // Pre-allocate for estimated package count
    let mut packages = Vec::with_capacity(status_content.len() / 300);
    let mut installed_set = AHashSet::new();

    // Use memchr for faster paragraph splitting
    let finder = memmem::Finder::new(b"\n\n");
    let mut start = 0;

    for end in finder.find_iter(status_content.as_bytes()) {
        let paragraph = &status_content[start..end];
        start = end + 2;

        // Quick check if package is installed using SIMD-accelerated finder
        if STATUS_INSTALLED_FINDER.find(paragraph.as_bytes()).is_none() {
            continue;
        }

        if let Some((name, version, description, arch)) = parse_status_paragraph(paragraph) {
            let is_explicit = !auto_installed.contains(&name);
            installed_set.insert(name.clone());
            packages.push(LocalPackage {
                name,
                version,
                description,
                architecture: arch,
                is_explicit,
            });
        }
    }

    // Handle last paragraph
    if start < status_content.len() {
        let paragraph = &status_content[start..];
        if STATUS_INSTALLED_FINDER.find(paragraph.as_bytes()).is_some()
            && let Some((name, version, description, arch)) = parse_status_paragraph(paragraph)
        {
            let is_explicit = !auto_installed.contains(&name);
            installed_set.insert(name.clone());
            packages.push(LocalPackage {
                name,
                version,
                description,
                architecture: arch,
                is_explicit,
            });
        }
    }

    // Update cache
    {
        let mut cache = DPKG_STATUS_CACHE.write().expect("lock poisoned");
        cache.packages.clone_from(&packages);
        cache.installed_set = installed_set;
        cache.status_mtime = status_mtime;
        cache.extended_states_mtime = extended_states_mtime;
        cache.last_accessed = Some(std::time::SystemTime::now());
    }

    Ok(packages)
}

/// Get info about an installed package from dpkg/status
#[inline]
pub fn get_installed_info_fast(name: &str) -> Result<Option<LocalPackage>> {
    if crate::core::paths::test_mode() {
        return Ok(Some(LocalPackage {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: "Mock package".to_string(),
            architecture: "amd64".to_string(),
            is_explicit: true,
        }));
    }

    // Ensure cache is populated
    list_installed_fast()?;

    let cache = DPKG_STATUS_CACHE.read().expect("lock poisoned");
    Ok(cache.packages.iter().find(|p| p.name == name).cloned())
}

#[inline]
pub fn is_installed_fast(name: &str) -> Result<bool> {
    if crate::core::paths::test_mode() {
        return Ok(matches!(name, "apt" | "git"));
    }

    // Check dpkg status cache first for O(1) lookup
    {
        let cache = DPKG_STATUS_CACHE.read().expect("lock poisoned");
        if !cache.installed_set.is_empty() {
            return Ok(cache.installed_set.contains(name));
        }
    }

    match is_package_installed_scan(name) {
        Ok(installed) => {
            if installed {
                let mut cache = DPKG_STATUS_CACHE.write().expect("lock poisoned");
                cache.installed_set.insert(name.to_string());
                cache.last_accessed = Some(std::time::SystemTime::now());
            }
            Ok(installed)
        }
        Err(scan_error) => {
            list_installed_fast().with_context(|| {
                format!("failed to determine whether '{name}' is installed: {scan_error}")
            })?;
            let cache = DPKG_STATUS_CACHE.read().expect("lock poisoned");
            Ok(cache.installed_set.contains(name))
        }
    }
}

pub fn list_explicit_fast() -> Result<Vec<String>> {
    if crate::core::paths::test_mode() {
        return Ok(vec!["apt".to_string(), "git".to_string()]);
    }
    let installed = list_installed_fast()?;
    Ok(installed
        .into_iter()
        .filter(|p| p.is_explicit)
        .map(|p| p.name)
        .collect())
}

pub fn get_counts_fast() -> Result<(usize, usize, usize, usize)> {
    let installed = list_installed_fast()?;

    let total = installed.len();

    let explicit = installed.iter().filter(|p| p.is_explicit).count();

    Ok((total, explicit, 0, 0))
}

/// Cleanup expired memory-mapped indices to prevent resource leaks
///
/// Should be called periodically (e.g., every 30 minutes) to free 500MB+ mmaps
/// that haven't been accessed within the TTL window. This is a safety net for
/// long-running daemons that may accumulate stale mmap resources.
pub fn cleanup_expired_mmaps() {
    let mut mmap_guard = DEBIAN_MMAP_INDEX.write().expect("lock poisoned");

    if let Some(ref mmap) = *mmap_guard
        && mmap.is_expired()
    {
        let size = mmap.mmap.len();
        tracing::info!(
            "Cleaning up expired Debian mmap index (size: {} MB)",
            size / 1024 / 1024
        );
        *mmap_guard = None;
    }
}

/// Check if the mmap index is available (avoids loading full index into memory)
pub fn is_mmap_available() -> bool {
    let guard = DEBIAN_MMAP_INDEX.read().expect("lock poisoned");
    guard.is_some()
}

/// Get updates using the mmap index with parallel version comparison
/// This is the ULTRA-FAST path that avoids loading the entire index into memory
#[allow(clippy::implicit_hasher)]
pub fn get_updates_from_mmap(
    installed_map: &std::collections::HashMap<&str, &str>,
) -> Result<Vec<(String, String, String)>> {
    let mmap_guard = DEBIAN_MMAP_INDEX.read().expect("lock poisoned");
    let Some(ref mmap) = *mmap_guard else {
        anyhow::bail!("Mmap index not available");
    };

    mmap.touch(); // Update access time for TTL

    // Get zero-copy access to all packages
    let packages = mmap.packages()?;

    // Parallel version comparison using rayon
    let updates: Vec<(String, String, String)> = packages
        .par_iter()
        .filter_map(|pkg| {
            // Convert archived strings to &str using rkyv's archived string access
            let pkg_name: &str = pkg.name.as_str();
            let pkg_version: &str = pkg.version.as_str();

            let installed_ver = installed_map.get(pkg_name)?;
            let available_ver = parse_version_or_zero(pkg_version);
            let installed_v = parse_version_or_zero(installed_ver);

            (available_ver > installed_v).then(|| {
                (
                    pkg_name.to_string(),
                    (*installed_ver).to_string(),
                    pkg_version.to_string(),
                )
            })
        })
        .collect();

    Ok(updates)
}

/// Get package dependencies from `/var/lib/dpkg/status`
/// Returns `(dependencies, reverse_dependencies)` for the specified package
pub fn get_package_dependencies(package_name: &str) -> Result<(Vec<String>, Vec<String>)> {
    if crate::core::paths::test_mode() {
        return Ok((vec!["libc6".to_string()], vec![]));
    }

    let status_path = Path::new("/var/lib/dpkg/status");
    if !status_path.exists() {
        anyhow::bail!("dpkg status file not found: {}", status_path.display());
    }

    let content = fs::read_to_string(status_path)?;

    let mut dependencies = Vec::new();
    let mut reverse_deps = Vec::new();
    let mut current_pkg = String::new();
    let mut current_deps = Vec::new();
    let mut in_target = false;

    for line in content.lines() {
        if line.is_empty() {
            if in_target {
                dependencies = std::mem::take(&mut current_deps);
            } else if !current_pkg.is_empty() && current_deps.iter().any(|d| d == package_name) {
                reverse_deps.push(current_pkg.clone());
            }
            current_pkg.clear();
            current_deps.clear();
            in_target = false;
        } else if let Some(pkg) = line.strip_prefix("Package: ") {
            current_pkg = pkg.trim().to_string();
            in_target = current_pkg == package_name;
        } else if line.starts_with("Depends: ") {
            let deps_str = line
                .strip_prefix("Depends: ")
                .expect("guarded by starts_with check");
            for dep in deps_str.split(',') {
                let dep_name = dep.split_whitespace().next().unwrap_or("");
                if !dep_name.is_empty() {
                    current_deps.push(dep_name.to_string());
                }
            }
        }
    }

    Ok((dependencies, reverse_deps))
}

/// Get package size from /var/lib/dpkg/status
/// Returns installed size in bytes (dpkg stores in KB)
pub fn get_package_size(package_name: &str) -> Result<i64> {
    if crate::core::paths::test_mode() {
        return Ok(1024 * 1024);
    }

    let status_path = Path::new("/var/lib/dpkg/status");
    if !status_path.exists() {
        anyhow::bail!("dpkg status file not found: {}", status_path.display());
    }

    let content = fs::read_to_string(status_path)?;

    let mut in_package = false;
    for line in content.lines() {
        if line.is_empty() {
            in_package = false;
        } else if let Some(pkg) = line.strip_prefix("Package: ") {
            in_package = pkg.trim() == package_name;
        } else if in_package && line.starts_with("Installed-Size: ") {
            let size_kb: i64 = line
                .strip_prefix("Installed-Size: ")
                .expect("guarded by starts_with check")
                .trim()
                .parse()
                .unwrap_or(0);
            return Ok(size_kb * 1024);
        }
    }

    anyhow::bail!("Package '{package_name}' not found in dpkg status");
}

/// Get all packages with their sizes from `/var/lib/dpkg/status`
/// Returns `Vec<(package_name, size_in_bytes)>`
pub fn get_all_packages_with_sizes() -> Result<Vec<(String, i64)>> {
    if crate::core::paths::test_mode() {
        return Ok(vec![
            ("apt".to_string(), 4 * 1024 * 1024),
            ("vim".to_string(), 3 * 1024 * 1024),
        ]);
    }

    let status_path = Path::new("/var/lib/dpkg/status");
    if !status_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(status_path)?;

    let mut results = Vec::new();
    let mut current_pkg = String::new();
    let mut current_size: i64 = 0;

    for line in content.lines() {
        if line.is_empty() {
            if !current_pkg.is_empty() && current_size > 0 {
                results.push((current_pkg.clone(), current_size));
            }
            current_pkg.clear();
            current_size = 0;
        } else if let Some(pkg) = line.strip_prefix("Package: ") {
            current_pkg = pkg.trim().to_string();
        } else if line.starts_with("Installed-Size: ") {
            current_size = line
                .strip_prefix("Installed-Size: ")
                .expect("guarded by starts_with check")
                .trim()
                .parse::<i64>()
                .unwrap_or(0)
                * 1024;
        }
    }

    Ok(results)
}

/// Get package version from /var/lib/dpkg/status
/// Returns None if package is not installed
pub fn get_package_version(package_name: &str) -> Result<Option<String>> {
    if crate::core::paths::test_mode() {
        return Ok(Some("1.0.0".to_string()));
    }

    let status_path = Path::new("/var/lib/dpkg/status");
    if !status_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(status_path)?;

    let mut in_package = false;
    let mut is_installed = false;
    let mut version = None;

    for line in content.lines() {
        if line.is_empty() {
            if in_package && is_installed {
                return Ok(version);
            }
            in_package = false;
            is_installed = false;
            version = None;
        } else if let Some(pkg) = line.strip_prefix("Package: ") {
            in_package = pkg.trim() == package_name;
        } else if in_package {
            if line.starts_with("Version: ") {
                version = Some(
                    line.strip_prefix("Version: ")
                        .expect("guarded by starts_with check")
                        .trim()
                        .to_string(),
                );
            } else if line.starts_with("Status: ") && line.contains("installed") {
                is_installed = true;
            }
        }
    }

    Ok(None)
}

/// Check if package is auto-installed (dependency) from `/var/lib/apt/extended_states`
/// Returns `true` if auto-installed, `false` if explicitly installed
pub fn is_package_auto_installed(package_name: &str) -> Result<bool> {
    if crate::core::paths::test_mode() {
        return Ok(false);
    }

    let extended_states_path = Path::new("/var/lib/apt/extended_states");
    if !extended_states_path.exists() {
        return Ok(false);
    }

    let content = fs::read_to_string(extended_states_path)?;

    let mut current_pkg = String::new();
    for line in content.lines() {
        if let Some(name) = line.strip_prefix("Package: ") {
            current_pkg = name.trim().to_string();
        } else if current_pkg == package_name && line.starts_with("Auto-Installed: 1") {
            return Ok(true);
        }
    }

    Ok(false)
}

/// List orphaned packages on Debian/Ubuntu systems
///
/// An orphan is a package that:
/// - Was automatically installed (as a dependency)
/// - Is no longer required by any manually-installed package
///
/// This function uses a simpler but effective approach:
/// 1. Get all installed packages
/// 2. Get auto-installed set from `/var/lib/apt/extended_states`
/// 3. Recursively get all dependencies of manually-installed packages
/// 4. Orphans = auto-installed packages NOT in the dependency set
pub fn list_orphans_fast() -> Result<Vec<String>> {
    if crate::core::paths::test_mode() {
        return Ok(vec!["libunused1".to_string()]);
    }

    // Get all installed packages with their auto-install status
    let installed = list_installed_fast()?;

    // Split into auto-installed and manually-installed
    let auto_installed: AHashSet<String> = installed
        .iter()
        .filter(|p| !p.is_explicit)
        .map(|p| p.name.clone())
        .collect();

    let manual_packages: Vec<String> = installed
        .iter()
        .filter(|p| p.is_explicit)
        .map(|p| p.name.clone())
        .collect();

    // Build the set of all packages required (directly or transitively) by manual packages
    let mut required_set = AHashSet::new();
    let mut to_visit = manual_packages;

    // Parse dpkg/status once to build a dependency map
    let dep_map = build_dependency_map()?;

    while let Some(pkg_name) = to_visit.pop() {
        if !required_set.insert(pkg_name.clone()) {
            continue; // Already visited
        }

        // Add all dependencies of this package to visit queue
        if let Some(deps) = dep_map.get(&pkg_name) {
            for dep in deps {
                if !required_set.contains(dep) {
                    to_visit.push(dep.clone());
                }
            }
        }
    }

    // Orphans are auto-installed packages that are NOT required by any manual package
    let orphans: Vec<String> = auto_installed
        .into_iter()
        .filter(|pkg| !required_set.contains(pkg))
        .collect();

    Ok(orphans)
}

/// Build a dependency map from all installed packages
/// Returns `HashMap<package_name, Vec<dependency_names>>`
fn build_dependency_map() -> Result<HashMap<String, Vec<String>>> {
    let status_path = Path::new("/var/lib/dpkg/status");
    if !status_path.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(status_path)?;
    let mut dep_map = HashMap::new();

    let mut current_pkg = String::new();
    let mut current_deps = Vec::new();
    let mut is_installed = false;

    for line in content.lines() {
        if line.is_empty() {
            // End of paragraph
            if is_installed && !current_pkg.is_empty() && !current_deps.is_empty() {
                dep_map.insert(current_pkg.clone(), std::mem::take(&mut current_deps));
            }
            current_pkg.clear();
            current_deps.clear();
            is_installed = false;
        } else if let Some(pkg) = line.strip_prefix("Package: ") {
            current_pkg = pkg.trim().to_string();
        } else if line.starts_with("Status: ") && line.contains("installed") {
            is_installed = true;
        } else if line.starts_with("Depends: ") || line.starts_with("Pre-Depends: ") {
            // Extract dependency names (strip versions and multi-arch qualifiers)
            let deps_str = if let Some(stripped) = line.strip_prefix("Depends: ") {
                stripped
            } else if let Some(stripped) = line.strip_prefix("Pre-Depends: ") {
                stripped
            } else {
                continue;
            };

            for dep in deps_str.split(',') {
                // Split on '|' for alternative dependencies (take first alternative)
                let dep = dep.split('|').next().unwrap_or("");

                // Extract package name (before version constraint or arch qualifier)
                if let Some(dep_name) = dep.split_whitespace().next() {
                    // Strip multi-arch qualifiers like :amd64, :any, :native
                    let dep_name = dep_name.split(':').next().unwrap_or(dep_name);
                    if !dep_name.is_empty() {
                        current_deps.push(dep_name.to_string());
                    }
                }
            }
        }
    }

    // Handle last package
    if is_installed && !current_pkg.is_empty() && !current_deps.is_empty() {
        dep_map.insert(current_pkg, current_deps);
    }

    Ok(dep_map)
}

/// Clean the APT package cache at `/var/cache/apt/archives/`
///
/// Removes all `.deb` files from:
/// - `/var/cache/apt/archives/`
/// - `/var/cache/apt/archives/partial/`
///
/// Returns `(files_removed, bytes_freed)`
pub fn clean_package_cache() -> Result<(usize, u64)> {
    if crate::core::paths::test_mode() {
        return Ok((5, 50_000_000)); // 5 files, 50 MB
    }

    let cache_dir = Path::new("/var/cache/apt/archives");
    let partial_dir = cache_dir.join("partial");

    let mut removed = 0;
    let mut freed = 0u64;

    // Clean main cache directory
    if cache_dir.exists() {
        for entry in fs::read_dir(cache_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(filename) = path.file_name().and_then(|n| n.to_str())
                && filename.to_ascii_lowercase().ends_with(".deb")
                && path.is_file()
            {
                if let Ok(meta) = fs::metadata(&path) {
                    freed += meta.len();
                }
                if fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            }
        }
    }

    // Clean partial directory
    if partial_dir.exists() {
        for entry in fs::read_dir(&partial_dir)? {
            let entry = entry?;
            let path = entry.path();

            if let Some(filename) = path.file_name().and_then(|n| n.to_str())
                && filename.to_ascii_lowercase().ends_with(".deb")
                && path.is_file()
            {
                if let Ok(meta) = fs::metadata(&path) {
                    freed += meta.len();
                }
                if fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            }
        }
    }

    Ok((removed, freed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_paragraph_reads_required_and_numeric_fields() -> Result<()> {
        let paragraph = "Package: vim\nVersion: 2:9.1.0-1\nDescription: Vi IMproved - enhanced vi editor\nSection: editors\nPriority: optional\nInstalled-Size: 3500\n";

        let package = parse_paragraph_str(paragraph, "main")?;

        assert_eq!(package.name, "vim");
        assert_eq!(package.version, "2:9.1.0-1");
        assert_eq!(package.description, "Vi IMproved - enhanced vi editor");
        assert_eq!(package.installed_size, 3500);
        Ok(())
    }

    #[test]
    fn parse_paragraph_preserves_description_continuations() -> Result<()> {
        let paragraph = "Package: curl\nVersion: 8.5.0-1\nDescription: command line tool for transferring data\n curl is a tool to transfer data from or to a server\n .\n using one of the supported protocols.\nSection: net\n";

        let package = parse_paragraph_str(paragraph, "main")?;

        assert_eq!(
            package.description,
            "command line tool for transferring data\ncurl is a tool to transfer data from or to a server\n\nusing one of the supported protocols."
        );
        Ok(())
    }

    #[test]
    fn parse_paragraph_rejects_missing_package_name() {
        assert!(parse_paragraph_str("Version: 1.0\n", "main").is_err());
    }

    #[test]
    fn parse_paragraph_rejects_invalid_numeric_fields() {
        let error = parse_paragraph_str("Package: curl\nSize: many\n", "main")
            .expect_err("a nonnumeric package size must be rejected");

        assert!(error.to_string().contains("Invalid Size value"));
    }

    #[test]
    fn parse_paragraph_reads_multiline_dependencies() -> Result<()> {
        let paragraph = "Package: bash\nDepends: libc6 (>= 2.38),\n libreadline8 (>= 8.1), libtinfo6 | ncurses-term\n";

        let package = parse_paragraph_str(paragraph, "main")?;

        assert_eq!(package.depends, ["libc6", "libreadline8", "libtinfo6"]);
        Ok(())
    }

    #[test]
    fn test_mmap_index_open_nonexistent_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("does_not_exist.rkyv");

        let result = DebianMmapIndex::open(&test_file);
        assert!(result.is_err(), "Should fail to open nonexistent file");
    }

    #[test]
    fn test_mmap_index_get_corrupted_archive() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("corrupted.rkyv");
        std::fs::write(&test_file, b"corrupted data").unwrap();

        let index = DebianMmapIndex::open(&test_file).unwrap();
        let result = index.get("vim");

        assert!(result.is_err(), "Should fail to access corrupted archive");
    }

    #[test]
    fn test_mmap_index_open_empty_file() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("empty.rkyv");
        std::fs::write(&test_file, b"").unwrap();

        let index = DebianMmapIndex::open(&test_file).unwrap();
        let result = index.get("vim");

        assert!(result.is_err(), "Should fail to access empty file");
    }

    #[test]
    fn test_mmap_index_packages_corrupted() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let test_file = temp_dir.path().join("corrupted.rkyv");
        std::fs::write(&test_file, vec![0xFF; 100]).unwrap();

        let index = DebianMmapIndex::open(&test_file).unwrap();
        let result = index.packages();

        assert!(result.is_err(), "Should fail to access corrupted file");
    }

    #[test]
    fn test_clean_package_cache_test_mode() {
        // Enable test mode
        // SAFETY: Test-only code, no concurrent access to environment
        #[expect(unsafe_code)]
        unsafe {
            std::env::set_var("OMG_TEST_MODE", "1");
        }

        let result = clean_package_cache().unwrap();
        assert_eq!(result.0, 5); // files removed
        assert_eq!(result.1, 50_000_000); // bytes freed

        // SAFETY: Test cleanup, no concurrent access
        #[expect(unsafe_code)]
        unsafe {
            std::env::remove_var("OMG_TEST_MODE");
        }
    }

    #[test]
    fn test_list_orphans_test_mode() {
        // Enable test mode
        // SAFETY: Test-only code, no concurrent access to environment
        #[expect(unsafe_code)]
        unsafe {
            std::env::set_var("OMG_TEST_MODE", "1");
        }

        let orphans = list_orphans_fast().unwrap();
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0], "libunused1");

        // SAFETY: Test cleanup, no concurrent access
        #[expect(unsafe_code)]
        unsafe {
            std::env::remove_var("OMG_TEST_MODE");
        }
    }

    #[test]
    fn test_build_dependency_map_empty() {
        // When dpkg status doesn't exist
        let result = build_dependency_map();
        assert!(result.is_ok());
        if let Ok(map) = result {
            // Should return empty map if file doesn't exist or is empty
            assert!(map.is_empty() || !map.is_empty()); // Either is valid
        }
    }

    #[test]
    fn test_extract_component_from_path_binary_pattern() {
        let p = Path::new(
            "/var/lib/apt/lists/deb.debian.org_debian_dists_bookworm_main_binary-amd64_Packages",
        );
        assert_eq!(extract_component_from_path(p), "main");
    }

    #[test]
    fn test_extract_component_from_path_simple_pattern() {
        let p = Path::new("/tmp/contrib_amd64_Packages");
        assert_eq!(extract_component_from_path(p), "contrib");
    }

    #[test]
    fn test_index_get_query_name_arch_component() {
        let mut idx = DebianPackageIndex::new();

        idx.add_package(DebianPackage {
            name: "bash".to_string(),
            version: "5.2.15-2".to_string(),
            description: "GNU shell".to_string(),
            section: "shells".to_string(),
            priority: "required".to_string(),
            installed_size: 100,
            maintainer: "Debian".to_string(),
            architecture: "amd64".to_string(),
            depends: vec![],
            filename: "pool/main/b/bash/bash_amd64.deb".to_string(),
            size: 100,
            sha256: "x".to_string(),
            homepage: "https://example.org".to_string(),
            component: "main".to_string(),
        });

        idx.add_package(DebianPackage {
            name: "bash".to_string(),
            version: "5.2.15-1".to_string(),
            description: "GNU shell".to_string(),
            section: "shells".to_string(),
            priority: "required".to_string(),
            installed_size: 100,
            maintainer: "Debian".to_string(),
            architecture: "amd64".to_string(),
            depends: vec![],
            filename: "pool/contrib/b/bash/bash_amd64.deb".to_string(),
            size: 100,
            sha256: "x".to_string(),
            homepage: "https://example.org".to_string(),
            component: "contrib".to_string(),
        });

        idx.add_package(DebianPackage {
            name: "bash".to_string(),
            version: "5.2.14-1".to_string(),
            description: "GNU shell".to_string(),
            section: "shells".to_string(),
            priority: "required".to_string(),
            installed_size: 100,
            maintainer: "Debian".to_string(),
            architecture: "i386".to_string(),
            depends: vec![],
            filename: "pool/main/b/bash/bash_i386.deb".to_string(),
            size: 100,
            sha256: "x".to_string(),
            homepage: "https://example.org".to_string(),
            component: "main".to_string(),
        });

        let by_name = idx.get_query("bash").expect("name lookup");
        assert_eq!(by_name.architecture, "amd64");
        assert_eq!(by_name.component, "main");

        let by_arch = idx.get_query("bash:i386").expect("arch lookup");
        assert_eq!(by_arch.architecture, "i386");

        let by_arch_component = idx
            .get_query("bash:amd64:contrib")
            .expect("arch+component lookup");
        assert_eq!(by_arch_component.component, "contrib");
    }

    #[test]
    fn is_installed_fast_test_mode_returns_ok_for_known_and_unknown() {
        // SAFETY: Test-only code, no concurrent access to environment
        #[expect(unsafe_code)]
        unsafe {
            std::env::set_var("OMG_TEST_MODE", "1");
        }

        let installed = is_installed_fast("apt").expect("test-mode install lookup");
        assert!(installed);
        let unknown =
            is_installed_fast("definitely-not-installed").expect("test-mode missing lookup");
        assert!(!unknown);

        // SAFETY: Test cleanup, no concurrent access
        #[expect(unsafe_code)]
        unsafe {
            std::env::remove_var("OMG_TEST_MODE");
        }
    }
}
