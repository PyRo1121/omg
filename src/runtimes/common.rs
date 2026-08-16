//! Common utilities for runtime managers
//!
//! Shared functionality for downloading, extracting, and managing runtime versions.

use std::cmp::Ordering;
use std::fs::{self, File};
use std::io::{BufReader, Write};
use std::path::Path;

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use sha2::{Digest, Sha256};

use crate::core::archive::stripped_archive_path;

/// Progress bar style for downloads
#[expect(clippy::expect_used)] // Path operations on known-valid HOME directory; failure is unrecoverable
pub fn download_progress_style() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template(
            "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        )
        .expect("valid template")
        .progress_chars("█▓▒░")
}

/// Progress bar style for extraction
#[expect(clippy::expect_used)] // Path operations on known-valid HOME directory; failure is unrecoverable
pub fn extract_progress_style() -> ProgressStyle {
    ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .expect("valid template")
}

/// Download a file with progress bar and optional checksum verification
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

    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(parent)
        .await
        .with_context(|| format!("Failed to create parent directory: {}", parent.display()))?;

    // Stream into a same-filesystem temporary file so a failed, aborted, or
    // checksum-mismatched download never leaves a partial artifact at `dest`.
    let temporary = tempfile::Builder::new()
        .prefix(".download-")
        .tempfile_in(parent)
        .with_context(|| format!("Failed to create temporary download for {}", dest.display()))?;
    let (std_file, temporary_path) = temporary.into_parts();
    let mut file = tokio::fs::File::from_std(std_file);

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    let mut hasher = expected_sha256.is_some().then(Sha256::new);

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
    file.sync_all()
        .await
        .with_context(|| format!("Failed to sync download to: {}", dest.display()))?;
    drop(file);

    // Verify checksum before publishing the download to its final path.
    if let Some(expected) = expected_sha256 {
        let actual = hasher
            .map(|hasher| hex::encode(hasher.finalize()))
            .ok_or_else(|| anyhow::anyhow!("Checksum verifier was not initialized"))?;
        let expected = expected.trim();

        if !actual.eq_ignore_ascii_case(expected) {
            anyhow::bail!(
                "Checksum mismatch!\n  Expected: {expected}\n  Got: {actual}\n\nThis could indicate a corrupted download or security issue."
            );
        }
        pb.println(format!("  {} Checksum verified", "✓".green()));
    }

    temporary_path
        .persist(dest)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to finalize download: {}", dest.display()))?;
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
            let Some(stripped) = stripped_archive_path(&path, strip_components)? else {
                continue;
            };

            let dest_path = dest_dir.join(&stripped);
            pb.set_message(format!("Extracting: {}", stripped.display()));

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let entry_type = entry.header().entry_type();
            if entry_type.is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else if entry_type.is_file() {
                entry.unpack(&dest_path)?;
            } else {
                anyhow::bail!(
                    "Unsupported link or special entry in runtime archive: {}",
                    path.display()
                );
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

        // Pure Rust XZ decompression with size limit to prevent zip bombs
        const MAX_DECOMPRESSED_SIZE: usize = 2 * 1024 * 1024 * 1024; // 2 GB limit for runtimes
        let mut decompressed = Vec::new();
        lzma_rs::xz_decompress(&mut BufReader::new(file), &mut decompressed)
            .context("Failed to decompress XZ archive")?;

        if decompressed.len() > MAX_DECOMPRESSED_SIZE {
            anyhow::bail!(
                "Decompressed archive too large: {} bytes exceeds {} byte limit",
                decompressed.len(),
                MAX_DECOMPRESSED_SIZE
            );
        }

        pb.set_message("Extracting...");

        let mut archive = tar::Archive::new(decompressed.as_slice());
        fs::create_dir_all(&dest_dir)?;

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;

            let Some(stripped) = stripped_archive_path(&path, strip_components)? else {
                continue;
            };

            let dest_path = dest_dir.join(&stripped);

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            let entry_type = entry.header().entry_type();
            if entry_type.is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else if entry_type.is_file() {
                entry.unpack(&dest_path)?;
            } else {
                anyhow::bail!(
                    "Unsupported link or special entry in runtime archive: {}",
                    path.display()
                );
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
            let path = file.enclosed_name().ok_or_else(|| {
                anyhow::anyhow!("Unsafe path in runtime ZIP archive: {}", file.name())
            })?;
            let Some(stripped) = stripped_archive_path(&path, strip_components)? else {
                continue;
            };
            if file.is_symlink() {
                anyhow::bail!(
                    "Unsupported symlink entry in runtime ZIP archive: {}",
                    path.display()
                );
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

const INSTALL_MARKER: &str = ".omg-install-complete";

/// Best-effort file removal for leftover archives and stale current links.
/// Failure only wastes cache space or leaves a dangling symlink; the next
/// successful install or activation repairs it.
pub fn remove_file_best_effort(path: &Path, kind: &str) {
    if let Err(error) = fs::remove_file(path) {
        tracing::debug!("Failed to remove {kind} {}: {error}", path.display());
    }
}

/// Begin a same-filesystem staged runtime install.
///
/// Extraction writes into the returned staging directory, which only becomes
/// the final version directory when [`complete_staged_install`] publishes it
/// after a successful extraction. An interrupted install therefore never
/// leaves a version directory that looks installed.
pub fn begin_staged_install(versions_dir: &Path) -> Result<tempfile::TempDir> {
    fs::create_dir_all(versions_dir).with_context(|| {
        format!(
            "Failed to create runtime versions directory: {}",
            versions_dir.display()
        )
    })?;
    tempfile::Builder::new()
        .prefix(".install-")
        .tempdir_in(versions_dir)
        .with_context(|| {
            format!(
                "Failed to create runtime staging directory in {}",
                versions_dir.display()
            )
        })
}

/// Atomically publish a staged runtime installation.
///
/// Writes the completion marker, then renames the staging directory into its
/// final path on the same filesystem. Fails if the final path appeared during
/// staging (for example, a concurrent install).
pub fn complete_staged_install(
    staging: &tempfile::TempDir,
    version_dir: &Path,
    version: &str,
) -> Result<()> {
    write_install_marker(staging.path(), version)?;
    if fs::symlink_metadata(version_dir).is_ok() {
        anyhow::bail!(
            "Runtime installation appeared during staging: {}",
            version_dir.display()
        );
    }
    fs::rename(staging.path(), version_dir).with_context(|| {
        format!(
            "Failed to publish runtime installation at {}",
            version_dir.display()
        )
    })?;
    Ok(())
}

/// Atomically replace a published runtime directory with a staged successor.
///
/// The existing version is moved aside first. If publishing the staged tree
/// fails, the previous directory is restored. A crash after the old tree is
/// moved aside leaves no published version directory, which is fail-closed:
/// the next lookup treats the toolchain as uninstalled instead of half-updated.
pub fn replace_staged_install(
    staging: &tempfile::TempDir,
    version_dir: &Path,
    version: &str,
) -> Result<()> {
    write_install_marker(staging.path(), version)?;
    if !is_valid_version_dir(version_dir) {
        anyhow::bail!(
            "Cannot replace missing or invalid runtime version: {}",
            version_dir.display()
        );
    }
    let parent = version_dir.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "Runtime version path has no parent directory: {}",
            version_dir.display()
        )
    })?;
    let backup = tempfile::Builder::new()
        .prefix(".replace-")
        .tempfile_in(parent)
        .with_context(|| {
            format!(
                "Failed to reserve replacement backup path in {}",
                parent.display()
            )
        })?
        .into_temp_path();
    fs::remove_file(&backup).with_context(|| {
        format!(
            "Failed to prepare replacement backup path: {}",
            backup.display()
        )
    })?;
    fs::rename(version_dir, &backup).with_context(|| {
        format!(
            "Failed to move existing runtime version aside: {}",
            version_dir.display()
        )
    })?;
    if let Err(error) = fs::rename(staging.path(), version_dir) {
        if let Err(restore_error) = fs::rename(&backup, version_dir) {
            return Err(error).with_context(|| {
                format!(
                    "Failed to publish replacement at {} and failed to restore previous version: {restore_error}",
                    version_dir.display()
                )
            });
        }
        return Err(error).with_context(|| {
            format!(
                "Failed to publish replacement runtime version at {}",
                version_dir.display()
            )
        });
    }
    if let Err(error) = fs::remove_dir_all(&backup) {
        tracing::warn!(
            "Failed to remove replaced runtime backup {}: {error}",
            backup.display()
        );
    }
    let _ = backup.keep();
    Ok(())
}

