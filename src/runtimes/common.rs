//! Common utilities for runtime managers
//!
//! Shared functionality for downloading, extracting, and managing runtime versions.

use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use sha2::{Digest, Sha256};

/// Progress bar style for downloads
#[allow(clippy::expect_used)] // Path operations on known-valid HOME directory; failure is unrecoverable
pub fn download_progress_style() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template(
            "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        )
        .expect("valid template")
        .progress_chars("█▓▒░")
}

/// Progress bar style for extraction
#[allow(clippy::expect_used)] // Path operations on known-valid HOME directory; failure is unrecoverable
pub fn extract_progress_style() -> ProgressStyle {
    ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .expect("valid template")
}

/// Download a file with progress bar and optional checksum verification
#[allow(clippy::unwrap_used)] // Path parsing on validated runtime paths; failure indicates corrupted state
pub async fn download_with_progress(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha256: Option<&str>,
) -> Result<()> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    let response = client
        .get(url)
        .header("User-Agent", "omg-package-manager/0.1")
        .send()
        .await
        .with_context(|| format!("Failed to connect to {}", extract_domain(url)))?;

    if !response.status().is_success() {
        let status = response.status();
        if status.as_u16() == 404 {
            anyhow::bail!(
                "Version not found (404). Check available versions with: omg list --available"
            );
        }
        anyhow::bail!("Download failed: HTTP {status}");
    }

    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(download_progress_style());

    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;
    }

    let mut file = tokio::fs::File::create(dest)
        .await
        .with_context(|| format!("Failed to create file: {}", dest.display()))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut hasher = if expected_sha256.is_some() {
        Some(Sha256::new())
    } else {
        None
    };

    while let Some(item) = stream.next().await {
        let chunk = item.context("Error downloading chunk")?;
        file.write_all(&chunk)
            .await
            .context("Error writing to file")?;

        if let Some(h) = &mut hasher {
            h.update(&chunk);
        }

        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }

    file.flush()
        .await
        .with_context(|| format!("Failed to flush download to: {}", dest.display()))?;

    // Verify checksum if provided
    if let Some(expected) = expected_sha256 {
        // SAFETY: hasher is guaranteed to be Some when expected_sha256 is Some
        // (see initialization at line 76-80)
        let hasher = hasher.expect("hasher initialized when expected_sha256 is Some");
        let actual = format!("{:x}", hasher.finalize());

        if actual != expected.to_lowercase() {
            anyhow::bail!(
                "Checksum mismatch!\n  Expected: {expected}\n  Got: {actual}\n\nThis could indicate a corrupted download or security issue."
            );
        }
        pb.println(format!("  {} Checksum verified", "✓".green()));
    }

    pb.finish_and_clear();
    Ok(())
}

/// Extract a .tar.gz archive with progress
pub async fn extract_tar_gz(
    archive_path: &Path,
    dest_dir: &Path,
    strip_components: usize,
) -> Result<()> {
    let archive_path = archive_path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = File::open(&archive_path)
            .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

        let decoder = flate2::read::GzDecoder::new(BufReader::new(file));
        let mut archive = tar::Archive::new(decoder);

        let pb = ProgressBar::new_spinner();
        pb.set_style(extract_progress_style());
        pb.set_message("Extracting...");

        fs::create_dir_all(&dest_dir)?;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;

            // Strip leading components
            let stripped: PathBuf = path.components().skip(strip_components).collect();
            if stripped.as_os_str().is_empty() {
                continue;
            }

            let dest_path = dest_dir.join(&stripped);
            pb.set_message(format!("Extracting: {}", stripped.display()));

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else {
                entry.unpack(&dest_path)?;
            }
        }

        pb.finish_and_clear();
        Ok(())
    })
    .await?
}

/// Extract a .tar.xz archive with progress (pure Rust)
pub async fn extract_tar_xz(
    archive_path: &Path,
    dest_dir: &Path,
    strip_components: usize,
) -> Result<()> {
    let archive_path = archive_path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = File::open(&archive_path)
            .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

        let pb = ProgressBar::new_spinner();
        pb.set_style(extract_progress_style());
        pb.set_message("Decompressing XZ...");

        // Pure Rust XZ decompression
        let mut decompressed = Vec::new();
        lzma_rs::xz_decompress(&mut BufReader::new(file), &mut decompressed)
            .context("Failed to decompress XZ archive")?;

        pb.set_message("Extracting...");

        let mut archive = tar::Archive::new(decompressed.as_slice());
        fs::create_dir_all(&dest_dir)?;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;

            // Strip leading components
            let stripped: PathBuf = path.components().skip(strip_components).collect();
            if stripped.as_os_str().is_empty() {
                continue;
            }

            let dest_path = dest_dir.join(&stripped);

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else {
                entry.unpack(&dest_path)?;
            }
        }

        pb.finish_and_clear();
        Ok(())
    })
    .await?
}

