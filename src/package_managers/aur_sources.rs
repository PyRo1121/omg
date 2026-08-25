//! Parallel source downloading for AUR packages
//!
//! This module parses .SRCINFO files to extract HTTP/HTTPS sources and downloads
//! them concurrently before makepkg runs. This dramatically improves build times
//! for packages with multiple source files (10-60 second savings).

use std::path::Path;

use alpm_srcinfo::SourceInfoV1;
use alpm_types::SystemArchitecture;
use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, warn};

use crate::core::http::shared_client;

/// Represents a source file that can be downloaded
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Full URL to download from
    pub url: String,
    /// Base filename (may be renamed via :: syntax)
    pub filename: String,
}

/// Get the current system architecture
fn current_arch() -> Option<SystemArchitecture> {
    match std::env::consts::ARCH {
        "x86_64" => Some(SystemArchitecture::X86_64),
        "aarch64" => Some(SystemArchitecture::Aarch64),
        "arm" => Some(SystemArchitecture::Arm),
        "i686" => Some(SystemArchitecture::I686),
        _ => None,
    }
}

/// Parse .SRCINFO to extract HTTP/HTTPS source URLs
///
/// Returns a list of downloadable sources that makepkg would normally fetch.
/// Only includes http:// and https:// URLs, skipping local files and git repos.
pub fn parse_sources(pkg_dir: &Path) -> Result<Vec<SourceFile>> {
    let srcinfo_path = pkg_dir.join(".SRCINFO");
    if !srcinfo_path.exists() {
        debug!("No .SRCINFO found at {}", srcinfo_path.display());
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&srcinfo_path)
        .with_context(|| format!("Failed to read .SRCINFO at {}", srcinfo_path.display()))?;

    let srcinfo = SourceInfoV1::from_string(&content).context("Failed to parse .SRCINFO")?;

    let mut sources = Vec::new();

    // Parse common sources (apply to all architectures)
    for source in &srcinfo.base.sources {
        let source_str = source.to_string();
        if let Some(source_file) = extract_http_source(&source_str) {
            sources.push(source_file);
        }
    }

    // Also parse architecture-specific sources
    if let Some(arch) = current_arch()
        && let Some(arch_props) = srcinfo.base.architecture_properties.get(&arch)
    {
        for source in &arch_props.sources {
            let source_str = source.to_string();
            if let Some(source_file) = extract_http_source(&source_str) {
                sources.push(source_file);
            }
        }
    }

    debug!("Parsed {} HTTP/HTTPS sources from .SRCINFO", sources.len());
    Ok(sources)
}

/// Extract HTTP/HTTPS source from a source URL string
///
/// Handles PKGBUILD rename syntax: `newname::https://url/oldname.tar.gz`
/// Returns None for local files, git repos, or other non-downloadable sources.
fn extract_http_source(source_url: &str) -> Option<SourceFile> {
    // Handle PKGBUILD rename syntax: "newname::url"
    let (custom_filename, url) = if let Some(idx) = source_url.find("::") {
        let name = &source_url[..idx];
        let url = &source_url[idx + 2..];
        (Some(name.to_string()), url)
    } else {
        (None, source_url)
    };

    // Skip non-HTTP sources
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return None;
    }

    // Use custom filename if provided, otherwise extract from URL
    let filename = custom_filename.unwrap_or_else(|| {
        url.rsplit('/')
            .next()
            .unwrap_or("unknown")
            .split('?')
            .next()
            .unwrap_or("unknown")
            .split('#')
            .next()
            .unwrap_or("unknown")
            .to_string()
    });

    Some(SourceFile {
        url: url.to_string(),
        filename,
    })
}

/// Result of a best-effort AUR source pre-download.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceDownloadSummary {
    pub succeeded: usize,
    pub failed: usize,
}

