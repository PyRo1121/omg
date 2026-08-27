//! Common utilities for runtime managers
//!
//! Shared functionality for downloading, extracting, and managing runtime versions.

use std::cmp::Ordering;
use std::fs::{self, File};
#[cfg(any(feature = "debian", feature = "debian-pure"))]
use std::io::Read;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::core::archive::stripped_archive_path;

pub(crate) const GITHUB_USER_AGENT: &str = "omg-package-manager/0.1";

#[derive(Debug, Deserialize)]
pub(crate) struct GithubRelease {
    pub(crate) tag_name: String,
    #[serde(default)]
    pub(crate) prerelease: bool,
    #[serde(default)]
    pub(crate) assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GithubAsset {
    pub(crate) name: String,
    pub(crate) browser_download_url: Option<String>,
    pub(crate) digest: Option<String>,
}

pub(crate) fn host_os_tag(
    runtime: &str,
    linux: &'static str,
    macos: &'static str,
) -> Result<&'static str> {
    match std::env::consts::OS {
        "linux" => Ok(linux),
        "macos" => Ok(macos),
        other => anyhow::bail!("Unsupported operating system for {runtime}: {other}"),
    }
}

pub(crate) fn host_arch_tag(
    runtime: &str,
    x86_64: &'static str,
    aarch64: &'static str,
) -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok(x86_64),
        "aarch64" => Ok(aarch64),
        other => anyhow::bail!("Unsupported architecture for {runtime}: {other}"),
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Bounded decompression
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Maximum accepted decompressed size for untrusted archive payloads (2 GiB).
///
/// The bound is enforced *while* bytes are produced, so a decompression bomb
/// aborts with an error instead of exhausting memory before a post-hoc size
/// check can run.
pub(crate) const MAX_DECOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// Reader that enforces a decompressed-size budget while streaming.
///
/// Every read that would push the cumulative output past `budget` fails with
/// [`ErrorKind::InvalidData`], so downstream consumers never buffer more than
/// the budget. Only Debian-side extraction consumes this today; runtime
/// extraction uses [`BudgetedSink`] because xz has no streaming decoder.
#[cfg(any(feature = "debian", feature = "debian-pure"))]
pub(crate) struct BudgetedReader<R> {
    inner: R,
    remaining: u64,
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
impl<R> BudgetedReader<R> {
    /// Explicit budget: production callers pass [`MAX_DECOMPRESSED_BYTES`],
    /// tests pass a small budget so the abort path is exercisable without
    /// gigabyte allocations.
    pub(crate) fn new(inner: R, budget: u64) -> Self {
        Self {
            inner,
            remaining: budget,
        }
    }
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
impl<R: Read> Read for BudgetedReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buf)?;
        let read = u64::try_from(read).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "impossible read size while decompressing",
            )
        })?;
        if read > self.remaining {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "decompressed data exceeds the maximum supported size of {MAX_DECOMPRESSED_BYTES} bytes"
                ),
            ));
        }
        self.remaining -= read;
        Ok(read as usize)
    }
}

/// In-memory sink that refuses to grow past a decompressed-size budget.
///
/// Used with compressors that only expose a `Read -> Write` API (xz), where a
/// budgeted reader is not available: the sink errors as soon as the budget is
/// exhausted, so the backing buffer stops growing at the cap instead of after
/// decompression has completed.
pub(crate) struct BudgetedSink {
    buf: Vec<u8>,
    remaining: u64,
}

impl BudgetedSink {
    pub(crate) fn with_default_budget() -> Self {
        Self {
            buf: Vec::new(),
            remaining: MAX_DECOMPRESSED_BYTES,
        }
    }

    /// The configured maximum budget, for callers that delegate the choice.
    /// Only Debian-side extraction delegates today.
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    pub(crate) fn max_budget() -> u64 {
        MAX_DECOMPRESSED_BYTES
    }

    /// Explicit budget: production callers pass [`MAX_DECOMPRESSED_BYTES`],
    /// tests pass a small budget so the abort path is exercisable without
    /// gigabyte allocations. Only Debian-side extraction needs a custom
    /// budget today.
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    pub(crate) fn with_budget(budget: u64) -> Self {
        Self {
            buf: Vec::new(),
            remaining: budget,
        }
    }

