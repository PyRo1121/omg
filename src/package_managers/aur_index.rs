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
use tempfile::NamedTempFile;

use crate::package_managers::aur_metadata::AurJsonPackage;
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

impl std::fmt::Debug for AurIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AurIndex").finish_non_exhaustive()
    }
}

impl AurIndex {
    /// Open an existing AUR index using memory mapping
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open index at {}", path.display()))?;

        // SAFETY: `Mmap::map` creates a read-only mapping; the mapping itself
        // never writes to the file. Reading rkyv archived types through the
        // mapping is sound only while no writer mutates the mapped bytes.
        // That precondition holds because omg publishes indexes exclusively
        // via `build_index`, which writes a temp file and installs it with
        // `NamedTempFile::persist` (an atomic rename): an existing mapping
        // keeps pinning the old inode, whose bytes are final before the swap.
        // The file at `path` must therefore never be written in place — only
        // replaced by rename — which is true for all writers in this crate.
        #[expect(unsafe_code)]
        let mmap = unsafe { Mmap::map(&file)? };

        Ok(Self { mmap })
    }

    /// Access the archived data with validation
    fn archive(&self) -> Result<&ArchivedAurArchive> {
        rkyv::access::<rkyv::Archived<AurArchive>, rkyv::rancor::Error>(&self.mmap)
            .map_err(|e| anyhow::anyhow!("Corrupted AUR index: {e}"))
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

    /// Search for packages matching a query (substring match in name or description).
    ///
    /// Case-insensitive comparison reuses a single scratch buffer instead of
    /// allocating two lowercase copies per entry (the index holds ~90k rows).
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<&ArchivedAurEntry>> {
        let archive = self.archive()?;
        let query_lower = query.to_lowercase();
        let mut lower_buf = String::new();

        Ok(archive
            .entries
            .iter()
            .filter(|e| {
                lower_buf.clear();
                lower_buf.extend(e.name.as_str().chars().flat_map(char::to_lowercase));
                if lower_buf.contains(&query_lower) {
                    return true;
                }
                match e.description.as_ref() {
                    Some(desc) => {
                        lower_buf.clear();
                        lower_buf.extend(desc.as_str().chars().flat_map(char::to_lowercase));
                        lower_buf.contains(&query_lower)
                    }
                    None => false,
                }
            })
            .take(limit)
            .collect())
    }

    /// Batch update check against the index.
    ///
    /// Returns `(updates, missing)`:
    /// - `updates`: remote versions newer than the local ones, for packages
    ///   present in the index;
    /// - `missing`: queried package names absent from this index, so callers
    ///   can fall back to the RPC for exactly those names instead of either
    ///   treating them as up-to-date or re-querying everything.
    pub fn updates_for(
        &self,
        local_pkgs: &[(String, alpm_types::Version)],
    ) -> Result<(
        Vec<(String, alpm_types::Version, alpm_types::Version)>,
        Vec<String>,
    )> {
        let mut updates = Vec::new();
        let mut missing = Vec::new();
        let archive = self.archive()?;

        for (name, local_version) in local_pkgs {
            match archive
                .entries
                .binary_search_by_key(&name.as_str(), |e: &ArchivedAurEntry| e.name.as_str())
            {
                Ok(idx) => {
                    let entry = &archive.entries[idx];
                    let remote_version = parse_version_or_zero(entry.version.as_str());
                    if remote_version > *local_version {
                        updates.push((name.clone(), local_version.clone(), remote_version));
                    }
                }
                Err(_) => missing.push(name.clone()),
            }
        }

        Ok((updates, missing))
    }
}

