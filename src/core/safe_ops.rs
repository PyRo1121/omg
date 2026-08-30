//! Safe operations library for OMG
//!
//! Provides safe constructors and utilities for common operations that would otherwise
//! require `unwrap()` or `expect()`. This module helps eliminate panic-prone patterns
//! throughout the codebase while maintaining performance and ergonomics.

use anyhow::{Context, Result};
use std::io::Write;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
/// Create a `NonZeroU32` with a default fallback value.
///
/// If both `value` and `default` are zero, falls back to `NonZeroU32::MIN` (1).
pub fn nonzero_u32_or_default(value: u32, default: u32) -> NonZeroU32 {
    NonZeroU32::new(value)
        .or_else(|| NonZeroU32::new(default))
        .unwrap_or(NonZeroU32::MIN)
}

/// Validate only the basic string syntax of a path.
///
/// This rejects empty, non-UTF-8, and NUL-containing paths. It deliberately
/// does not claim symlink, traversal, ownership, or containment safety;
/// callers crossing those boundaries must apply a domain-specific validator.
pub fn validate_path_syntax<P: AsRef<Path>>(path: P) -> Result<PathBuf> {
    let path = path.as_ref();

    // Check for empty path
    if path.as_os_str().is_empty() {
        return Err(anyhow::anyhow!("Path cannot be empty"));
    }

    // Check for null bytes
    let Some(path_str) = path.to_str() else {
        return Err(anyhow::anyhow!("Path contains invalid UTF-8"));
    };
    if path_str.contains('\0') {
        return Err(anyhow::anyhow!("Path contains null byte"));
    }

    // Return the path as-is (canonicalize() fails for non-existent paths)
    Ok(path.to_path_buf())
}

/// Create an executable file without following or racing a pre-existing path.
///
/// Returns `false` when `overwrite` is disabled and any filesystem entry is
/// already present. Forced replacement uses a same-directory atomic rename,
/// which replaces a destination symlink rather than writing through it.
#[cfg(unix)]
pub fn write_executable(path: &Path, contents: &[u8], overwrite: bool) -> Result<bool> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;

    if !overwrite {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true).mode(0o755);
        return match options.open(path) {
            Ok(mut file) => {
                file.write_all(contents)?;
                file.sync_all()?;
                Ok(true)
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
            Err(error) => {
                Err(error).with_context(|| format!("Failed to create {}", path.display()))
            }
        };
    }

    let mut temporary = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "Failed to create temporary executable in {}",
            parent.display()
        )
    })?;
    temporary
        .as_file_mut()
        .set_permissions(std::fs::Permissions::from_mode(0o755))?;
    temporary.as_file_mut().write_all(contents)?;
    temporary.as_file_mut().sync_all()?;
    temporary
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to replace executable {}", path.display()))?;
    sync_parent_directory_sync(path)?;
    Ok(true)
}

/// Atomically claim a private marker path without following an existing entry.
#[cfg(unix)]
pub fn create_private_marker(path: &Path, contents: &[u8]) -> Result<bool> {
    use std::os::unix::fs::OpenOptionsExt;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true).mode(0o600);
    match options.open(path) {
        Ok(mut file) => {
            file.write_all(contents)?;
            file.sync_all()?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        Err(error) => Err(error).with_context(|| format!("Failed to create {}", path.display())),
    }
}

/// Make a persisted file replacement durable by syncing its parent directory.
pub(crate) fn sync_parent_directory_sync(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("Failed to sync parent directory: {}", parent.display()))
}

/// Async bridge for [`sync_parent_directory_sync`].
#[cfg(feature = "arch")]
pub(crate) async fn sync_parent_directory(path: PathBuf) -> Result<()> {
    tokio::task::spawn_blocking(move || sync_parent_directory_sync(&path))
        .await
        .context("Parent-directory sync task failed")?
}

/// Safe file write with atomic operations
pub async fn atomic_write_file<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    let path = path.as_ref().to_path_buf();
    let contents = contents.as_ref().to_vec();
    tokio::task::spawn_blocking(move || atomic_write_file_sync(path, contents))
        .await
        .context("Atomic file writer task failed")?
}

/// Safe synchronous file write with atomic operations
pub fn atomic_write_file_sync<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> Result<()> {
    let path = validate_path_syntax(path)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;

    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("Failed to create temporary file in {}", parent.display()))?;
    temporary
        .as_file_mut()
        .write_all(contents.as_ref())
        .with_context(|| format!("Failed to write temporary file for {}", path.display()))?;
    temporary
        .as_file_mut()
        .sync_all()
        .with_context(|| format!("Failed to sync temporary file for {}", path.display()))?;
    temporary
        .persist(&path)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to replace {}", path.display()))?;
    // The rename above is only durable once the parent directory entry is
    // synced; without this, a crash can resurrect the previous version of the
    // file. https://lwn.net/Articles/457667/
    sync_parent_directory_sync(&path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[test]
    fn test_nonzero_u32_or_default() {
        let nz1 = nonzero_u32_or_default(0, 100);
        assert_eq!(nz1.get(), 100);

        let nz2 = nonzero_u32_or_default(50, 100);
        assert_eq!(nz2.get(), 50);
    }

    #[test]
    fn test_validate_path_syntax_valid() {
        let path = "/tmp";
        let result = validate_path_syntax(path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_path_syntax_empty() {
        let path = "";
        let result = validate_path_syntax(path);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_atomic_write_file() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = b"Hello, world!";

        let result = atomic_write_file(&file_path, content).await;
        assert!(result.is_ok());

        let read_content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(read_content, "Hello, world!");
    }

    #[cfg(unix)]
    #[test]
    fn executable_writer_never_follows_destination_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let target = temp.path().join("target");
        std::fs::write(&target, b"keep").unwrap();
        let link = temp.path().join("hook");
        symlink(&target, &link).unwrap();

        assert!(!write_executable(&link, b"new", false).unwrap());
        assert_eq!(std::fs::read(&target).unwrap(), b"keep");
        assert!(write_executable(&link, b"new", true).unwrap());
        assert_eq!(std::fs::read(&target).unwrap(), b"keep");
        assert_eq!(std::fs::read(&link).unwrap(), b"new");
    }

    #[cfg(unix)]
    #[test]
    fn private_marker_refuses_dangling_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let target = temp.path().join("target");
        let marker = temp.path().join("marker");
        symlink(&target, &marker).unwrap();

        assert!(!create_private_marker(&marker, b"1").unwrap());
        assert!(!target.exists());
    }

    #[test]
    fn test_atomic_write_file_sync() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let content = b"Hello, world!";

        let result = atomic_write_file_sync(&file_path, content);
        assert!(result.is_ok());

        let read_content = std::fs::read_to_string(&file_path).unwrap();
        assert_eq!(read_content, "Hello, world!");
    }
}
