//! Content-Addressable Storage for .deb Packages
//!
//! Inspired by pnpm's content store architecture, this module provides:
//! - SHA256-based deduplication of downloaded packages
//! - Hard linking for zero-copy file sharing
//! - Automatic garbage collection of unused packages
//! - Efficient storage with 2-char hash sharding

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};

use crate::core::paths;

/// Content-addressable storage for .deb packages
///
/// Stores packages by their SHA256 hash to eliminate duplicates across
/// multiple installations. Uses a sharded directory structure (first 2 chars
/// of hash) to avoid file system performance issues with large directories.
#[derive(Clone)]
pub struct ContentStore {
    /// Base directory for the content store
    store_dir: PathBuf,
}

impl ContentStore {
    /// Create a new content store at the default cache location
    pub fn new() -> Self {
        Self {
            store_dir: paths::cache_dir().join("debian_content_store"),
        }
    }

    /// Create a content store at a custom location
    pub fn with_path(path: PathBuf) -> Self {
        Self { store_dir: path }
    }

    /// Initialize the content store, creating directories if needed
    pub fn init(&self) -> Result<()> {
        fs::create_dir_all(&self.store_dir).with_context(|| {
            format!(
                "Failed to create content store at {}",
                self.store_dir.display()
            )
        })
    }

    /// Store a .deb file by its SHA256 hash
    ///
    /// Returns the hash for later retrieval. If the file already exists in the
    /// store, it's not copied again (deduplication).
    pub fn store(&self, deb_path: &Path) -> Result<String> {
        // Calculate SHA256 hash
        let mut hasher = Sha256::new();
        let data = fs::read(deb_path)
            .with_context(|| format!("Failed to read .deb file: {}", deb_path.display()))?;
        hasher.update(&data);
        let hash = hex::encode(hasher.finalize());

        // Store under sharded directory (first 2 chars)
        let hash_dir = self.store_dir.join(&hash[..2]);
        fs::create_dir_all(&hash_dir)
            .with_context(|| format!("Failed to create shard directory: {}", hash_dir.display()))?;

        let dest = hash_dir.join(&hash);

        // Skip if already exists (deduplication)
        if dest.exists() {
            tracing::debug!("Package already in content store: {hash}");
        } else {
            // Atomic write using temp file + rename
            let temp_dest = hash_dir.join(format!(".{hash}.tmp"));
            fs::copy(deb_path, &temp_dest).context("Failed to copy .deb to content store")?;
            fs::rename(&temp_dest, &dest)
                .context("Failed to atomically move .deb to content store")?;

            tracing::debug!("Stored .deb in content store with hash: {hash}");
        }

        Ok(hash)
    }

    /// Create a hard link from the content store to a target location
    ///
    /// This provides zero-copy file sharing - the same physical file is shared
    /// between the content store and the target location. Changes to either
    /// affect both (but .deb files are read-only in practice).
    pub fn hard_link(&self, hash: &str, target: &Path) -> Result<()> {
        let hash_dir = self.store_dir.join(&hash[..2]);
        let source = hash_dir.join(hash);

        if !source.exists() {
            anyhow::bail!("Package not found in content store: {hash}");
        }

        // Create parent directories for target
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create target directory: {}", parent.display())
            })?;
        }

        // Remove target if it exists (hard_link fails if target exists)
        if target.exists() {
            fs::remove_file(target).with_context(|| {
                format!("Failed to remove existing target: {}", target.display())
            })?;
        }

        // Create hard link (zero-copy)
        fs::hard_link(&source, target).with_context(|| {
            format!(
                "Failed to hard link {} to {}",
                source.display(),
                target.display()
            )
        })?;

        tracing::debug!("Hard linked {} to {}", hash, target.display());
        Ok(())
    }

    /// Check if a package exists in the content store by its hash
    #[inline]
    pub fn contains(&self, hash: &str) -> bool {
        if hash.len() < 2 {
            return false;
        }
        let hash_dir = self.store_dir.join(&hash[..2]);
        hash_dir.join(hash).exists()
    }

    /// Get the path to a package in the content store
    ///
    /// Returns `None` if the package doesn't exist.
    pub fn get_path(&self, hash: &str) -> Option<PathBuf> {
        if hash.len() < 2 {
            return None;
        }
        let hash_dir = self.store_dir.join(&hash[..2]);
        let path = hash_dir.join(hash);
        if path.exists() { Some(path) } else { None }
    }

    /// Calculate the SHA256 hash of a file without storing it
    ///
    /// Useful for checking if a package is already in the store before
    /// downloading or copying it.
    pub fn hash_file(path: &Path) -> Result<String> {
        let mut hasher = Sha256::new();
        let data =
            fs::read(path).with_context(|| format!("Failed to read file: {}", path.display()))?;
        hasher.update(&data);
        Ok(hex::encode(hasher.finalize()))
    }

    /// Get the total size of the content store in bytes
    ///
    /// Walks the entire store directory and sums file sizes.
    pub fn total_size(&self) -> Result<u64> {
        let mut total = 0u64;

        if !self.store_dir.exists() {
            return Ok(0);
        }

        for entry in fs::read_dir(&self.store_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Recurse into shard directories
                for file_entry in fs::read_dir(&path)? {
                    let file_entry = file_entry?;
                    if let Ok(metadata) = file_entry.metadata() {
                        total += metadata.len();
                    }
                }
            }
        }

        Ok(total)
    }

    /// Count the number of packages in the content store
    pub fn package_count(&self) -> Result<usize> {
        let mut count = 0;

        if !self.store_dir.exists() {
            return Ok(0);
        }

        for entry in fs::read_dir(&self.store_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Count files in shard directories (excluding temp files)
                for file_entry in fs::read_dir(&path)? {
                    let file_entry = file_entry?;
                    let file_name = file_entry.file_name();
                    if !file_name.to_string_lossy().starts_with('.') {
                        count += 1;
                    }
                }
            }
        }

        Ok(count)
    }

    /// Clean up temporary files left by failed operations
    pub fn cleanup_temp_files(&self) -> Result<usize> {
        let mut cleaned = 0;

        if !self.store_dir.exists() {
            return Ok(0);
        }

        for entry in fs::read_dir(&self.store_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                for file_entry in fs::read_dir(&path)? {
                    let file_entry = file_entry?;
                    let file_path = file_entry.path();
                    let file_name = file_entry.file_name();

                    // Remove temp files (start with '.')
                    if file_name.to_string_lossy().starts_with('.')
                        && fs::remove_file(&file_path).is_ok()
                    {
                        cleaned += 1;
                        tracing::debug!("Cleaned temp file: {}", file_path.display());
                    }
                }
            }
        }

        if cleaned > 0 {
            tracing::info!("Cleaned {} temporary files from content store", cleaned);
        }

        Ok(cleaned)
    }
}

