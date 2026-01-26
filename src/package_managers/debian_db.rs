//! Pure Rust Debian/Ubuntu Database Parser - ULTRA FAST
//!
//! Parses /var/lib/apt/lists/*_Packages and /var/lib/dpkg/status files directly
//! and provides a high-performance index with zero-copy deserialization via rkyv.

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::LazyLock;

use ahash::{AHashMap, AHashSet};
use anyhow::{Context, Result};
use memchr::memmem;
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

#[derive(Default)]
struct DebianIndexCache {
    index: Option<DebianPackageIndex>,
    last_modified: Option<std::time::SystemTime>,
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

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Default)]
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

    let newest_mtime = get_newest_mtime(lists_dir);
    {
        let cache = DEBIAN_INDEX_CACHE.read();
        if cache.last_modified == Some(newest_mtime) && cache.index.is_some() {
            return Ok(());
        }
    }

    let mut index = None;
    let cache_path = paths::cache_dir().join("debian_index_v4.bin");

    // SAFE: Read file into memory and use rkyv for zero-allocation access
    if cache_path.exists()
        && let Ok(bytes) = fs::read(&cache_path)
        && let Ok(idx) = rkyv::from_bytes::<DebianPackageIndex, rkyv::rancor::Error>(&bytes)
    {
        index = Some(idx);
    }

    if index.is_none() {
        index = Some(rebuild_index()?);
    }

    if let Some(idx) = index {
        let mut search_buffer = Vec::new();
        let mut package_offsets = Vec::with_capacity(idx.packages.len() + 1);

        for pkg in &idx.packages {
            package_offsets.push(search_buffer.len());
            search_buffer.extend_from_slice(pkg.name.as_bytes());
            search_buffer.push(b' ');
            search_buffer.extend_from_slice(pkg.description.as_bytes());
            search_buffer.push(0);
        }
        package_offsets.push(search_buffer.len());

        let installed_set = list_installed_fast()
            .unwrap_or_default()
            .into_iter()
            .map(|p| p.name)
            .collect();

        let mut cache = DEBIAN_INDEX_CACHE.write();
        cache.index = Some(idx);
        cache.last_modified = Some(newest_mtime);
        cache.search_buffer = search_buffer;
        cache.package_offsets = package_offsets;
        cache.installed_set = installed_set;
    }
    Ok(())
}

fn get_newest_mtime(dir: &Path) -> std::time::SystemTime {
    fs::read_dir(dir)
        .ok()
        .and_then(|entries| {
            entries
                .flatten()
                .filter_map(|e| e.metadata().ok()?.modified().ok())
                .max()
        })
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
}

fn rebuild_index() -> Result<DebianPackageIndex> {
    let lists_dir = Path::new("/var/lib/apt/lists");
    let mut new_index = DebianPackageIndex::new();
    new_index.updated_at = jiff::Timestamp::now().as_second();

    let mut pkg_files = Vec::new();
    if let Ok(entries) = fs::read_dir(lists_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if filename.contains("_Packages")
                && !path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("diff"))
            {
                pkg_files.push(path);
            }
        }
    }

    let all_packages: Vec<DebianPackage> = pkg_files
        .par_iter()
        .map(|path| parse_packages_file_sync(path))
        .collect::<Result<Vec<Vec<DebianPackage>>>>()?
        .into_iter()
        .flatten()
        .collect();

    for pkg in all_packages {
        new_index.add_package(pkg);
    }

    let cache_path = paths::cache_dir().join("debian_index_v4.bin");
    if let Some(p) = cache_path.parent() {
        let _ = fs::create_dir_all(p);
    }
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&new_index)
        .map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;
    fs::write(&cache_path, bytes)?;

    Ok(new_index)
}

