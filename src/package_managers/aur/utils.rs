use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::process::Command;

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
        let path_str = path.to_string_lossy();
        let status = Command::new("sudo")
            .args(["-u", &user, "mkdir", "-p", "--", path_str.as_ref()])
            .status()
            .await
            .with_context(|| format!("Failed to create directory as user '{user}': {path_str}"))?;

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
        let path_str = path.to_string_lossy();
        let status = Command::new("sudo")
            .args(["-u", &user, "rm", "-rf", "--", path_str.as_ref()])
            .status()
            .await
            .with_context(|| format!("Failed to remove directory as user '{user}': {path_str}"))?;

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
        let path_str = path.to_string_lossy();
        let status = std::process::Command::new("sudo")
            .args(["-u", &user, "mkdir", "-p", "--", path_str.as_ref()])
            .status()
            .with_context(|| format!("Failed to create directory as user '{user}': {path_str}"))?;

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
