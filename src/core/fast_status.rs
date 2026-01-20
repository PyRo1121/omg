//! Zero-IPC fast status for sub-millisecond reads
//!
//! The daemon writes a binary status file that CLI reads directly,
//! bypassing socket connection and IPC serialization overhead.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::core::paths;

/// Fast status structure - fixed size for mmap-friendly reads
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
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
            total_packages: total as u32,
            explicit_packages: explicit as u32,
            orphan_packages: orphans as u32,
            updates_available: updates as u32,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs()),
        }
    }

    /// Write status to file (atomic via temp + rename)
    pub fn write_to_file(&self, path: &Path) -> std::io::Result<()> {
        let tmp_path = path.with_extension("tmp");

        // SAFETY: FastStatus is a #[repr(C)] POD struct with fixed layout and no padding
        // between fields that could contain uninitialized bytes. transmute_copy is sound
        // because Self has a well-defined byte representation.
        let bytes: [u8; std::mem::size_of::<Self>()] = unsafe { std::mem::transmute_copy(self) };

        let mut file = File::create(&tmp_path)?;
        file.write_all(&bytes)?;
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

        // SAFETY: FastStatus is a #[repr(C)] POD struct. The byte array has the exact
        // size of Self and was read from a file that was written by write_to_file().
        // We validate magic and version immediately after to catch corrupted data.
        let status: Self = unsafe { std::mem::transmute(bytes) };
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
