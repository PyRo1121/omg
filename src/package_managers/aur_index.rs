//! Persistent rkyv-based index for AUR metadata
//!
//! This module provides a fast, zero-copy binary index for AUR package metadata,
//! allowing sub-millisecond lookups by memory mapping the index file.

use std::fs::File;
use std::io::{BufReader, Write as _};
use std::path::Path;

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use memmap2::Mmap;
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::Deserialize;
use tempfile::NamedTempFile;

use crate::package_managers::parse_version_or_zero;

/// Minimal AUR package metadata stored in the index.
/// Using rkyv for zero-copy deserialization.
#[derive(Archive, RkyvDeserialize, RkyvSerialize, Debug, Clone, PartialEq)]
pub struct AurEntry {
    pub name: String,
    pub version: String,
    pub maintainer: Option<String>,
    pub last_modified: Option<i64>,
    pub description: Option<String>,
    pub num_votes: i32,
    pub popularity: f64,
    pub out_of_date: Option<i64>,
}

/// The root of the rkyv archive
#[derive(Archive, RkyvDeserialize, RkyvSerialize, Debug)]
pub struct AurArchive {
    /// Entries sorted by name for binary search
    pub entries: Vec<AurEntry>,
}

pub struct AurIndex {
    mmap: Mmap,
}

impl AurIndex {
    /// Open an existing AUR index using memory mapping
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open index at {}", path.display()))?;

        // SAFETY: Memory mapping requires unsafe but is sound here:
        // - File is opened read-only, preventing modification
        // - Mmap maintains exclusive ownership of the file handle
        // - rkyv validation (in archive()) ensures data integrity
        // - No concurrent mutations possible (read-only file descriptor)
        // Alternative considered: Read entire file into memory would be slower
        // and use more RAM for large AUR archives (>100MB)
        let mmap = unsafe { Mmap::map(&file)? };

        Ok(Self { mmap })
    }

    /// Access the archived data with validation
    fn archive(&self) -> Result<&ArchivedAurArchive> {
        rkyv::access::<rkyv::Archived<AurArchive>, rkyv::rancor::Error>(&self.mmap)
            .map_err(|e| anyhow::anyhow!("Corrupted AUR index: {e}"))
    }

    /// Check if a package exists in the index
    #[allow(dead_code)] // Struct fields used by daemon indexer; deserialized at runtime
    pub fn contains(&self, name: &str) -> Result<bool> {
        Ok(self.get(name)?.is_some())
    }

    /// Get metadata for a specific package (zero-copy)
    ///
    /// Returns a reference to the archived entry in the memory-mapped file.
    pub fn get(&self, name: &str) -> Result<Option<&ArchivedAurEntry>> {
        let archive = self.archive()?;
        let Ok(idx) = archive
            .entries
            .binary_search_by_key(&name, |e: &ArchivedAurEntry| e.name.as_str())
        else {
            return Ok(None);
        };
        Ok(Some(&archive.entries[idx]))
    }

    /// Search for packages matching a query (substring match in name or description)
    #[allow(clippy::map_unwrap_or)] // Readability: map().unwrap_or() is explicit about the transformation
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<&ArchivedAurEntry>> {
        let archive = self.archive()?;
        let query = query.to_lowercase();

        Ok(archive
            .entries
            .iter()
            .filter(|e: &&ArchivedAurEntry| {
                e.name.as_str().to_lowercase().contains(&query)
                    || e.description
                        .as_ref()
                        .map(|d: &rkyv::string::ArchivedString| {
                            d.as_str().to_lowercase().contains(&query)
                        })
                        .unwrap_or(false)
            })
            .take(limit)
            .collect())
    }

    /// Batch update check
    pub fn get_updates(
        &self,
        local_pkgs: &[(String, alpm_types::Version)],
    ) -> Result<Vec<(String, alpm_types::Version, alpm_types::Version)>> {
        let mut updates = Vec::new();
        let archive = self.archive()?;

        for (name, local_version) in local_pkgs {
            if let Ok(idx) = archive
                .entries
                .binary_search_by_key(&name.as_str(), |e: &ArchivedAurEntry| e.name.as_str())
            {
                let entry = &archive.entries[idx];
                let remote_version = parse_version_or_zero(entry.version.as_str());
                if remote_version > *local_version {
                    updates.push((name.clone(), local_version.clone(), remote_version));
                }
            }
        }

        Ok(updates)
    }
}