/// Download sources concurrently (up to 8 at a time)
///
/// Downloads are skipped if a regular file already exists in SRCDEST.
/// Failures are counted and logged; makepkg still retries on build.
pub async fn download_sources(sources: Vec<SourceFile>, srcdest: &Path) -> SourceDownloadSummary {
    if sources.is_empty() {
        return SourceDownloadSummary {
            succeeded: 0,
            failed: 0,
        };
    }

    // Ensure SRCDEST exists
    if let Err(e) = tokio::fs::create_dir_all(srcdest).await {
        warn!("Failed to create SRCDEST directory: {e}");
        return SourceDownloadSummary {
            succeeded: 0,
            failed: sources.len(),
        };
    }

    let multi = MultiProgress::new();
    let style = ProgressStyle::with_template(
        "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} {msg}",
    )
    .expect("valid progress template")
    .progress_chars("#>-");

    let download_futures = sources.into_iter().map(|source| {
        // Filename captured for the security check inside the async block.
        let filename = source.filename.clone();
        let dest_path = srcdest.join(&source.filename);
        let pb = multi.add(ProgressBar::new(0));
        pb.set_style(style.clone());
        pb.set_message(source.filename.clone());

        async move {
            // SECURITY (audit ADV-23-01): the filename may come from a
            // hostile PKGBUILD's `name::url` rename syntax. Reject anything
            // that is not a plain filename — separators, parent components,
            // absolute paths — so downloads can never escape SRCDEST.
            if filename.is_empty()
                || filename.contains('/')
                || filename.contains('\\')
                || Path::new(&filename).is_absolute()
                || filename.split('/').any(|part| part == "..")
            {
                warn!("Rejecting unsafe source filename from PKGBUILD: {filename:?}");
                return Err(anyhow::anyhow!("unsafe source filename: {filename:?}"));
            }
            match tokio::fs::symlink_metadata(&dest_path).await {
                Ok(metadata) if metadata.is_file() => {
                    pb.finish_with_message(format!("{} (cached)", source.filename));
                    return Ok(());
                }
                Ok(_) => {
                    return Err(anyhow::anyhow!(
                        "SRCDEST path is not a regular file: {}",
                        dest_path.display()
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("Failed to inspect SRCDEST path {}", dest_path.display())
                    });
                }
            }

            download_file(&source.url, &dest_path, pb).await
        }
    });

    // Download up to 8 files concurrently (network I/O bound, safe to parallelize)
    let results: Vec<Result<()>> = stream::iter(download_futures)
        .buffer_unordered(8)
        .collect()
        .await;

    let mut succeeded = 0;
    let mut failed = 0;
    for result in results {
        match result {
            Ok(()) => succeeded += 1,
            Err(error) => {
                failed += 1;
                warn!("Failed to pre-download AUR source: {error}");
            }
        }
    }
    SourceDownloadSummary { succeeded, failed }
}

/// Download a single file with progress tracking
async fn download_file(url: &str, dest_path: &Path, pb: ProgressBar) -> Result<()> {
    let client = shared_client();
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch {url}"))?;

    if !response.status().is_success() {
        pb.finish_with_message(format!("Failed: HTTP {}", response.status()));
        return Err(anyhow::anyhow!("HTTP error: {}", response.status()));
    }

    // Get content length for progress bar
    let expected_length = response.content_length();
    if let Some(total) = expected_length {
        pb.set_length(total);
    }

    let parent = dest_path.parent().unwrap_or_else(|| Path::new("."));
    let temporary = tempfile::Builder::new()
        .prefix(".src-")
        .tempfile_in(parent)
        .with_context(|| {
            format!(
                "Failed to create temporary AUR source in {}",
                parent.display()
            )
        })?;
    let (std_file, temporary_path) = temporary.into_parts();

    download_to_file(
        std_file,
        temporary_path,
        dest_path,
        &pb,
        expected_length,
        response,
    )
    .await
}

/// Stream the response into a same-directory temp file, then persist it.
async fn download_to_file(
    std_file: std::fs::File,
    temporary_path: tempfile::TempPath,
    dest_path: &Path,
    pb: &ProgressBar,
    expected_length: Option<u64>,
    response: reqwest::Response,
) -> Result<()> {
    let mut file = File::from_std(std_file);

    // Stream download with progress updates
    let mut downloaded = 0u64;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read download chunk")?;

        tokio::io::copy(&mut chunk.as_ref(), &mut file)
            .await
            .context("Failed to write chunk to file")?;

        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }

    file.flush().await.context("Failed to flush file")?;
    file.sync_all()
        .await
        .context("Failed to sync AUR source download")?;
    drop(file);

    // Validate download size if Content-Length was provided
    if let Some(expected) = expected_length
        && downloaded != expected
    {
        return Err(anyhow::anyhow!(
            "Download incomplete: got {downloaded} bytes, expected {expected}"
        ));
    }

    temporary_path
        .persist(dest_path)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to persist AUR source at {}", dest_path.display()))?;

    pb.finish_with_message(format!(
        "{} (done)",
        dest_path.file_name().map_or_else(
            || dest_path.display().to_string(),
            |f| f.to_string_lossy().into_owned()
        )
    ));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_http_source_simple() {
        let result = extract_http_source("https://example.com/file.tar.gz");
        assert!(result.is_some());
        let source = result.unwrap();
        assert_eq!(source.url, "https://example.com/file.tar.gz");
        assert_eq!(source.filename, "file.tar.gz");
    }

    #[test]
    fn test_extract_http_source_with_query_string() {
        let result = extract_http_source("https://example.com/file.tar.gz?token=abc");
        assert!(result.is_some());
        let source = result.unwrap();
        assert_eq!(source.filename, "file.tar.gz");
    }

    #[test]
    fn test_extract_http_source_with_fragment() {
        let result = extract_http_source("https://example.com/file.tar.gz#hash");
        assert!(result.is_some());
        let source = result.unwrap();
        assert_eq!(source.filename, "file.tar.gz");
    }

    #[test]
    fn test_extract_http_source_with_rename() {
        let result = extract_http_source("custom-name.tar.gz::https://example.com/original.tar.gz");
        assert!(result.is_some());
        let source = result.unwrap();
        assert_eq!(source.url, "https://example.com/original.tar.gz");
        assert_eq!(source.filename, "custom-name.tar.gz");
    }

    #[test]
    fn test_extract_http_source_git_ignored() {
        let result = extract_http_source("git+https://github.com/user/repo.git");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_http_source_local_ignored() {
        let result = extract_http_source("local-file.patch");
        assert!(result.is_none());
    }
}
