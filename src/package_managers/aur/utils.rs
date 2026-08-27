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
    let base_canonical = base
        .canonicalize()
        .with_context(|| format!("Base directory does not exist: {}", base.display()))?;

    // For target, if it exists, canonicalize it. If not, canonicalize parent and append filename.
    let target_canonical = if target.exists() {
        target
            .canonicalize()
            .with_context(|| format!("Failed to canonicalize: {}", target.display()))?
    } else {
        // Path doesn't exist yet - canonicalize parent and append leaf
        let parent = target
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Path has no parent: {}", target.display()))?;
        let leaf = target
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Path has no filename: {}", target.display()))?;

        let parent_canonical = parent
            .canonicalize()
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

/// Check if a path is a symlink (for critical operations that shouldn't follow symlinks).
///
/// Fails closed: a metadata error (permissions, loop, …) is returned as
/// [`Err`] so security callers reject the path instead of treating an
/// undecidable path as "not a symlink".
///
/// # Errors
/// Returns the underlying [`std::io::Error`] when `symlink_metadata` fails.
pub fn is_symlink(path: &Path) -> std::io::Result<bool> {
    Ok(path.symlink_metadata()?.file_type().is_symlink())
}

/// Validate a package build directory is safe to use.
/// Rejects symlinks and paths that escape the build root.
/// Fails closed when the directory's type cannot be determined.
pub fn validate_build_dir(build_root: &Path, package_name: &str) -> Result<PathBuf> {
    let pkg_dir = build_root.join(package_name);

    // If pkg_dir exists, verify it's not a symlink; an unreadable directory
    // is rejected rather than silently allowed through.
    match std::fs::symlink_metadata(&pkg_dir) {
        Ok(meta) if meta.file_type().is_symlink() => bail!(
            "Security: Package directory is a symlink (potential attack): {}",
            pkg_dir.display()
        ),
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "Security: Cannot inspect package directory (failing closed): {}",
                    pkg_dir.display()
                )
            });
        }
    }

    // Validate path stays inside build_root
    if pkg_dir.exists() {
        validate_path_inside(build_root, &pkg_dir)?;
    }

    Ok(pkg_dir)
}

#[inline]
#[must_use]
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

/// User name of the original (pre-`sudo`/`doas`) invoker, regardless of
/// whether we are currently running as root.
///
/// Distinct from [`original_user`], which only resolves when running as
/// root and is used to decide whether de-escalation is needed at all.
#[must_use]
pub fn build_user() -> Option<String> {
    std::env::var("SUDO_USER")
        .ok()
        .or_else(|| std::env::var("DOAS_USER").ok())
}

pub fn original_user() -> Option<String> {
    if !crate::core::is_root() {
        return None;
    }
    build_user()
}

pub fn original_user_home() -> Option<PathBuf> {
    original_user().map(|user| {
        std::env::var("SUDO_HOME")
            .map_or_else(|_| PathBuf::from(format!("/home/{user}")), PathBuf::from)
    })
}

fn sudo_as_user_command(
    user: &str,
    program: &str,
    flags: &[&str],
    path: &Path,
) -> std::process::Command {
    let mut command = std::process::Command::new("sudo");
    command
        .arg("-u")
        .arg(user)
        .arg(program)
        .args(flags)
        .arg("--")
        .arg(path.as_os_str());
    command
}

fn ensure_sudo_success(
    status: std::process::ExitStatus,
    user: &str,
    operation: &str,
    path: &Path,
) -> Result<()> {
    anyhow::ensure!(
        status.success(),
        "Failed to {operation} as user '{user}': {}",
        path.display()
    );
    Ok(())
}

pub async fn create_dir_as_user(path: &Path) -> Result<()> {
    if let Some(user) = original_user() {
        let status = Command::from(sudo_as_user_command(&user, "mkdir", &["-p"], path))
            .status()
            .await
            .with_context(|| {
                format!(
                    "Failed to create directory as user '{user}': {}",
                    path.display()
                )
            })?;

        ensure_sudo_success(status, &user, "create directory", path)
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
    if let Some(user) = original_user() {
        let status = Command::from(sudo_as_user_command(&user, "rm", &["-rf"], path))
            .status()
            .await
            .with_context(|| {
                format!(
                    "Failed to remove directory as user '{user}': {}",
                    path.display()
                )
            })?;

        ensure_sudo_success(status, &user, "remove directory", path)
    } else {
        tokio::fs::remove_dir_all(path)
            .await
            .with_context(|| format!("Failed to remove directory: {}", path.display()))
    }
}

pub fn create_dir_as_user_sync(path: &Path) -> Result<()> {
    if let Some(user) = original_user() {
        let status = sudo_as_user_command(&user, "mkdir", &["-p"], path)
            .status()
            .with_context(|| {
                format!(
                    "Failed to create directory as user '{user}': {}",
                    path.display()
                )
            })?;

        ensure_sudo_success(status, &user, "create directory", path)
    } else {
        std::fs::create_dir_all(path)
            .with_context(|| format!("Failed to create directory: {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sudo_as_user_command_always_separates_path_from_options() {
        let command = sudo_as_user_command("builder", "rm", &["-rf"], Path::new("-cache"));
        assert_eq!(command.get_program(), "sudo");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ["-u", "builder", "rm", "-rf", "--", "-cache"]
        );
    }

    #[test]
    fn is_symlink_detects_symlinks_and_regular_files() {
        let temp = tempfile::tempdir().expect("temp dir");
        let file = temp.path().join("regular.txt");
        std::fs::write(&file, "x").expect("write");
        let link = temp.path().join("link");

        // Unix-only crate: symlinks are always available.
        {
            std::os::unix::fs::symlink(&file, &link).expect("symlink");
            assert!(is_symlink(&link).expect("link must be inspected"));
        }
        assert!(!is_symlink(&file).expect("file must be inspected"));
    }

    #[test]
    fn is_symlink_fails_closed_for_missing_path() {
        // Fail-closed contract: an undecidable path is an Err, never `false`.
        let missing = std::env::temp_dir().join("omg-is-symlink-does-not-exist");
        let _ = std::fs::remove_file(&missing);
        assert!(
            is_symlink(&missing).is_err(),
            "missing path must propagate an error instead of reporting 'not a symlink'"
        );
    }

    #[test]
    fn validate_build_dir_fails_closed_on_uninspectable_directory() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().expect("temp dir");
        let build_root = temp.path().join("root");
        std::fs::create_dir_all(&build_root).expect("build root");
        let pkg_dir = build_root.join("mypkg");
        std::fs::create_dir(&pkg_dir).expect("pkg dir");

        let meta = std::fs::metadata(&pkg_dir).expect("meta");
        let mut perms = meta.permissions();
        perms.set_mode(0o000);
        std::fs::set_permissions(&pkg_dir, perms).expect("chmod");

        let result = validate_build_dir(&build_root, "mypkg");
        let unreadable = std::fs::symlink_metadata(&pkg_dir).is_err();

        let mut restore = std::fs::metadata(&pkg_dir).expect("meta").permissions();
        restore.set_mode(0o755);
        std::fs::set_permissions(&pkg_dir, restore).expect("restore chmod");

        if unreadable {
            let error = result.expect_err("uninspectable directory must fail closed");
            assert!(error.to_string().contains("fail"), "got: {error}");
        }
    }
}