/// Helper struct for parsing the raw AUR JSON (which has Capitalized keys)
#[derive(Deserialize)]
struct RawAurPackage {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Maintainer")]
    maintainer: Option<String>,
    #[serde(rename = "LastModified")]
    last_modified: Option<i64>,
    #[serde(rename = "Description")]
    description: Option<String>,
    #[serde(rename = "NumVotes")]
    num_votes: Option<i32>,
    #[serde(rename = "Popularity")]
    popularity: Option<f64>,
    #[serde(rename = "OutOfDate")]
    out_of_date: Option<i64>,
}

/// Build the binary index from the AUR JSON archive
pub fn build_index(json_path: &Path, output_path: &Path) -> Result<()> {
    let file = File::open(json_path).context("Failed to open AUR JSON")?;
    let reader = BufReader::new(file);
    let decoder = GzDecoder::new(reader);

    // Parse the JSON array. AUR's metadata is a large array of objects.
    let mut raw_entries: Vec<RawAurPackage> =
        serde_json::from_reader(decoder).context("Failed to parse AUR JSON metadata")?;

    // Sort by name for binary search (critical for zero-copy lookups)
    raw_entries.sort_by(|a, b| a.name.cmp(&b.name));

    let entries = raw_entries
        .into_iter()
        .map(|p| AurEntry {
            name: p.name,
            version: p.version,
            maintainer: p.maintainer,
            last_modified: p.last_modified,
            description: p.description,
            num_votes: p.num_votes.unwrap_or(0),
            popularity: p.popularity.unwrap_or(0.0),
            out_of_date: p.out_of_date,
        })
        .collect();

    let archive = AurArchive { entries };

    // Serialize to rkyv format
    let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(&archive)
        .map_err(|e| anyhow::anyhow!("Serialization error: {e}"))?;

    // Use a temporary file for atomic update to avoid corrupting the index
    let parent = output_path.parent().unwrap_or_else(|| Path::new("."));
    let mut temp =
        NamedTempFile::new_in(parent).context("Failed to create temporary index file")?;
    temp.write_all(&bytes)
        .context("Failed to write index data")?;
    temp.persist(output_path)
        .context("Failed to persist index file")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_index() -> Result<()> {
        let temp_dir = tempfile::tempdir()?;
        let json_path = temp_dir.path().join("metadata.json.gz");
        let index_path = temp_dir.path().join("metadata.rkyv");

        // Create a mock Gzip JSON
        let data = r#"[
            {"Name": "pkg-a", "Version": "1.0", "Maintainer": "user1", "LastModified": 100, "Description": "desc a", "NumVotes": 10, "Popularity": 0.5},
            {"Name": "pkg-b", "Version": "2.0", "Maintainer": null, "LastModified": 200, "Description": null, "NumVotes": 5, "Popularity": 0.1},
            {"Name": "Another-Pkg", "Version": "0.1", "Maintainer": "user2", "LastModified": 300, "Description": "another", "NumVotes": 0, "Popularity": 0.0}
        ]"#;

        let file = File::create(&json_path)?;
        let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, data.as_bytes())?;
        encoder.finish()?;

        // Build index
        build_index(&json_path, &index_path)?;

        // Verify index
        let index = AurIndex::open(&index_path)?;
        assert!(index.contains("pkg-a")?);
        assert!(index.contains("pkg-b")?);
        assert!(index.contains("Another-Pkg")?);

        let pkg_a = index.get("pkg-a")?.unwrap();
        assert_eq!(pkg_a.name.as_str(), "pkg-a");
        assert_eq!(pkg_a.version.as_str(), "1.0");
        assert_eq!(pkg_a.description.as_ref().unwrap().as_str(), "desc a");
        assert_eq!(pkg_a.num_votes, 10);

        // Test search
        let results = index.search("pkg", 10)?;
        assert_eq!(results.len(), 3); // pkg-a, pkg-b, Another-Pkg (contains 'pkg')

        let results = index.search("another", 10)?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name.as_str(), "Another-Pkg");

        Ok(())
    }
}
