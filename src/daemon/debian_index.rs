//! Debian/Ubuntu Package Index with Zero-Copy Deserialization
//!
//! This module provides a high-performance package index for Debian-based systems
//! using rkyv for zero-copy deserialization and nucleo for SIMD-accelerated fuzzy search.
//!
//! Performance targets:
//! - Search: <5ms for 100k packages (50x faster than Nala)
//! - Info lookup: <1ms (100x faster than apt-cache)
//! - Index load: <100ms from disk cache

#![cfg(feature = "debian")]

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Result;
use parking_lot::RwLock;
use rkyv::{Archive, Deserialize, Serialize};

use crate::core::{Package, PackageSource};

/// A Debian package entry optimized for zero-copy access
#[derive(Archive, Serialize, Deserialize, Debug, Clone)]
#[rkyv(compare(PartialEq), derive(Debug))]
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
    pub recommends: Vec<String>,
    pub suggests: Vec<String>,
    pub filename: String,
    pub size: u64,
    pub sha256: String,
}

impl DebianPackage {
    /// Convert to the common Package type
    pub fn to_package(&self) -> Package {
        Package {
            name: self.name.clone(),
            version: self.version.clone(),
            description: self.description.clone(),
            source: PackageSource::Official,
            installed: false,
        }
    }
}

/// The main package index with fast lookup structures
#[derive(Archive, Serialize, Deserialize, Debug)]
#[rkyv(derive(Debug))]
pub struct DebianPackageIndex {
    /// All packages in the index
    pub packages: Vec<DebianPackage>,
    /// Name to index mapping for O(1) lookup
    pub name_to_idx: HashMap<String, usize>,
    /// Package names for fuzzy search (pre-extracted for nucleo)
    pub package_names: Vec<String>,
    /// Index version for cache invalidation
    pub version: u64,
    /// Timestamp of last update
    pub updated_at: i64,
}

impl DebianPackageIndex {
    /// Create a new empty index
    pub fn new() -> Self {
        Self {
            packages: Vec::new(),
            name_to_idx: HashMap::new(),
            package_names: Vec::new(),
            version: 1,
            updated_at: 0,
        }
    }

    /// Get a package by name (O(1) lookup)
    pub fn get(&self, name: &str) -> Option<&DebianPackage> {
        self.name_to_idx.get(name).map(|&idx| &self.packages[idx])
    }

    /// Get total package count
    pub fn len(&self) -> usize {
        self.packages.len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.packages.is_empty()
    }

    /// Add a package to the index
    pub fn add_package(&mut self, pkg: DebianPackage) {
        let idx = self.packages.len();
        self.package_names.push(pkg.name.clone());
        self.name_to_idx.insert(pkg.name.clone(), idx);
        self.packages.push(pkg);
    }
}

impl Default for DebianPackageIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe wrapper around the package index
pub struct DebianIndexState {
    index: Arc<RwLock<DebianPackageIndex>>,
    cache_path: PathBuf,
}

impl DebianIndexState {
    /// Create a new index state
    pub fn new(cache_dir: &Path) -> Self {
        Self {
            index: Arc::new(RwLock::new(DebianPackageIndex::new())),
            cache_path: cache_dir.join("debian_index.rkyv"),
        }
    }

    /// Load index from cache or build from scratch
    pub fn load_or_build(&self) -> Result<()> {
        // Try to load from cache first
        if self.cache_path.exists() {
            if let Ok(()) = self.load_from_cache() {
                tracing::info!("Loaded Debian index from cache");
                return Ok(());
            }
        }

        // Build from Packages files
        self.rebuild_index()?;
        Ok(())
    }

    /// Load index from rkyv cache file
    fn load_from_cache(&self) -> Result<()> {
        let bytes = fs::read(&self.cache_path)?;

        // Use rkyv 0.8's from_bytes API for safe deserialization
        let index: DebianPackageIndex =
            rkyv::from_bytes::<DebianPackageIndex, rkyv::rancor::Error>(&bytes)
                .map_err(|e| anyhow::anyhow!("Failed to deserialize index: {e}"))?;

        let mut guard = self.index.write();
        *guard = index;
        Ok(())
    }

    /// Save index to rkyv cache file
    fn save_to_cache(&self) -> Result<()> {
        let guard = self.index.read();
        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&*guard)?;