fn parse_packages_file_sync(path: &Path) -> Result<Vec<DebianPackage>> {
    let file = fs::File::open(path)?;
    let reader = BufReader::new(file);

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
        let mut buf = String::new();
        fs::File::open(path)?.read_to_string(&mut buf)?;
        buf
    };

    let mut packages = Vec::new();
    for paragraph in content.split("\n\n") {
        if paragraph.trim().is_empty() {
            continue;
        }
        if let Ok(pkg) = parse_paragraph_str(paragraph) {
            packages.push(pkg);
        }
    }
    Ok(packages)
}

fn parse_paragraph_str(paragraph: &str) -> Result<DebianPackage> {
    let mut name = String::new();
    let mut version = String::new();
    let mut description = String::new();
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

    for line in paragraph.lines() {
        if line.starts_with(' ') || line.starts_with('\t') {
            if !description.is_empty() {
                description.push('\n');
                description.push_str(line.trim());
            }
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            match key {
                "Package" => name = value.to_string(),
                "Version" => version = value.to_string(),
                "Description" => description = value.to_string(),
                "Section" => section = value.to_string(),
                "Priority" => priority = value.to_string(),
                "Installed-Size" => installed_size = value.parse().unwrap_or(0),
                "Maintainer" => maintainer = value.to_string(),
                "Architecture" => architecture = value.to_string(),
                "Depends" => {
                    depends = value
                        .split(',')
                        .map(|d| d.split_whitespace().next().unwrap_or("").to_string())
                        .collect();
                }
                "Filename" => filename = value.to_string(),
                "Size" => size = value.parse().unwrap_or(0),
                "SHA256" => sha256 = value.to_string(),
                "Homepage" => homepage = value.to_string(),
                _ => {}
            }
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

    let query_lower = query.to_lowercase();
    let finder = memmem::Finder::new(query_lower.as_bytes());
    let mut results = Vec::new();
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
            results.push(p);
        }
        if results.len() >= 100 {
            break;
        }
    }
    Ok(results)
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

    let mut auto_installed = AHashSet::new();
    if let Ok(ext_content) = fs::read_to_string(extended_states_path) {
        let mut current_pkg = String::new();
        for line in ext_content.lines() {
            if let Some(name) = line.strip_prefix("Package: ") {
                current_pkg = name.trim().to_string();
            } else if line.starts_with("Auto-Installed: 1") && !current_pkg.is_empty() {
                auto_installed.insert(current_pkg.clone());
            }
        }
    }

    let mut packages = Vec::new();
    let mut installed_set = AHashSet::new();
    for paragraph in status_content.split("\n\n") {
        if !paragraph.contains("Status: install ok installed") {
            continue;
        }
        let mut name = String::new();
        let mut version = String::new();
        let mut description = String::new();
        let mut arch = String::new();
        for line in paragraph.lines() {
            if let Some((key, value)) = line.split_once(':') {
                let value = value.trim();
                match key.trim() {
                    "Package" => name = value.to_string(),
                    "Version" => version = value.to_string(),
                    "Description" => description = value.to_string(),
                    "Architecture" => arch = value.to_string(),
                    _ => {}
                }
            }
        }
        if !name.is_empty() {
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
        let mut cache = DPKG_STATUS_CACHE.write();
        cache.packages = packages.clone();
        cache.installed_set = installed_set;
        cache.status_mtime = status_mtime;
        cache.extended_states_mtime = extended_states_mtime;
    }

    Ok(packages)
}

pub fn is_installed_fast(name: &str) -> bool {
    if crate::core::paths::test_mode() {
        return name == "apt" || name == "git";
    }
    let guard = DEBIAN_INDEX_CACHE.read();
    if !guard.installed_set.is_empty() {
        return guard.installed_set.contains(name);
    }
    if let Ok(content) = fs::read_to_string("/var/lib/dpkg/status") {
        let pattern = format!("Package: {name}\n");
        if let Some(pos) = content.find(&pattern) {
            let end = (pos + 500).min(content.len());
            let chunk = &content[pos..end];
            return chunk.contains("Status: install ok installed");
        }
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