    pub(crate) fn into_inner(self) -> Vec<u8> {
        self.buf
    }
}

impl Write for BudgetedSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = buf.len() as u64;
        if len > self.remaining {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "decompressed data exceeds the maximum supported size of {MAX_DECOMPRESSED_BYTES} bytes"
                ),
            ));
        }
        self.remaining -= len;
        self.buf.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Progress bar style for downloads
#[expect(clippy::expect_used)] // Path operations on known-valid HOME directory; failure is unrecoverable
fn download_progress_style() -> ProgressStyle {
    ProgressStyle::default_bar()
        .template(
            "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})",
        )
        .expect("valid template")
        .progress_chars("█▓▒░")
}

/// Progress bar style for extraction
#[expect(clippy::expect_used)] // Path operations on known-valid HOME directory; failure is unrecoverable
fn extract_progress_style() -> ProgressStyle {
    ProgressStyle::default_spinner()
        .template("{spinner:.green} {msg}")
        .expect("valid template")
}

/// Download a file with progress bar and checksum verification.
pub async fn download_with_progress(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    expected_sha256: &str,
) -> Result<()> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    let response = client
        .get(url)
        .header("User-Agent", GITHUB_USER_AGENT)
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
    let mut hasher = Sha256::new();

    while let Some(item) = stream.next().await {
        let chunk = item.context("Error downloading chunk")?;
        file.write_all(&chunk)
            .await
            .context("Error writing to file")?;

        hasher.update(&chunk);

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
    let actual = hex::encode(hasher.finalize());
    let expected = expected_sha256.trim();
    if !actual.eq_ignore_ascii_case(expected) {
        anyhow::bail!(
            "Checksum mismatch!\n  Expected: {expected}\n  Got: {actual}\n\nThis could indicate a corrupted download or security issue."
        );
    }
    pb.println(format!("  {} Checksum verified", "✓".green()));

    temporary_path
        .persist(dest)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to finalize download: {}", dest.display()))?;
    pb.finish_and_clear();
    Ok(())
}

enum PendingArchiveLink {
    Symbolic { path: PathBuf, target: PathBuf },
    Hard { path: PathBuf, target: PathBuf },
}

fn validate_relative_symlink_target(link_path: &Path, target: &Path) -> Result<()> {
    let mut resolved = link_path
        .parent()
        .map_or_else(PathBuf::new, Path::to_path_buf);
    for component in target.components() {
        match component {
            std::path::Component::Normal(part) => resolved.push(part),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                anyhow::ensure!(
                    resolved.pop(),
                    "Archive symlink escapes the extraction directory: {} -> {}",
                    link_path.display(),
                    target.display()
                );
            }
            std::path::Component::RootDir | std::path::Component::Prefix(_) => {
                anyhow::bail!(
                    "Archive symlink target must be relative: {} -> {}",
                    link_path.display(),
                    target.display()
                );
            }
        }
    }
    Ok(())
}

fn create_archive_links(links: Vec<PendingArchiveLink>) -> Result<()> {
    for link in links {
        match link {
            PendingArchiveLink::Symbolic { path, target } => {
                #[cfg(unix)]
                std::os::unix::fs::symlink(&target, &path).with_context(|| {
                    format!(
                        "Failed to create archive symlink {} -> {}",
                        path.display(),
                        target.display()
                    )
                })?;
                #[cfg(not(unix))]
                anyhow::bail!(
                    "Runtime archive contains a symbolic link unsupported on this platform: {}",
                    path.display()
                );
            }
            PendingArchiveLink::Hard { path, target } => {
                fs::hard_link(&target, &path).with_context(|| {
                    format!(
                        "Failed to create archive hard link {} -> {}",
                        path.display(),
                        target.display()
                    )
                })?;
            }
        }
    }
    Ok(())
}