/// Extract a .zip archive with progress
pub async fn extract_zip(
    archive_path: &Path,
    dest_dir: &Path,
    strip_components: usize,
) -> Result<()> {
    let archive_path = archive_path.to_path_buf();
    let dest_dir = dest_dir.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = File::open(&archive_path)
            .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

        let mut archive = zip::ZipArchive::new(file).context("Failed to read ZIP archive")?;

        let pb = ProgressBar::new_spinner();
        pb.set_style(extract_progress_style());
        pb.set_message("Extracting...");

        fs::create_dir_all(&dest_dir)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let path = file.mangled_name();

            // Strip leading components
            let stripped: PathBuf = path.components().skip(strip_components).collect();
            if stripped.as_os_str().is_empty() {
                continue;
            }

            let dest_path = dest_dir.join(&stripped);
            pb.set_message(format!("Extracting: {}", stripped.display()));

            if file.is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else {
                if let Some(parent) = dest_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut outfile = File::create(&dest_path)?;
                std::io::copy(&mut file, &mut outfile)?;

                // Preserve permissions on Unix
                #[cfg(unix)]
                if let Some(mode) = file.unix_mode() {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&dest_path, fs::Permissions::from_mode(mode))?;
                }
            }
        }

        pb.finish_and_clear();
        Ok(())
    })
    .await?
}

/// Create or update the "current" symlink
pub fn set_current_version(versions_dir: &Path, version: &str) -> Result<()> {
    let current_link = versions_dir.join("current");
    let version_dir = versions_dir.join(version);

    if !version_dir.exists() {
        anyhow::bail!(
            "Version {version} is not installed. Install it first with: omg use <runtime>@{version}"
        );
    }

    // Remove existing symlink
    if current_link.exists() || current_link.is_symlink() {
        fs::remove_file(&current_link)?;
    }

    // Create new symlink
    #[cfg(unix)]
    std::os::unix::fs::symlink(&version_dir, &current_link)?;

    Ok(())
}

/// Get the current version from the "current" symlink
pub fn get_current_version(versions_dir: &Path) -> Option<String> {
    let current_link = versions_dir.join("current");
    fs::read_link(&current_link)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
}

/// List installed versions in a directory
pub fn list_installed_versions(versions_dir: &Path) -> Result<Vec<String>> {
    if !versions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut versions = Vec::new();
    for entry in fs::read_dir(versions_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        if name != "current" && entry.file_type()?.is_dir() {
            versions.push(name);
        }
    }

    versions.sort_by(|a, b| version_cmp(b, a));
    Ok(versions)
}

/// Compare semantic version strings (descending order)
pub fn version_cmp(a: &str, b: &str) -> Ordering {
    let a_parts: Vec<u32> = a
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|p| p.parse().ok())
        .collect();
    let b_parts: Vec<u32> = b
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|p| p.parse().ok())
        .collect();

    for i in 0..a_parts.len().max(b_parts.len()) {
        let a_part = a_parts.get(i).unwrap_or(&0);
        let b_part = b_parts.get(i).unwrap_or(&0);
        if a_part != b_part {
            return a_part.cmp(b_part);
        }
    }

    Ordering::Equal
}

/// Normalize version string (remove leading 'v' if present)
pub fn normalize_version(version: &str) -> String {
    version.trim_start_matches('v').to_string()
}

/// Extract domain from URL for error messages
fn extract_domain(url: &str) -> &str {
    url.split("://")
        .nth(1)
        .and_then(|s| s.split('/').next())
        .unwrap_or(url)
}

/// Print installation success message
pub fn print_installed(runtime: &str, version: &str) {
    tracing::info!(
        "\n{} {} {} installed successfully!",
        "✓".green().bold(),
        runtime.cyan(),
        version.yellow()
    );
}

/// Print version switch message
pub fn print_using(runtime: &str, version: &str, bin_path: &Path) {
    tracing::info!(
        "{} Now using {} {}",
        "✓".green(),
        runtime.cyan(),
        version.yellow()
    );
    tracing::info!(
        "  {} {}",
        "PATH:".dimmed(),
        bin_path.display().to_string().dimmed()
    );
}

/// Print already installed message
pub fn print_already_installed(runtime: &str, version: &str) {
    tracing::info!(
        "{} {} {} is already installed",
        "✓".green(),
        runtime.cyan(),
        version.yellow()
    );
}