/// Copy a directory tree of regular files and directories only.
///
/// Symlinks and special files are rejected so a staged replacement cannot
/// inherit a link that later escapes the published version directory.
pub fn copy_regular_tree(src: &Path, dest: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(src)
        .with_context(|| format!("Failed to inspect source path: {}", src.display()))?;
    if !metadata.is_dir() {
        anyhow::bail!("Source is not a regular directory: {}", src.display());
    }
    fs::create_dir_all(dest)
        .with_context(|| format!("Failed to create destination directory: {}", dest.display()))?;
    copy_regular_tree_contents(src, dest)
}

fn copy_regular_tree_contents(src: &Path, dest: &Path) -> Result<()> {
    for entry in
        fs::read_dir(src).with_context(|| format!("Failed to read directory: {}", src.display()))?
    {
        let entry = entry
            .with_context(|| format!("Failed to read directory entry in {}", src.display()))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to inspect {}", entry.path().display()))?;
        let dest_path = dest.join(entry.file_name());
        if file_type.is_dir() {
            fs::create_dir_all(&dest_path)
                .with_context(|| format!("Failed to create directory: {}", dest_path.display()))?;
            copy_regular_tree_contents(&entry.path(), &dest_path)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &dest_path).with_context(|| {
                format!(
                    "Failed to copy {} to {}",
                    entry.path().display(),
                    dest_path.display()
                )
            })?;
        } else {
            anyhow::bail!(
                "Refusing to copy symlink or special file: {}",
                entry.path().display()
            );
        }
    }
    Ok(())
}