/// Process every tar entry into `dest_dir`, deferring symlink/hard-link
/// creation until all regular content exists.
///
/// Shared by [`extract_tar_gz`] and [`extract_tar_xz`]; the decompression
/// strategy differs, the entry handling must not.
fn extract_tar_entries<R: std::io::Read>(
    archive: &mut tar::Archive<R>,
    dest_dir: &Path,
    strip_components: usize,
    pb: &ProgressBar,
) -> Result<()> {
    pb.set_message("Extracting...");
    let mut pending_links = Vec::new();

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
        } else if entry_type.is_symlink() {
            let target = entry
                .link_name()?
                .context("Archive symlink is missing its target")?
                .into_owned();
            validate_relative_symlink_target(&stripped, &target)?;
            pending_links.push(PendingArchiveLink::Symbolic {
                path: dest_path,
                target,
            });
        } else if entry_type.is_hard_link() {
            let target = entry
                .link_name()?
                .context("Archive hard link is missing its target")?;
            let target = stripped_archive_path(&target, strip_components)?
                .context("Archive hard link target was stripped away")?;
            pending_links.push(PendingArchiveLink::Hard {
                path: dest_path,
                target: dest_dir.join(target),
            });
        } else {
            anyhow::bail!(
                "Unsupported special entry in runtime archive: {}",
                path.display()
            );
        }
    }
    create_archive_links(pending_links)
}

/// Extract a .tar.gz archive with progress
pub(crate) async fn extract_tar_gz(
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

        fs::create_dir_all(&dest_dir)?;
        extract_tar_entries(&mut archive, &dest_dir, strip_components, &pb)?;

        pb.finish_and_clear();
        Ok(())
    })
    .await?
}

/// Extract a .tar.xz archive with progress (pure Rust)
pub(crate) async fn extract_tar_xz(
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

        // Pure Rust XZ decompression with the decompressed-size budget
        // enforced during streaming: the sink stops accepting bytes at the
        // budget, so a bomb aborts instead of exhausting memory before a
        // post-hoc size check could run.
        let mut sink = BudgetedSink::with_default_budget();
        lzma_rs::xz_decompress(&mut BufReader::new(file), &mut sink)
            .context("Failed to decompress XZ archive")?;
        let decompressed = sink.into_inner();

        let mut archive = tar::Archive::new(decompressed.as_slice());
        fs::create_dir_all(&dest_dir)?;
        extract_tar_entries(&mut archive, &dest_dir, strip_components, &pb)?;

        pb.finish_and_clear();
        Ok(())
    })
    .await?
}