/// Macro to generate common runtime manager methods
///
/// Eliminates ~300 lines of duplicated code across runtime managers
#[macro_export]
macro_rules! impl_runtime_common {
    ($manager_type:ty, $runtime_name:expr) => {
        impl $manager_type {
            /// List all installed versions of this runtime
            pub fn list_installed(&self) -> Result<Vec<String>> {
                $crate::runtimes::common::list_installed_versions(&self.versions_dir)
            }

            /// Get the currently active version
            #[must_use]
            pub fn current_version(&self) -> Option<String> {
                $crate::runtimes::common::get_current_version(&self.versions_dir)
            }

            /// Uninstall a specific version of this runtime
            pub fn uninstall(&self, version: &str) -> Result<()> {
                use anyhow::Context;
                use owo_colors::OwoColorize;
                use std::fs;

                let version = $crate::runtimes::common::normalize_version(version);
                let version_dir = self.versions_dir.join(&version);

                if !version_dir.exists() {
                    println!("{} {} {} is not installed", "→".dimmed(), $runtime_name, version);
                    return Ok(());
                }

                // Clear current link if uninstalling the active version
                if let Some(current) = self.current_version()
                    && current == version
                {
                    let _ = fs::remove_file(&self.current_link);
                }

                fs::remove_dir_all(&version_dir)
                    .with_context(|| format!("Failed to remove {} directory", version_dir.display()))?;

                println!("{} {} {} uninstalled", "✓".green(), $runtime_name, version);
                Ok(())
            }
        }
    };
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_version_cmp() {
        assert_eq!(version_cmp("1.0.0", "1.0.0"), Ordering::Equal);
        assert_eq!(version_cmp("1.0.1", "1.0.0"), Ordering::Greater);
        assert_eq!(version_cmp("1.0.0", "1.0.1"), Ordering::Less);
        assert_eq!(version_cmp("2.0.0", "1.9.9"), Ordering::Greater);
        assert_eq!(version_cmp("22.0.0", "20.10.0"), Ordering::Greater);
    }

    #[test]
    fn test_version_cmp_partial() {
        assert_eq!(version_cmp("1.0", "1.0.0"), Ordering::Equal);
        assert_eq!(version_cmp("1", "1.0.0"), Ordering::Equal);
        assert_eq!(version_cmp("2", "1.9.9"), Ordering::Greater);
    }

    #[test]
    fn test_normalize_version() {
        assert_eq!(normalize_version("v1.0.0"), "1.0.0");
        assert_eq!(normalize_version("1.0.0"), "1.0.0");
        assert_eq!(normalize_version("v22.0.0"), "22.0.0");
        assert_eq!(normalize_version("latest"), "latest");
    }

    #[test]
    fn test_extract_domain() {
        assert_eq!(
            extract_domain("https://nodejs.org/dist/v20.0.0/node.tar.gz"),
            "nodejs.org"
        );
        assert_eq!(extract_domain("https://github.com/foo/bar"), "github.com");
        // Invalid URLs return the original string (no :// separator)
        assert_eq!(extract_domain("invalid-url"), "invalid-url");
    }

    #[test]
    fn test_list_installed_versions_empty() {
        let temp = TempDir::new().unwrap();
        let versions = list_installed_versions(temp.path()).unwrap();
        assert!(versions.is_empty());
    }

    #[test]
    fn test_list_installed_versions() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("1.0.0")).unwrap();
        fs::create_dir(temp.path().join("2.0.0")).unwrap();
        fs::create_dir(temp.path().join("current")).unwrap(); // Should be excluded

        let versions = list_installed_versions(temp.path()).unwrap();
        assert_eq!(versions.len(), 2);
        assert!(versions.contains(&"1.0.0".to_string()));
        assert!(versions.contains(&"2.0.0".to_string()));
        assert!(!versions.contains(&"current".to_string()));
    }

    #[test]
    fn test_get_current_version_none() {
        let temp = TempDir::new().unwrap();
        assert!(get_current_version(temp.path()).is_none());
    }

    #[test]
    #[cfg(unix)]
    fn test_set_and_get_current_version() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("1.0.0")).unwrap();

        set_current_version(temp.path(), "1.0.0").unwrap();
        assert_eq!(get_current_version(temp.path()), Some("1.0.0".to_string()));
    }

    #[test]
    fn test_set_current_version_not_installed() {
        let temp = TempDir::new().unwrap();
        let result = set_current_version(temp.path(), "1.0.0");
        assert!(result.is_err());
    }
}