fn write_install_marker(version_dir: &Path, version: &str) -> Result<()> {
    let mut marker = tempfile::NamedTempFile::new_in(version_dir).with_context(|| {
        format!(
            "Failed to create install marker in {}",
            version_dir.display()
        )
    })?;
    writeln!(marker, "{version}")?;
    marker.as_file_mut().sync_all()?;
    marker
        .persist(version_dir.join(INSTALL_MARKER))
        .map_err(|error| error.error)
        .with_context(|| {
            format!(
                "Failed to persist install marker in {}",
                version_dir.display()
            )
        })?;
    Ok(())
}

/// Return whether a runtime version path is a real directory, not a symlink or file.
#[must_use]
pub fn is_valid_version_dir(version_dir: &Path) -> bool {
    fs::symlink_metadata(version_dir).is_ok_and(|metadata| metadata.is_dir())
}

/// Require a regular file at `path`. Symlinks, directories, and missing paths fail closed.
pub fn require_regular_file(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(()),
        Ok(_) => anyhow::bail!(
            "Expected a regular file at {}, found a symlink or special path",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("Missing required runtime binary: {}", path.display())
        }
        Err(error) => Err(error)
            .with_context(|| format!("Failed to inspect runtime binary: {}", path.display())),
    }
}

/// Activate a runtime version only after its expected binary is a regular file.
pub fn activate_version(versions_dir: &Path, version: &str, expected_binary: &Path) -> Result<()> {
    require_regular_file(&versions_dir.join(version).join(expected_binary))?;
    set_current_version(versions_dir, version)
}