/// Build the binary index from the AUR JSON archive
pub fn build_index(json_path: &Path, output_path: &Path) -> Result<()> {
    let file = File::open(json_path).context("Failed to open AUR JSON")?;
    let reader = BufReader::new(file);
    let decoder = GzDecoder::new(reader);

    // Parse the JSON array. AUR's metadata is a large array of objects.
    let mut raw_entries: Vec<AurJsonPackage> =
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
    temp.as_file_mut()
        .sync_all()
        .context("Failed to sync AUR index")?;
    temp.persist(output_path)
        .map_err(|error| error.error)
        .context("Failed to persist index file")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_test_index(data: &str) -> Result<(tempfile::TempDir, AurIndex)> {
        let temp_dir = tempfile::tempdir()?;
        let json_path = temp_dir.path().join("metadata.json.gz");
        let index_path = temp_dir.path().join("metadata.rkyv");
        let file = File::create(&json_path)?;
        let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        std::io::Write::write_all(&mut encoder, data.as_bytes())?;
        encoder.finish()?;
        build_index(&json_path, &index_path)?;
        let index = AurIndex::open(&index_path)?;
        Ok((temp_dir, index))
    }

    #[test]
    fn test_build_index() -> Result<()> {
        let data = r#"[
            {"Name": "pkg-a", "Version": "1.0", "Maintainer": "user1", "LastModified": 100, "Description": "desc a", "NumVotes": 10, "Popularity": 0.5},
            {"Name": "pkg-b", "Version": "2.0", "Maintainer": null, "LastModified": 200, "Description": null, "NumVotes": 5, "Popularity": 0.1},
            {"Name": "Another-Pkg", "Version": "0.1", "Maintainer": "user2", "LastModified": 300, "Description": "another", "NumVotes": 0, "Popularity": 0.0}
        ]"#;
        let (_temp_dir, index) = open_test_index(data)?;
        assert!(index.get("pkg-a")?.is_some());
        assert!(index.get("pkg-b")?.is_some());
        assert!(index.get("Another-Pkg")?.is_some());

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

    /// Regression test for the AUR update fallback bug (fixed in v0.1.215)
    /// and its partial-staleness extension. `updates_for()` must report
    /// queried packages absent from the index via the `missing` vec so the
    /// caller (`AurClient::get_update_list`) can fall back to the RPC for
    /// exactly those names instead of treating them as up-to-date.
    #[test]
    fn test_updates_for_reports_missing_packages() -> Result<()> {
        // Index contains only pkg-a and pkg-b
        let data = r#"[
            {"Name": "pkg-a", "Version": "1.0", "Maintainer": "user1", "LastModified": 100, "Description": "desc a", "NumVotes": 10, "Popularity": 0.5},
            {"Name": "pkg-b", "Version": "2.0", "Maintainer": null, "LastModified": 200, "Description": null, "NumVotes": 5, "Popularity": 0.1}
        ]"#;
        let (_temp_dir, index) = open_test_index(data)?;

        // Query for packages NOT in the index — simulates a stale index
        // that doesn't contain the user's installed AUR packages
        let local_pkgs = vec![
            ("my-custom-pkg".to_string(), parse_version_or_zero("1.0")),
            ("another-missing".to_string(), parse_version_or_zero("0.5")),
        ];

        let (updates, missing) = index.updates_for(&local_pkgs)?;
        assert_eq!(
            missing,
            vec!["my-custom-pkg".to_string(), "another-missing".to_string()],
            "packages absent from the index must be reported as missing \
             so the caller can query the RPC for exactly those names"
        );
        assert!(
            updates.is_empty(),
            "missing packages must produce no updates"
        );

        Ok(())
    }

    /// Verify that a partially stale index both reports known updates and
    /// names the entries it is missing, instead of truncating results.
    #[test]
    fn test_updates_for_partial_staleness_keeps_updates_and_missing() -> Result<()> {
        let data = r#"[
            {"Name": "pkg-a", "Version": "2.0", "Maintainer": null, "LastModified": 100, "Description": null, "NumVotes": 1, "Popularity": 0.1}
        ]"#;
        let (_temp_dir, index) = open_test_index(data)?;
        let local_pkgs = vec![
            ("pkg-a".to_string(), parse_version_or_zero("1.0")), // in index, newer remote
            ("stale-only".to_string(), parse_version_or_zero("0.5")), // absent from index
        ];

        let (updates, missing) = index.updates_for(&local_pkgs)?;
        assert_eq!(
            updates.len(),
            1,
            "pkg-a update must survive partial staleness"
        );
        assert_eq!(updates[0].0, "pkg-a");
        assert_eq!(missing, vec!["stale-only".to_string()]);

        Ok(())
    }

    /// Verify that `updates_for()` correctly detects when the remote version
    /// is newer than the local version.
    #[test]
    fn test_updates_for_detects_newer_version() -> Result<()> {
        let data = r#"[
            {"Name": "pkg-a", "Version": "2.0", "Maintainer": "user1", "LastModified": 100, "Description": "desc a", "NumVotes": 10, "Popularity": 0.5},
            {"Name": "pkg-b", "Version": "1.0", "Maintainer": null, "LastModified": 200, "Description": null, "NumVotes": 5, "Popularity": 0.1}
        ]"#;
        let (_temp_dir, index) = open_test_index(data)?;

        let local_pkgs = vec![
            ("pkg-a".to_string(), parse_version_or_zero("1.0")), // remote 2.0 > local 1.0
            ("pkg-b".to_string(), parse_version_or_zero("1.0")), // remote 1.0 == local 1.0
        ];

        let (updates, missing) = index.updates_for(&local_pkgs)?;
        assert!(missing.is_empty());
        assert_eq!(updates.len(), 1, "Only pkg-a should have an update");
        assert_eq!(updates[0].0, "pkg-a");

        Ok(())
    }

    /// Verify that `updates_for()` reports no updates when local versions are
    /// already at or ahead of the index versions (no updates available).
    #[test]
    fn test_updates_for_no_updates_when_current() -> Result<()> {
        let data = r#"[
            {"Name": "pkg-a", "Version": "1.0", "Maintainer": "user1", "LastModified": 100, "Description": "desc a", "NumVotes": 10, "Popularity": 0.5}
        ]"#;
        let (_temp_dir, index) = open_test_index(data)?;

        // Local version matches remote — no update
        let local_same = vec![("pkg-a".to_string(), parse_version_or_zero("1.0"))];
        let (updates_same, _) = index.updates_for(&local_same)?;
        assert!(updates_same.is_empty());

        // Local version ahead of remote — no update
        let local_ahead = vec![("pkg-a".to_string(), parse_version_or_zero("2.0"))];
        let (updates_ahead, _) = index.updates_for(&local_ahead)?;
        assert!(updates_ahead.is_empty());

        Ok(())
    }
}
