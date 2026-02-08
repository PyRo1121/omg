use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tokio::process::Command;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// SECURITY: Path Validation (TOCTOU Prevention)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Validate that a path is safely inside the expected base directory.
/// Prevents symlink attacks and directory traversal.
///
/// # Security
/// This function canonicalizes both paths and verifies the target
/// starts with the base, preventing:
/// - Symlink escapes (`build_dir/evil -> /etc`)
/// - Directory traversal (`build_dir/../../../etc`)
pub fn validate_path_inside(base: &Path, target: &Path) -> Result<PathBuf> {
    // Canonicalize base - must exist
    let base_canonical = base.canonicalize()
        .with_context(|| format!("Base directory does not exist: {}", base.display()))?;

    // For target, if it exists, canonicalize it. If not, canonicalize parent and append filename.
    let target_canonical = if target.exists() {
        target.canonicalize()
            .with_context(|| format!("Failed to canonicalize: {}", target.display()))?
    } else {
        // Path doesn't exist yet - canonicalize parent and append leaf
        let parent = target.parent()
            .ok_or_else(|| anyhow::anyhow!("Path has no parent: {}", target.display()))?;
        let leaf = target.file_name()
            .ok_or_else(|| anyhow::anyhow!("Path has no filename: {}", target.display()))?;

        let parent_canonical = parent.canonicalize()
            .with_context(|| format!("Parent directory does not exist: {}", parent.display()))?;

        parent_canonical.join(leaf)
    };

    // Verify target is inside base
    if !target_canonical.starts_with(&base_canonical) {
        bail!(
            "Security: Path escapes base directory!\n  Base: {}\n  Target: {}",
            base_canonical.display(),
            target_canonical.display()
        );
    }

    Ok(target_canonical)
}

/// Check if a path is a symlink (for critical operations that shouldn't follow symlinks)
pub fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|m| m.file_type().is_symlink())
        .unwrap_or(false)
}

/// Validate a package build directory is safe to use.
/// Rejects symlinks and paths that escape the build root.
pub fn validate_build_dir(build_root: &Path, package_name: &str) -> Result<PathBuf> {
    let pkg_dir = build_root.join(package_name);

    // If pkg_dir exists, verify it's not a symlink
    if pkg_dir.exists() && is_symlink(&pkg_dir) {
        bail!(
            "Security: Package directory is a symlink (potential attack): {}",
            pkg_dir.display()
        );
    }

    // Validate path stays inside build_root
    if pkg_dir.exists() {
        validate_path_inside(build_root, &pkg_dir)?;
    }

    Ok(pkg_dir)
}

#[inline]
pub fn has_word_boundary_match(haystack: &str, needle: &str) -> bool {
    for (pos, _) in haystack.match_indices(needle) {
        if pos == 0
            || haystack.as_bytes()[pos - 1].is_ascii_whitespace()
            || haystack.as_bytes()[pos - 1] == b'-'
            || haystack.as_bytes()[pos - 1] == b'_'
            || haystack.as_bytes()[pos - 1] == b'.'
        {
            return true;
        }
    }
    false
}

pub fn get_original_user() -> Option<String> {
    if !crate::core::is_root() {
        return None;
    }
    std::env::var("SUDO_USER")
        .ok()
        .or_else(|| std::env::var("DOAS_USER").ok())
}

pub fn get_original_user_home() -> Option<PathBuf> {
    get_original_user().map(|user| {
        std::env::var("SUDO_HOME")
            .map_or_else(|_| PathBuf::from(format!("/home/{user}")), PathBuf::from)
    })
}

pub async fn create_dir_as_user(path: &Path) -> Result<()> {
    if let Some(user) = get_original_user() {
        // Use OsStr directly to handle non-UTF8 paths correctly
        let status = Command::new("sudo")
            .arg("-u")
            .arg(&user)
            .arg("mkdir")
            .arg("-p")
            .arg("--")
            .arg(path.as_os_str())
            .status()
            .await
            .with_context(|| format!("Failed to create directory as user '{user}': {}", path.display()))?;

        if !status.success() {
            anyhow::bail!(
                "Failed to create directory as user '{}': {}",
                user,
                path.display()
            );
        }
        Ok(())
    } else {
        tokio::fs::create_dir_all(path)
            .await
            .with_context(|| format!("Failed to create directory: {}", path.display()))
    }
}

pub fn is_root_owned(path: &Path) -> bool {
    path.metadata().is_ok_and(|m| m.uid() == 0)
}

pub async fn remove_dir_as_user(path: &Path) -> Result<()> {
    if let Some(user) = get_original_user() {
        // Use OsStr directly to handle non-UTF8 paths correctly
        let status = Command::new("sudo")
            .arg("-u")
            .arg(&user)
            .arg("rm")
            .arg("-rf")
            .arg("--")
            .arg(path.as_os_str())
            .status()
            .await
            .with_context(|| format!("Failed to remove directory as user '{user}': {}", path.display()))?;

        if !status.success() {
            anyhow::bail!(
                "Failed to remove directory as user '{}': {}",
                user,
                path.display()
            );
        }
        Ok(())
    } else {
        tokio::fs::remove_dir_all(path)
            .await
            .with_context(|| format!("Failed to remove directory: {}", path.display()))
    }
}

pub fn create_dir_as_user_sync(path: &Path) -> Result<()> {
    if let Some(user) = get_original_user() {
        // Use OsStr directly to handle non-UTF8 paths correctly
        let status = std::process::Command::new("sudo")
            .arg("-u")
            .arg(&user)
            .arg("mkdir")
            .arg("-p")
            .arg("--")
            .arg(path.as_os_str())
            .status()
            .with_context(|| format!("Failed to create directory as user '{user}': {}", path.display()))?;

        if !status.success() {
            anyhow::bail!(
                "Failed to create directory as user '{}': {}",
                user,
                path.display()
            );
        }
        Ok(())
    } else {
        std::fs::create_dir_all(path)
            .with_context(|| format!("Failed to create directory: {}", path.display()))
    }
}