/// Create or update the "current" symlink
pub fn set_current_version(versions_dir: &Path, version: &str) -> Result<()> {
    crate::core::security::validate_runtime_version(version)?;

    let current_link = versions_dir.join("current");
    let version_dir = versions_dir.join(version);

    if !is_valid_version_dir(&version_dir) {
        anyhow::bail!(
            "Version {version} is not installed as a valid directory. Install it first with: omg use <runtime>@{version}"
        );
    }

    #[cfg(not(unix))]
    anyhow::bail!("Runtime version switching is unsupported on this platform");

    #[cfg(unix)]
    {
        match fs::symlink_metadata(&current_link) {
            Ok(metadata) if !metadata.file_type().is_symlink() => {
                anyhow::bail!(
                    "Refusing to replace non-symlink current runtime path: {}",
                    current_link.display()
                );
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to inspect current runtime path: {}",
                        current_link.display()
                    )
                });
            }
        }

        let temp_link = tempfile::Builder::new()
            .prefix(".current-")
            .tempfile_in(versions_dir)
            .with_context(|| {
                format!(
                    "Failed to create temporary runtime link in {}",
                    versions_dir.display()
                )
            })?
            .into_temp_path();
        fs::remove_file(&temp_link).with_context(|| {
            format!(
                "Failed to prepare temporary runtime link: {}",
                temp_link.display()
            )
        })?;
        std::os::unix::fs::symlink(&version_dir, &temp_link).with_context(|| {
            format!(
                "Failed to create temporary runtime link: {}",
                temp_link.display()
            )
        })?;
        fs::rename(&temp_link, &current_link).with_context(|| {
            format!(
                "Failed to activate runtime version {version} at {}",
                current_link.display()
            )
        })?;

        Ok(())
    }
}

/// Get the current version from the "current" symlink
pub fn get_current_version(versions_dir: &Path) -> Option<String> {
    let current_link = versions_dir.join("current");
    fs::read_link(&current_link)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
}

/// List installed versions in a directory
pub fn list_installed_versions(versions_dir: &Path) -> Result<Vec<String>> {
    if !versions_dir.exists() {
        return Ok(Vec::new());
    }

    let mut versions = Vec::new();
    for entry in fs::read_dir(versions_dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // Skip the "current" symlink and dot-prefixed entries (e.g. staging
        // directories left by an interrupted install, which must never be
        // reported as installed versions).
        if name.starts_with('.') || name == "current" || !is_valid_version_dir(&entry.path()) {
            continue;
        }
        versions.push(name);
    }

    versions.sort_by(|a, b| version_cmp(b, a));
    Ok(versions)
}

