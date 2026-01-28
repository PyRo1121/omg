//! Zero-IPC fast status for sub-millisecond reads
//!
//! The daemon writes a binary status file that CLI reads directly,
//! bypassing socket connection and IPC serialization overhead.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

use crate::core::paths;

/// Fast status structure - fixed size for mmap-friendly reads
///
/// Uses zerocopy for safe serialization without unsafe transmute.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, FromBytes, IntoBytes, Immutable, KnownLayout)]
pub struct FastStatus {
    /// Magic number for validation (0x4F4D4753 = "OMGS")
    pub magic: u32,
    /// Version of the format
    pub version: u8,
    /// Padding for alignment
    pub pad: [u8; 3],
    /// Total installed packages
    pub total_packages: u32,
    /// Explicitly installed packages
    pub explicit_packages: u32,
    /// Orphan packages
    pub orphan_packages: u32,
    /// Available updates
    pub updates_available: u32,
    /// Timestamp (unix seconds)
    pub timestamp: u64,
}

const MAGIC: u32 = 0x4F4D_4753; // "OMGS"
const VERSION: u8 = 1;

impl FastStatus {
    /// Create a new fast status
    #[must_use]
    pub fn new(total: usize, explicit: usize, orphans: usize, updates: usize) -> Self {
        Self {
            magic: MAGIC,
            version: VERSION,
            pad: [0; 3],
            total_packages: total.min(u32::MAX as usize) as u32,
            explicit_packages: explicit.min(u32::MAX as usize) as u32,
            orphan_packages: orphans.min(u32::MAX as usize) as u32,
            updates_available: updates.min(u32::MAX as usize) as u32,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
        }
    }

    /// Write status to file (atomic via temp + rename)
    pub fn write_to_file(&self, path: &Path) -> std::io::Result<()> {
        let tmp_path = path.with_extension("tmp");

        // Safe serialization using zerocopy - no unsafe needed
        let bytes = self.as_bytes();

        let mut file = File::create(&tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;

        // Atomic rename
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    }

    /// Read status from file (sub-millisecond)
    pub fn read_from_file(path: &Path) -> Option<Self> {
        let mut file = File::open(path).ok()?;
        let mut bytes = [0u8; std::mem::size_of::<Self>()];
        file.read_exact(&mut bytes).ok()?;

        // Safe deserialization using zerocopy - no unsafe needed
        let status = Self::read_from_bytes(&bytes).ok()?;
        if status.magic != MAGIC || status.version != VERSION {
            return None;
        }

        // Check freshness (max 60 seconds old)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        if now.saturating_sub(status.timestamp) > 60 {
            return None;
        }

        Some(status)
    }

    /// Read explicit count directly (fastest path)
    #[must_use]
    pub fn read_explicit_count() -> Option<usize> {
        let path = paths::fast_status_path();
        Self::read_from_file(&path).map(|s| s.explicit_packages as usize)
    }

    /// Write status to default path
    pub fn write_default(&self) -> std::io::Result<()> {
        self.write_to_file(&paths::fast_status_path())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_fast_status_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("status.bin");

        let status = FastStatus::new(1000, 200, 10, 5);
        status.write_to_file(&path).unwrap();

        let read = FastStatus::read_from_file(&path).unwrap();
        assert_eq!(read.total_packages, 1000);
        assert_eq!(read.explicit_packages, 200);
        assert_eq!(read.orphan_packages, 10);
        assert_eq!(read.updates_available, 5);
        assert_eq!(read.magic, MAGIC);
        assert_eq!(read.version, VERSION);
    }

    #[test]
    fn test_fast_status_invalid_magic() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("invalid.bin");

        let mut status = FastStatus::new(100, 50, 0, 0);
        status.magic = 0xDEAD_BEEF;
        status.write_to_file(&path).unwrap();

        assert!(FastStatus::read_from_file(&path).is_none());
    }

    #[test]
    fn test_fast_status_stale() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("stale.bin");

        let mut status = FastStatus::new(100, 50, 0, 0);
        status.timestamp = 0; // Way in the past
        status.write_to_file(&path).unwrap();

        assert!(FastStatus::read_from_file(&path).is_none());
    }
}
