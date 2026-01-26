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

use ahash::AHashSet;
use anyhow::{Context, Result};
use memchr::memmem;
use memmap2::Mmap;
use parking_lot::RwLock;
use rayon::prelude::*;

use crate::core::paths;
use crate::core::{Package, PackageSource};

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
}

/// Cache for /var/lib/dpkg/status to avoid expensive reparsing
#[derive(Default)]
struct DpkgStatusCache {
    packages: Vec<LocalPackage>,
    installed_set: AHashSet<String>,
    status_mtime: Option<std::time::SystemTime>,
    extended_states_mtime: Option<std::time::SystemTime>,
}

/// Global mmap-based index for zero-copy access (optional, used when available)
static DEBIAN_MMAP_INDEX: LazyLock<RwLock<Option<DebianMmapIndex>>> =
    LazyLock::new(|| RwLock::new(None));

/// Zero-copy memory-mapped Debian package index
/// Provides sub-millisecond access to package metadata without deserialization
pub struct DebianMmapIndex {
    mmap: Mmap,
}

impl DebianMmapIndex {
    /// Open an existing index using memory mapping
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open mmap index at {}", path.display()))?;
        // SAFETY: File is read-only and we control the format
        let mmap = unsafe { Mmap::map(&file)? };
        Ok(Self { mmap })
    }

    /// Access the archived data with zero-copy
    #[inline]
    fn archive(&self) -> &rkyv::Archived<DebianPackageIndex> {
        // SAFETY: Index was created by our serializer, format is stable
        unsafe { rkyv::access_unchecked::<rkyv::Archived<DebianPackageIndex>>(&self.mmap) }
    }

    /// Get a package by name (zero-copy, O(1) via hash lookup in archived data)
    pub fn get(&self, name: &str) -> Option<&rkyv::Archived<DebianPackage>> {
        let archive = self.archive();
        let idx = archive.name_to_idx.get(name)?;
        // Convert archived u32 to native usize
        let idx = u32::from(*idx) as usize;
        archive.packages.get(idx)
    }

    /// Get all packages (zero-copy reference)
    pub fn packages(&self) -> &rkyv::vec::ArchivedVec<rkyv::Archived<DebianPackage>> {
        &self.archive().packages
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
    /// Note: Uses std HashMap for rkyv serialization compatibility
    /// Converted to AHashMap at runtime for faster lookups
    pub name_to_idx: HashMap<String, usize>,
    pub updated_at: i64,
}

impl DebianPackageIndex {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn add_package(&mut self, pkg: DebianPackage) {
        let idx = self.packages.len();
        self.name_to_idx.insert(pkg.name.clone(), idx);
        self.packages.push(pkg);
    }
    pub fn get(&self, name: &str) -> Option<&DebianPackage> {
        self.name_to_idx.get(name).map(|&idx| &self.packages[idx])
    }
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
            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.contains("_Packages")
                    && !path
                        .extension()
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("diff"))
                {
                    if let Ok(meta) = entry.metadata() {
                        if let Ok(mtime) = meta.modified() {
                            current_files.insert(path, mtime);
                        }
                    }
                }
            }
        }
    }

    // Check if we need to update
    let needs_update = {
        let cache = DEBIAN_INDEX_CACHE.read();
        if cache.index.is_none() {
            true // No index yet
        } else {
            // Check if any files changed or were added/removed
            cache.file_mtimes != current_files
        }
    };

    if !needs_update {
        return Ok(());
    }

    // Determine which files changed
    let (changed_files, mut index): (Vec<PathBuf>, Option<DebianPackageIndex>) = {
        let cache = DEBIAN_INDEX_CACHE.read();
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
            (current_files.keys().cloned().collect::<Vec<PathBuf>>(), None)
        }
    };

    // Load or create index (with LZ4 compression support)
    let cache_path = paths::cache_dir().join("debian_index_v5.lz4");
    let mmap_path = paths::cache_dir().join("debian_index_v5.mmap");

    if index.is_none() && cache_path.exists() {
        if let Ok(compressed) = fs::read(&cache_path) {
            // Decompress LZ4
            if let Ok(bytes) = lz4_flex::decompress_size_prepended(&compressed) {
                if let Ok(idx) = rkyv::from_bytes::<DebianPackageIndex, rkyv::rancor::Error>(&bytes) {
                    index = Some(idx);
                }
            }
        }
    }

    // Try to load the mmap index for zero-copy access
    if mmap_path.exists() {
        let mut mmap_guard = DEBIAN_MMAP_INDEX.write();
        if mmap_guard.is_none() {
            if let Ok(mmap_index) = DebianMmapIndex::open(&mmap_path) {
                *mmap_guard = Some(mmap_index);
            }
        }
    }

    let mut index = index.unwrap_or_else(DebianPackageIndex::new);

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

        // Add all packages
        for pkg in new_packages {
            index.add_package(pkg);
        }

        // Update timestamp and save
        index.updated_at = jiff::Timestamp::now().as_second();
        if let Some(p) = cache_path.parent() {
            let _ = fs::create_dir_all(p);
        }
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&index)
            .map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;

        // Save compressed version for space efficiency
        let compressed = lz4_flex::compress_prepend_size(&bytes);
        fs::write(&cache_path, compressed)?;

        // Also save uncompressed version for zero-copy mmap access
        let mmap_path = paths::cache_dir().join("debian_index_v5.mmap");
        fs::write(&mmap_path, &bytes)?;

        // Load the mmap index for zero-copy access
        if let Ok(mmap_index) = DebianMmapIndex::open(&mmap_path) {
            let mut mmap_guard = DEBIAN_MMAP_INDEX.write();
            *mmap_guard = Some(mmap_index);
        }
    }

    // Rebuild search buffer with pre-calculated capacity
    // IMPORTANT: Store lowercased content for case-insensitive SIMD search
    let estimated_size: usize = index.packages.iter()
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

    let newest_mtime = current_files.values().max().copied()
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

    let mut cache = DEBIAN_INDEX_CACHE.write();
    cache.index = Some(index);
    cache.last_modified = Some(newest_mtime);
    cache.file_mtimes = current_files;
    cache.search_buffer = search_buffer;
    cache.package_offsets = package_offsets;
    cache.installed_set = installed_set;

    Ok(())
}