impl Default for ContentStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_content_store_new() {
        let store = ContentStore::new();
        assert!(
            store
                .store_dir
                .to_string_lossy()
                .contains("debian_content_store")
        );
    }

    #[test]
    fn test_content_store_init() {
        let temp_dir = TempDir::new().unwrap();
        let store = ContentStore::with_path(temp_dir.path().join("store"));

        assert!(store.init().is_ok());
        assert!(store.store_dir.exists());
    }

    #[test]
    fn test_store_and_contains() {
        let temp_dir = TempDir::new().unwrap();
        let store = ContentStore::with_path(temp_dir.path().join("store"));
        store.init().unwrap();

        // Create a test file
        let test_file = temp_dir.path().join("test.deb");
        fs::write(&test_file, b"test package data").unwrap();

        // Store it
        let hash = store.store(&test_file).unwrap();
        assert_eq!(hash.len(), 64); // SHA256 is 64 hex chars
        assert!(store.contains(&hash));
    }

    #[test]
    fn test_hard_link() {
        let temp_dir = TempDir::new().unwrap();
        let store = ContentStore::with_path(temp_dir.path().join("store"));
        store.init().unwrap();

        // Create and store a test file
        let test_file = temp_dir.path().join("test.deb");
        fs::write(&test_file, b"test package data").unwrap();
        let hash = store.store(&test_file).unwrap();

        // Hard link it
        let target = temp_dir.path().join("linked.deb");
        assert!(store.hard_link(&hash, &target).is_ok());
        assert!(target.exists());

        // Verify content
        let content = fs::read(&target).unwrap();
        assert_eq!(content, b"test package data");
    }

    #[test]
    fn test_deduplication() {
        let temp_dir = TempDir::new().unwrap();
        let store = ContentStore::with_path(temp_dir.path().join("store"));
        store.init().unwrap();

        // Create a test file
        let test_file = temp_dir.path().join("test.deb");
        fs::write(&test_file, b"duplicate data").unwrap();

        // Store it twice
        let hash1 = store.store(&test_file).unwrap();
        let hash2 = store.store(&test_file).unwrap();

        // Should be the same hash
        assert_eq!(hash1, hash2);

        // Should only have one copy
        assert_eq!(store.package_count().unwrap(), 1);
    }

    #[test]
    fn test_hash_file() {
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.deb");
        fs::write(&test_file, b"test data").unwrap();

        let hash = ContentStore::hash_file(&test_file).unwrap();
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_total_size() {
        let temp_dir = TempDir::new().unwrap();
        let store = ContentStore::with_path(temp_dir.path().join("store"));
        store.init().unwrap();

        // Initially empty
        assert_eq!(store.total_size().unwrap(), 0);

        // Add a file
        let test_file = temp_dir.path().join("test.deb");
        let data = b"test package data";
        fs::write(&test_file, data).unwrap();
        store.store(&test_file).unwrap();

        // Size should match
        assert_eq!(store.total_size().unwrap(), data.len() as u64);
    }

    #[test]
    fn test_cleanup_temp_files() {
        let temp_dir = TempDir::new().unwrap();
        let store = ContentStore::with_path(temp_dir.path().join("store"));
        store.init().unwrap();

        // Create a shard directory and add a temp file
        let shard_dir = store.store_dir.join("ab");
        fs::create_dir_all(&shard_dir).unwrap();
        let temp_file = shard_dir.join(".temp.tmp");
        fs::write(&temp_file, b"temp").unwrap();

        assert_eq!(store.cleanup_temp_files().unwrap(), 1);
        assert!(!temp_file.exists());
    }
}