/// Extract a .zip archive with progress
pub(crate) async fn extract_zip(
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
pub(crate) fn remove_file_best_effort(path: &Path, kind: &str) {
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
pub(crate) fn begin_staged_install(versions_dir: &Path) -> Result<tempfile::TempDir> {
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
pub(crate) fn complete_staged_install(
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
pub(crate) fn replace_staged_install(
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
pub(crate) fn copy_regular_tree(src: &Path, dest: &Path) -> Result<()> {
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
pub(crate) fn is_valid_version_dir(version_dir: &Path) -> bool {
    fs::symlink_metadata(version_dir).is_ok_and(|metadata| metadata.is_dir())
}

/// Return whether a runtime binary directory is safe to prepend to `PATH`.
///
/// It must be a real directory owned by the current user (or root) and not
/// writable by group/other users. This prevents a repository pin from making
/// an attacker-writable runtime tree shadow ordinary commands.
#[must_use]
pub(crate) fn is_trusted_runtime_bin_dir(path: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return false;
    };
    if !metadata.file_type().is_dir() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        let current_uid = nix::unistd::geteuid().as_raw();
        if (metadata.uid() != 0 && metadata.uid() != current_uid) || metadata.mode() & 0o022 != 0 {
            return false;
        }
    }
    true
}

/// Require a regular file at `path`. Symlinks, directories, and missing paths fail closed.
pub(crate) fn require_regular_file(path: &Path) -> Result<()> {
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
pub(crate) fn activate_version(
    versions_dir: &Path,
    version: &str,
    expected_binary: &Path,
) -> Result<()> {
    require_regular_file(&versions_dir.join(version).join(expected_binary))?;
    set_current_version(versions_dir, version)
}

/// Create or update the "current" symlink
pub(crate) fn set_current_version(versions_dir: &Path, version: &str) -> Result<()> {
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
pub(crate) fn get_current_version(versions_dir: &Path) -> Option<String> {
    let current_link = versions_dir.join("current");
    let target = fs::read_link(&current_link).ok()?;
    let version = target.file_name()?.to_string_lossy().into_owned();
    let expected = versions_dir.join(&version);
    if !is_valid_version_dir(&expected) {
        return None;
    }

    let target = if target.is_absolute() {
        target
    } else {
        current_link.parent()?.join(target)
    };
    let target = fs::canonicalize(target).ok()?;
    let expected = fs::canonicalize(expected).ok()?;
    (target == expected).then_some(version)
}

/// List installed versions in a directory
pub(crate) fn list_installed_versions(versions_dir: &Path) -> Result<Vec<String>> {
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

/// Compare version strings by numeric dot-separated parts (ascending).
///
/// Non-numeric segments are ignored, so pre-release suffixes compare equal to
/// their release ("1.0.0-beta" == "1.0.0"); callers produce descending order
/// by swapping arguments (`sort_by(|a, b| version_cmp(b, a))`).
#[must_use]
pub(crate) fn version_cmp(a: &str, b: &str) -> Ordering {
    let a_parts = a
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|part| part.parse::<u32>().ok());
    let b_parts = b
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|part| part.parse::<u32>().ok());
    let max_len = a_parts.clone().count().max(b_parts.clone().count());

    a_parts
        .chain(std::iter::repeat(0))
        .zip(b_parts.chain(std::iter::repeat(0)))
        .take(max_len)
        .map(|(a_part, b_part)| a_part.cmp(&b_part))
        .find(|&ordering| ordering != Ordering::Equal)
        .unwrap_or(Ordering::Equal)
}

/// Normalize version string (remove leading 'v' if present)
/// Normalize a version string by stripping a single leading 'v' prefix.
#[must_use]
pub(crate) fn normalize_version(version: &str) -> String {
    version.strip_prefix('v').unwrap_or(version).to_owned()
}

/// Parse and validate a SHA-256 digest returned by a runtime vendor.
///
/// Vendors may serve the digest alone or as `"<hex>  <filename>"`; only the
/// digest is returned. Rejects anything that is not exactly 64 hex characters.
pub(crate) fn parse_sha256_digest(value: &str, source: &str) -> Result<String> {
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
pub(crate) fn print_installed(runtime: &str, version: &str) {
    let check_green = "✓".green();
    let check = check_green.bold();
    let rt = runtime.cyan();
    let ver = version.yellow();
    tracing::info!("\n{check} {rt} {ver} installed successfully!");
}

/// Print version switch message
pub(crate) fn print_using(runtime: &str, version: &str, bin_path: &Path) {
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
pub(crate) fn print_already_installed(runtime: &str, version: &str) {
    tracing::info!(
        "{} {} {} is already installed",
        "✓".green(),
        runtime.cyan(),
        version.yellow()
    );
}

/// Implement the shared runtime-manager methods for a manager with a
/// `versions_dir` field.
macro_rules! impl_runtime_common {
    ($manager_type:ty) => {
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
        }
    };
}
pub(crate) use impl_runtime_common;

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::expect_used)] // Idiomatic in tests: panics on failure with clear error context
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
    fn github_release_decodes_runtime_specific_payload_subsets() {
        let minimal: GithubRelease = serde_json::from_str(
            r#"{"tag_name":"v1.2.3","assets":[{"name":"runtime.zip","digest":"sha256:abc"}]}"#,
        )
        .unwrap();
        assert!(!minimal.prerelease);
        assert!(minimal.assets[0].browser_download_url.is_none());

        let downloadable: GithubRelease = serde_json::from_str(
            r#"{"tag_name":"v1.2.3","prerelease":true,"assets":[{"name":"runtime.tgz","browser_download_url":"https://example.invalid/runtime.tgz"}]}"#,
        )
        .unwrap();
        assert!(downloadable.prerelease);
        assert_eq!(
            downloadable.assets[0].browser_download_url.as_deref(),
            Some("https://example.invalid/runtime.tgz")
        );
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

    fn tar_archive_with_symlink(target: &str) -> anyhow::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        let mut builder = tar::Builder::new(&mut bytes);

        let mut file_header = tar::Header::new_gnu();
        file_header.set_entry_type(tar::EntryType::Regular);
        file_header.set_mode(0o755);
        file_header.set_size(4);
        file_header.set_cksum();
        builder.append_data(&mut file_header, "runtime/lib/tool", &b"tool"[..])?;

        let mut link_header = tar::Header::new_gnu();
        link_header.set_entry_type(tar::EntryType::Symlink);
        link_header.set_mode(0o755);
        link_header.set_size(0);
        link_header.set_link_name(target)?;
        link_header.set_cksum();
        builder.append_data(&mut link_header, "runtime/bin/tool", std::io::empty())?;
        builder.finish()?;
        drop(builder);
        Ok(bytes)
    }

    #[tokio::test]
    async fn tar_gz_extraction_rejects_escaping_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let archive_path = temp.path().join("runtime.tar.gz");
        let gz = {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            std::io::Write::write_all(&mut encoder, &tar_archive_with_symlink("../../outside")?)?;
            encoder.finish()?
        };
        fs::write(&archive_path, gz)?;

        let error = extract_tar_gz(&archive_path, temp.path().join("out").as_path(), 1)
            .await
            .expect_err("escaping symlink entries must be rejected");
        assert!(
            error
                .to_string()
                .contains("escapes the extraction directory")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn tar_gz_extraction_accepts_internal_symlinks() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let archive_path = temp.path().join("runtime.tar.gz");
        let gz = {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            std::io::Write::write_all(&mut encoder, &tar_archive_with_symlink("../lib/tool")?)?;
            encoder.finish()?
        };
        fs::write(&archive_path, gz)?;

        let output = temp.path().join("out");
        extract_tar_gz(&archive_path, &output, 1).await?;
        assert_eq!(fs::read(output.join("bin/tool"))?, b"tool");
        assert!(
            fs::symlink_metadata(output.join("bin/tool"))?
                .file_type()
                .is_symlink()
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
    fn current_version_rejects_missing_or_external_targets() {
        let temp = TempDir::new().unwrap();
        fs::create_dir(temp.path().join("1.0.0")).unwrap();
        let current = temp.path().join("current");

        std::os::unix::fs::symlink("missing", &current).unwrap();
        assert!(get_current_version(temp.path()).is_none());
        fs::remove_file(&current).unwrap();

        let outside_root = TempDir::new().unwrap();
        let outside = outside_root.path().join("1.0.0");
        fs::create_dir(&outside).unwrap();
        std::os::unix::fs::symlink(&outside, &current).unwrap();
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

    #[cfg(unix)]
    #[test]
    fn runtime_path_rejects_group_writable_directories() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o775)).unwrap();

        assert!(!is_trusted_runtime_bin_dir(&bin));
        fs::set_permissions(&bin, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(is_trusted_runtime_bin_dir(&bin));
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

    /// Build a gzip bomb: `expanded` bytes of zeros compress to a few KiB.
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    fn gzip_bomb(expanded: usize) -> Vec<u8> {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Read as _;

        let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
        std::io::copy(
            &mut std::io::repeat(0u8).take(expanded as u64),
            &mut encoder,
        )
        .unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    fn budgeted_reader_aborts_a_decompression_bomb_during_streaming() {
        let bomb = gzip_bomb(64 * 1024 * 1024); // 64 MiB of zeros compresses to ~60 KiB
        let decoder = flate2::read::GzDecoder::new(bomb.as_slice());
        let mut reader = BudgetedReader::new(decoder, 1024 * 1024); // 1 MiB budget

        let mut sink = Vec::new();
        let error = reader
            .read_to_end(&mut sink)
            .expect_err("budget must abort the stream");

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("maximum supported size"));
        // Allocation stopped at the budget instead of expanding to 64 MiB.
        assert!(sink.len() <= 1024 * 1024 + 64 * 1024);
    }

    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    #[test]
    fn budgeted_sink_stops_growing_at_the_budget() {
        let mut sink = BudgetedSink::with_budget(16);

        let ok = sink.write(&[0u8; 10]).unwrap();
        let error = sink.write(&[0u8; 32]).expect_err("budget must abort");

        assert_eq!(ok, 10);
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert_eq!(sink.into_inner().len(), 10);
    }
}