/// Compare semantic version strings (descending order)
pub fn version_cmp(a: &str, b: &str) -> Ordering {
    let parse_parts = |s: &str| -> Vec<u32> {
        s.split(|c: char| !c.is_ascii_digit())
            .filter_map(|p| p.parse().ok())
            .collect()
    };

    let a_parts = parse_parts(a);
    let b_parts = parse_parts(b);
    let max_len = a_parts.len().max(b_parts.len());

    (0..max_len)
        .map(|i| {
            let a_part = a_parts.get(i).copied().unwrap_or(0);
            let b_part = b_parts.get(i).copied().unwrap_or(0);
            a_part.cmp(&b_part)
        })
        .find(|&ord| ord != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

/// Normalize version string (remove leading 'v' if present)
pub fn normalize_version(version: &str) -> String {
    version.trim_start_matches('v').to_owned()
}

/// Parse and validate a SHA-256 digest returned by a runtime vendor.
///
/// Vendors may serve the digest alone or as `"<hex>  <filename>"`; only the
/// digest is returned. Rejects anything that is not exactly 64 hex characters.
pub fn parse_sha256_digest(value: &str, source: &str) -> Result<String> {
    let digest = value
        .split_whitespace()
        .next()
        .and_then(|digest| digest.strip_prefix("sha256:").or(Some(digest)))
        .filter(|digest| digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()))
        .ok_or_else(|| anyhow::anyhow!("Invalid SHA-256 digest returned by {source}"))?;
    Ok(digest.to_ascii_lowercase())
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
    let check_green = "✓".green();
    let check = check_green.bold();
    let rt = runtime.cyan();
    let ver = version.yellow();
    tracing::info!("\n{check} {rt} {ver} installed successfully!");
}

/// Print version switch message
pub fn print_using(runtime: &str, version: &str, bin_path: &Path) {
    // Bind styled temporaries to avoid Rust 2024 drop order issues
    let check = "✓".green();
    let rt = runtime.cyan();
    let ver = version.yellow();
    tracing::info!("{check} Now using {rt} {ver}");

    let path_label = "PATH:".dimmed();
    let path_display = bin_path.display();
    tracing::info!("  {path_label} {path_display}");
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
                $crate::core::security::validate_runtime_version(&version)?;
                let version_dir = self.versions_dir.join(&version);

                match fs::symlink_metadata(&version_dir) {
                    Ok(metadata) if metadata.is_dir() => {}
                    Ok(_) => {
                        anyhow::bail!(
                            "Refusing to remove non-directory runtime path: {}",
                            version_dir.display()
                        );
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        println!(
                            "{} {} {} is not installed",
                            "→".dimmed(),
                            $runtime_name,
                            version
                        );
                        return Ok(());
                    }
                    Err(error) => {
                        return Err(error).with_context(|| {
                            format!("Failed to inspect runtime path: {}", version_dir.display())
                        });
                    }
                }

                // Clear current link if uninstalling the active version
                if let Some(current) = self.current_version()
                    && current == version
                {
                    $crate::runtimes::common::remove_file_best_effort(
                        &self.current_link,
                        "current runtime symlink",
                    );
                }

                fs::remove_dir_all(&version_dir).with_context(|| {
                    format!("Failed to remove {} directory", version_dir.display())
                })?;

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

    fn tar_archive(entry_type: tar::EntryType) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut builder = tar::Builder::new(&mut bytes);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(entry_type);
        header.set_mode(0o755);
        header.set_size(if entry_type.is_file() { 4 } else { 0 });
        header.set_cksum();
        builder
            .append_data(&mut header, "runtime/bin/tool", &b"tool"[..])
            .unwrap();
        builder.finish().unwrap();
        drop(builder);
        bytes
    }

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
    fn parse_sha256_digest_accepts_a_vendor_manifest_line() -> Result<()> {
        let digest = "A".repeat(64);
        assert_eq!(
            parse_sha256_digest(
                &format!("{digest}  node-v20.0.0-linux-x64.tar.xz"),
                "nodejs.org"
            )?,
            digest.to_lowercase()
        );
        // Digest-only payloads (go.dev style) are also accepted.
        assert_eq!(
            parse_sha256_digest(&digest, "go.dev")?,
            digest.to_lowercase()
        );
        assert_eq!(
            parse_sha256_digest(&format!("sha256:{digest}"), "GitHub")?,
            digest.to_lowercase()
        );
        Ok(())
    }

    #[test]
    fn parse_sha256_digest_rejects_malformed_values() {
        assert!(parse_sha256_digest("not-a-digest", "nodejs.org").is_err());
        assert!(parse_sha256_digest(&"a".repeat(63), "nodejs.org").is_err());
        assert!(parse_sha256_digest(&"z".repeat(64), "nodejs.org").is_err());
    }

    #[tokio::test]
    async fn tar_gz_extraction_rejects_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let archive_path = temp.path().join("runtime.tar.gz");
        let gz = {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            std::io::Write::write_all(&mut encoder, &tar_archive(tar::EntryType::symlink()))?;
            encoder.finish()?
        };
        fs::write(&archive_path, gz)?;

        let error = extract_tar_gz(&archive_path, temp.path().join("out").as_path(), 1)
            .await
            .expect_err("symlink entries must be rejected");
        assert!(
            error
                .to_string()
                .contains("Unsupported link or special entry")
        );
        Ok(())
    }

    #[tokio::test]
    async fn tar_xz_extraction_accepts_regular_files() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let archive_path = temp.path().join("runtime.tar.xz");
        let mut compressed = Vec::new();
        lzma_rs::xz_compress(
            &mut std::io::Cursor::new(tar_archive(tar::EntryType::Regular)),
            &mut compressed,
        )?;
        fs::write(&archive_path, compressed)?;

        extract_tar_xz(&archive_path, temp.path().join("out").as_path(), 1).await?;
        assert_eq!(fs::read(temp.path().join("out/bin/tool"))?, b"tool");
        Ok(())
    }

    #[test]
    fn staged_install_publishes_marker_on_success() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let versions_dir = temp.path().join("versions");
        let version_dir = versions_dir.join("1.0.0");

        let staging = begin_staged_install(&versions_dir)?;
        fs::write(staging.path().join("bin"), "binary")?;
        complete_staged_install(&staging, &version_dir, "1.0.0")?;

        assert!(version_dir.join("bin").is_file());
        assert_eq!(
            fs::read_to_string(version_dir.join(INSTALL_MARKER))?,
            "1.0.0\n"
        );
        assert_eq!(
            list_installed_versions(&versions_dir)?,
            vec!["1.0.0".to_string()]
        );
        Ok(())
    }

    #[test]
    fn interrupted_staged_install_leaves_no_version_or_staging_entry() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let versions_dir = temp.path().join("versions");
        let version_dir = versions_dir.join("1.0.0");

        {
            let staging = begin_staged_install(&versions_dir)?;
            fs::write(staging.path().join("bin"), "partial")?;
            // Simulate an interrupted install: staging drops without publishing.
        }

        assert!(!version_dir.exists());
        assert!(list_installed_versions(&versions_dir)?.is_empty());
        Ok(())
    }

    #[test]
    fn staged_install_refuses_to_clobber_an_existing_version() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let versions_dir = temp.path().join("versions");
        let version_dir = versions_dir.join("1.0.0");
        fs::create_dir_all(&version_dir)?;

        let staging = begin_staged_install(&versions_dir)?;
        let error = complete_staged_install(&staging, &version_dir, "1.0.0")
            .expect_err("must refuse to publish over an existing version");
        assert!(error.to_string().contains("appeared during staging"));
        Ok(())
    }

    #[test]
    fn replace_staged_install_swaps_the_published_directory() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let versions_dir = temp.path().join("versions");
        let version_dir = versions_dir.join("1.0.0");
        fs::create_dir_all(&version_dir)?;
        fs::write(version_dir.join("bin"), "old")?;

        let staging = begin_staged_install(&versions_dir)?;
        fs::write(staging.path().join("bin"), "new")?;
        replace_staged_install(&staging, &version_dir, "1.0.0")?;

        assert_eq!(fs::read_to_string(version_dir.join("bin"))?, "new");
        assert_eq!(
            fs::read_to_string(version_dir.join(INSTALL_MARKER))?,
            "1.0.0\n"
        );
        assert_eq!(
            list_installed_versions(&versions_dir)?,
            vec!["1.0.0".to_string()]
        );
        Ok(())
    }

    #[test]
    #[cfg(unix)]
    fn copy_regular_tree_rejects_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let src = temp.path().join("src");
        let dest = temp.path().join("dest");
        fs::create_dir_all(&src)?;
        std::os::unix::fs::symlink("/tmp", src.join("link"))?;

        let error = copy_regular_tree(&src, &dest).expect_err("symlinks must be rejected");
        assert!(error.to_string().contains("symlink or special file"));
        Ok(())
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
    #[cfg(unix)]
    fn version_listing_and_activation_reject_symlinked_version_dirs() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("real")).unwrap();
        std::os::unix::fs::symlink("real", temp.path().join("1.0.0")).unwrap();
        fs::write(temp.path().join("2.0.0"), b"not a directory").unwrap();

        let versions = list_installed_versions(temp.path()).unwrap();
        assert_eq!(versions, vec!["real".to_string()]);

        let error = set_current_version(temp.path(), "1.0.0").unwrap_err();
        assert!(error.to_string().contains("valid directory"));
    }

    #[test]
    fn test_get_current_version_none() {
        let temp = TempDir::new().unwrap();
        assert!(get_current_version(temp.path()).is_none());
    }

    #[test]
    #[cfg(unix)]
    fn switching_current_version_replaces_the_existing_link() {
        let temp = TempDir::new().unwrap();
        let first_version = temp.path().join("1.0.0");
        let second_version = temp.path().join("2.0.0");
        fs::create_dir(&first_version).unwrap();
        fs::create_dir(&second_version).unwrap();

        set_current_version(temp.path(), "1.0.0").unwrap();
        set_current_version(temp.path(), "2.0.0").unwrap();

        assert_eq!(
            fs::read_link(temp.path().join("current")).unwrap(),
            second_version
        );
        assert_eq!(get_current_version(temp.path()), Some("2.0.0".to_string()));
        assert!(first_version.is_dir());
    }

    #[test]
    #[cfg(unix)]
    fn missing_version_preserves_the_current_link() {
        let temp = TempDir::new().unwrap();
        let installed_version = temp.path().join("1.0.0");
        fs::create_dir(&installed_version).unwrap();
        set_current_version(temp.path(), "1.0.0").unwrap();

        let error = set_current_version(temp.path(), "2.0.0").unwrap_err();

        assert!(error.to_string().contains("is not installed"));
        assert_eq!(
            fs::read_link(temp.path().join("current")).unwrap(),
            installed_version
        );
    }

    #[test]
    #[cfg(unix)]
    fn activation_refuses_to_replace_a_regular_file() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("1.0.0")).unwrap();
        let current_path = temp.path().join("current");
        fs::write(&current_path, "sentinel").unwrap();

        let error = set_current_version(temp.path(), "1.0.0").unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Refusing to replace non-symlink")
        );
        assert_eq!(fs::read_to_string(current_path).unwrap(), "sentinel");
    }

    #[test]
    fn set_current_version_rejects_missing_versions() {
        let temp = TempDir::new().unwrap();
        let error = set_current_version(temp.path(), "1.0.0").unwrap_err();
        assert!(error.to_string().contains("is not installed"));
    }

    #[test]
    fn activate_version_requires_a_regular_expected_binary() {
        let temp = TempDir::new().unwrap();
        let version_dir = temp.path().join("1.0.0");
        fs::create_dir_all(version_dir.join("bin")).unwrap();

        let missing = activate_version(temp.path(), "1.0.0", Path::new("bin/node")).unwrap_err();
        assert!(
            missing
                .to_string()
                .contains("Missing required runtime binary")
        );
        assert!(!temp.path().join("current").exists());

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("elsewhere", version_dir.join("bin/node")).unwrap();
            let linked = activate_version(temp.path(), "1.0.0", Path::new("bin/node")).unwrap_err();
            assert!(linked.to_string().contains("regular file"));
            assert!(!temp.path().join("current").exists());
            fs::remove_file(version_dir.join("bin/node")).unwrap();
        }

        fs::write(version_dir.join("bin/node"), b"node").unwrap();
        activate_version(temp.path(), "1.0.0", Path::new("bin/node")).unwrap();
        #[cfg(unix)]
        assert_eq!(
            fs::read_link(temp.path().join("current")).unwrap(),
            version_dir
        );
    }

    #[test]
    #[cfg(not(unix))]
    fn activation_fails_explicitly_on_unsupported_platforms() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("1.0.0")).unwrap();

        let error = set_current_version(temp.path(), "1.0.0").unwrap_err();

        assert!(error.to_string().contains("unsupported on this platform"));
        assert!(!temp.path().join("current").exists());
    }
}