        // Ensure cache directory exists
        if let Some(parent) = self.cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&self.cache_path, &bytes)?;
        tracing::info!("Saved Debian index to cache ({} bytes)", bytes.len());
        Ok(())
    }

    /// Rebuild index from /var/lib/apt/lists/*_Packages files
    pub fn rebuild_index(&self) -> Result<()> {
        use debian_packaging::repository::release::ReleaseFile;

        let lists_dir = Path::new("/var/lib/apt/lists");
        if !lists_dir.exists() {
            anyhow::bail!("APT lists directory not found: {:?}", lists_dir);
        }

        let mut new_index = DebianPackageIndex::new();
        new_index.updated_at = jiff::Timestamp::now().as_second();

        // Find all Packages files
        for entry in fs::read_dir(lists_dir)? {
            let entry = entry?;
            let path = entry.path();
            let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            // Match *_Packages files (uncompressed)
            if filename.ends_with("_Packages") && !filename.ends_with(".gz") {
                if let Err(e) = self.parse_packages_file(&path, &mut new_index) {
                    tracing::warn!("Failed to parse {:?}: {}", path, e);
                }
            }
        }

        tracing::info!(
            "Built Debian index with {} packages",
            new_index.packages.len()
        );

        // Update the index
        {
            let mut guard = self.index.write();
            *guard = new_index;
        }

        // Save to cache
        self.save_to_cache()?;

        Ok(())
    }

    /// Parse a single Packages file using debian-packaging crate
    fn parse_packages_file(&self, path: &Path, index: &mut DebianPackageIndex) -> Result<()> {
        use debian_packaging::control::ControlParagraph;

        let content = fs::read_to_string(path)?;

        // Parse control file paragraphs
        for paragraph in content.split("\n\n") {
            if paragraph.trim().is_empty() {
                continue;
            }

            // Parse each paragraph as a control file
            if let Ok(pkg) = self.parse_package_paragraph(paragraph) {
                index.add_package(pkg);
            }
        }

        Ok(())
    }

    /// Parse a single package paragraph from a Packages file
    fn parse_package_paragraph(&self, paragraph: &str) -> Result<DebianPackage> {
        let mut name = String::new();
        let mut version = String::new();
        let mut description = String::new();
        let mut section = String::new();
        let mut priority = String::new();
        let mut installed_size = 0u64;
        let mut maintainer = String::new();
        let mut architecture = String::new();
        let mut depends = Vec::new();
        let mut recommends = Vec::new();
        let mut suggests = Vec::new();
        let mut filename = String::new();
        let mut size = 0u64;
        let mut sha256 = String::new();

        for line in paragraph.lines() {
            if line.starts_with(' ') || line.starts_with('\t') {
                // Continuation of previous field (usually Description)
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
                    "Depends" => depends = Self::parse_deps(value),
                    "Recommends" => recommends = Self::parse_deps(value),
                    "Suggests" => suggests = Self::parse_deps(value),
                    "Filename" => filename = value.to_string(),
                    "Size" => size = value.parse().unwrap_or(0),
                    "SHA256" => sha256 = value.to_string(),
                    _ => {}
                }
            }
        }

        if name.is_empty() {
            anyhow::bail!("Package has no name");
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
            recommends,
            suggests,
            filename,
            size,
            sha256,
        })
    }

    /// Parse dependency string into list
    fn parse_deps(deps: &str) -> Vec<String> {
        deps.split(',')
            .map(|d| {
                // Extract just the package name (before any version constraint)
                d.trim().split_whitespace().next().unwrap_or("").to_string()
            })
            .filter(|d| !d.is_empty())
            .collect()
    }

    /// Search packages using nucleo fuzzy matcher
    pub fn search(&self, query: &str, limit: usize) -> Vec<Package> {
        use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
        use nucleo_matcher::{Config, Matcher};

        let guard = self.index.read();

        if query.is_empty() {
            return guard
                .packages
                .iter()
                .take(limit)
                .map(DebianPackage::to_package)
                .collect();
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Smart, Normalization::Smart);

        let mut results: Vec<(u32, usize)> = guard
            .package_names
            .iter()
            .enumerate()
            .filter_map(|(idx, name)| {
                let mut buf = Vec::new();
                pattern
                    .score(nucleo_matcher::Utf32Str::new(name, &mut buf), &mut matcher)
                    .map(|score| (score, idx))
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.0.cmp(&a.0));

        results
            .into_iter()
            .take(limit)
            .map(|(_, idx)| guard.packages[idx].to_package())
            .collect()
    }

    /// Get package info by exact name
    pub fn get_info(&self, name: &str) -> Option<Package> {
        let guard = self.index.read();
        guard.get(name).map(DebianPackage::to_package)
    }

    /// Get package count
    pub fn package_count(&self) -> usize {
        self.index.read().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_deps() {
        let deps = "libc6 (>= 2.17), libgcc-s1 (>= 3.0), libstdc++6 (>= 5.2)";
        let parsed = DebianIndexState::parse_deps(deps);
        assert_eq!(parsed, vec!["libc6", "libgcc-s1", "libstdc++6"]);
    }

    #[test]
    fn test_empty_index() {
        let index = DebianPackageIndex::new();
        assert!(index.is_empty());
        assert_eq!(index.len(), 0);
    }

    #[test]
    fn test_add_package() {
        let mut index = DebianPackageIndex::new();
        index.add_package(DebianPackage {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            description: "A test package".to_string(),
            section: "utils".to_string(),
            priority: "optional".to_string(),
            installed_size: 1024,
            maintainer: "Test <test@example.com>".to_string(),
            architecture: "amd64".to_string(),
            depends: vec!["libc6".to_string()],
            recommends: vec![],
            suggests: vec![],
            filename: "pool/main/t/test-pkg/test-pkg_1.0.0_amd64.deb".to_string(),
            size: 2048,
            sha256: "abc123".to_string(),
        });

        assert_eq!(index.len(), 1);
        assert!(index.get("test-pkg").is_some());
        assert!(index.get("nonexistent").is_none());
    }
}