fn parse_packages_file_sync(path: &Path) -> Result<Vec<DebianPackage>> {
    let file = fs::File::open(path)?;
    // Use 64KB buffer instead of default 8KB for fewer syscalls
    let reader = BufReader::with_capacity(64 * 1024, file);

    let content = if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("lz4"))
    {
        let mut decoder = lz4_flex::frame::FrameDecoder::new(reader);
        let mut buf = String::new();
        decoder.read_to_string(&mut buf)?;
        buf
    } else if path
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gz"))
    {
        let mut decoder = flate2::read::GzDecoder::new(reader);
        let mut buf = String::new();
        decoder.read_to_string(&mut buf)?;
        buf
    } else {
        // Use the already-opened buffered reader
        let mut buf = String::new();
        reader.into_inner().read_to_string(&mut buf)?;
        buf
    };

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
                    parse_paragraph_str(paragraph).ok()
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
                    parse_paragraph_str(paragraph).ok()
                }
            })
            .collect()
    };

    Ok(packages)
}

#[inline]
fn parse_paragraph_str(paragraph: &str) -> Result<DebianPackage> {
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

    let mut lines = paragraph.lines();
    while let Some(line) = lines.next() {
        // Handle continuation lines (multi-line descriptions)
        if line.starts_with(' ') || line.starts_with('\t') {
            if !description.is_empty() {
                description.push('\n');
                description.push_str(line.trim_start());
            }
            continue;
        }

        // Fast path: SIMD-accelerated colon search
        let Some(colon_pos) = memchr::memchr(b':', line.as_bytes()) else {
            continue;
        };

        let key = &line[..colon_pos];
        let value = line[colon_pos + 1..].trim_start();

        // Use match on byte slice for faster comparison
        match key.as_bytes() {
            b"Package" => name = value.to_string(),
            b"Version" => version = value.to_string(),
            b"Description" => description = value.to_string(),
            b"Section" => section = value.to_string(),
            b"Priority" => priority = value.to_string(),
            b"Installed-Size" => installed_size = value.parse().unwrap_or(0),
            b"Maintainer" => maintainer = value.to_string(),
            b"Architecture" => architecture = value.to_string(),
            b"Depends" => {
                // Optimized depends parsing - pre-allocate and avoid intermediate allocations
                depends.reserve(value.matches(',').count() + 1);
                for dep in value.split(',') {
                    if let Some(pkg) = dep.split_whitespace().next() {
                        depends.push(pkg.to_string());
                    }
                }
            }
            b"Filename" => filename = value.to_string(),
            b"Size" => size = value.parse().unwrap_or(0),
            b"SHA256" => sha256 = value.to_string(),
            b"Homepage" => homepage = value.to_string(),
            _ => {}
        }
    }

    if name.is_empty() {
        anyhow::bail!("Invalid");
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
        }]);
    }
    ensure_index_loaded()?;
    let guard = DEBIAN_INDEX_CACHE.read();
    let index = guard.index.as_ref().context("Index not loaded")?;
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
    ensure_index_loaded()?;
    let guard = DEBIAN_INDEX_CACHE.read();
    let index = guard.index.as_ref().context("Index not loaded")?;

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
    // This optimizes common operations like "apt install package-name"
    if let Some(exact_pkg) = index.get(query) {
        let mut p = exact_pkg.to_package();
        p.installed = guard.installed_set.contains(&p.name);
        return Ok(vec![p]);
    }

    // Also check lowercase version for case-insensitive exact match
    let query_lower = query.to_lowercase();
    if query_lower != query {
        if let Some(exact_pkg) = index.get(&query_lower) {
            let mut p = exact_pkg.to_package();
            p.installed = guard.installed_set.contains(&p.name);
            return Ok(vec![p]);
        }
    }

    // Slow path: fuzzy search using SIMD memchr
    let finder = memmem::Finder::new(query_lower.as_bytes());
    let mut exact_matches = Vec::new();
    let mut prefix_matches = Vec::new();
    let mut substring_matches = Vec::new();
    let mut seen_indices = AHashSet::new();

    for match_idx in finder.find_iter(&guard.search_buffer) {
        let pkg_idx = match guard.package_offsets.binary_search(&match_idx) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        if seen_indices.insert(pkg_idx)
            && let Some(pkg) = index.packages.get(pkg_idx)
        {
            let mut p = pkg.to_package();
            p.installed = guard.installed_set.contains(&p.name);

            // Categorize by match type for better relevance
            let name_lower = p.name.to_lowercase();
            if name_lower == query_lower {
                exact_matches.push(p);
            } else if name_lower.starts_with(&query_lower) {
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
    Ok(exact_matches)
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
    ensure_index_loaded()?;
    let guard = DEBIAN_INDEX_CACHE.read();
    let index = guard.index.as_ref().context("Index not loaded")?;
    if let Some(pkg) = index.get(name) {
        let mut p = pkg.to_package();
        p.installed = guard.installed_set.contains(name);
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

/// Parse a dpkg status paragraph into LocalPackage fields
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
    let status_mtime = fs::metadata(status_path).ok().and_then(|m| m.modified().ok());
    let extended_states_mtime = extended_states_path
        .exists()
        .then(|| fs::metadata(extended_states_path).ok().and_then(|m| m.modified().ok()))
        .flatten();

    // Check cache first
    {
        let cache = DPKG_STATUS_CACHE.read();
        if cache.status_mtime == status_mtime
            && cache.extended_states_mtime == extended_states_mtime
            && !cache.packages.is_empty()
        {
            // Cache hit!
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
        if STATUS_INSTALLED_FINDER.find(paragraph.as_bytes()).is_some() {
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
    }

    // Update cache
    {
        let mut cache = DPKG_STATUS_CACHE.write();
        cache.packages = packages.clone();
        cache.installed_set = installed_set;
        cache.status_mtime = status_mtime;
        cache.extended_states_mtime = extended_states_mtime;
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

    let cache = DPKG_STATUS_CACHE.read();
    Ok(cache.packages.iter().find(|p| p.name == name).cloned())
}

#[inline]
pub fn is_installed_fast(name: &str) -> bool {
    if crate::core::paths::test_mode() {
        return name == "apt" || name == "git";
    }

    // Check dpkg status cache first for O(1) lookup
    {
        let cache = DPKG_STATUS_CACHE.read();
        if !cache.installed_set.is_empty() {
            return cache.installed_set.contains(name);
        }
    }

    // Fallback: populate cache by calling list_installed_fast
    if list_installed_fast().is_ok() {
        let cache = DPKG_STATUS_CACHE.read();
        return cache.installed_set.contains(name);
    }

    false
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

#[cfg(test)]
mod tests {

    use super::*;

    #[test]

    fn test_parse_paragraph_basic() {
        let para = "Package: vim\nVersion: 2:9.1.0-1\nDescription: Vi IMproved - enhanced vi editor\nSection: editors\nPriority: optional\nInstalled-Size: 3500\n";

        let pkg = parse_paragraph_str(para).unwrap();

        assert_eq!(pkg.name, "vim");

        assert_eq!(pkg.version, "2:9.1.0-1");

        assert_eq!(pkg.description, "Vi IMproved - enhanced vi editor");

        assert_eq!(pkg.installed_size, 3500);
    }

    #[test]

    fn test_parse_paragraph_multiline_desc() {
        let para = "Package: curl\nVersion: 8.5.0-1\nDescription: command line tool for transferring data\n curl is a tool to transfer data from or to a server\n using one of the supported protocols.\nSection: net\n";

        let pkg = parse_paragraph_str(para).unwrap();

        assert_eq!(pkg.name, "curl");

        assert!(pkg.description.contains("curl is a tool"));

        assert!(pkg.description.contains("supported protocols."));
    }

    #[test]

    fn test_parse_paragraph_invalid() {
        let para = "Version: 1.0\n"; // Missing name

        assert!(parse_paragraph_str(para).is_err());
    }

    #[test]

    fn test_parse_paragraph_with_depends() {
        let para = "Package: bash\nDepends: libc6 (>= 2.38), libreadline8 (>= 8.1)\n";

        let pkg = parse_paragraph_str(para).unwrap();

        assert_eq!(pkg.name, "bash");

        assert_eq!(pkg.depends.len(), 2);

        assert_eq!(pkg.depends[0], "libc6");

        assert_eq!(pkg.depends[1], "libreadline8");
    }
}
