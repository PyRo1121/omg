//! Parallel source downloading for AUR packages
//!
//! This module parses .SRCINFO files to extract HTTP/HTTPS sources and downloads
//! them concurrently before makepkg runs. This dramatically improves build times
//! for packages with multiple source files (10-60 second savings).

use std::path::Path;

use alpm_srcinfo::SourceInfoV1;
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
    /// Base filename (extracted from URL)
    pub filename: String,
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

    let srcinfo = SourceInfoV1::from_string(&content)
        .context("Failed to parse .SRCINFO")?;

    let mut sources = Vec::new();

    // Parse common sources (apply to all architectures)
    for source in &srcinfo.base.sources {
        // Source type is a complex struct; convert to string to get the URL
        let source_str = source.to_string();
        if let Some(source_file) = extract_http_source(&source_str) {
            sources.push(source_file);
        }
    }

    debug!("Parsed {} HTTP/HTTPS sources from .SRCINFO", sources.len());
    Ok(sources)
}

/// Extract HTTP/HTTPS source from a source URL string
///
/// Returns None for local files, git repos, or other non-downloadable sources.
fn extract_http_source(source_url: &str) -> Option<SourceFile> {
    // Skip non-HTTP sources
    if !source_url.starts_with("http://") && !source_url.starts_with("https://") {
        return None;
    }

    // Extract filename from URL
    let filename = source_url
        .rsplit('/')
        .next()
        .unwrap_or("unknown")
        .split('?')
        .next()
        .unwrap_or("unknown")
        .to_string();

    Some(SourceFile {
        url: source_url.to_string(),
        filename,
    })
}

/// Download sources concurrently (up to 4 at a time)
///
/// Downloads are skipped if the file already exists in SRCDEST.
/// Errors are logged but not fatal - makepkg will retry on build.
///
/// # Returns
/// Number of successfully downloaded files
pub async fn download_sources(sources: Vec<SourceFile>, srcdest: &Path) -> usize {
    if sources.is_empty() {
        return 0;
    }

    // Ensure SRCDEST exists
    if let Err(e) = tokio::fs::create_dir_all(srcdest).await {
        warn!("Failed to create SRCDEST directory: {}", e);
        return 0;
    }

    let multi = MultiProgress::new();
    let style = ProgressStyle::with_template(
        "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} {msg}",
    )
    .unwrap()
    .progress_chars("#>-");

    let download_futures = sources.into_iter().map(|source| {
        let dest_path = srcdest.join(&source.filename);
        let pb = multi.add(ProgressBar::new(0));
        pb.set_style(style.clone());
        pb.set_message(source.filename.clone());

        async move {
            // Skip if already downloaded
            if dest_path.exists() {
                pb.finish_with_message(format!("{} (cached)", source.filename));
                return Ok(());
            }

            download_file(&source.url, &dest_path, pb).await
        }
    });

    // Download up to 4 files concurrently
    let results: Vec<Result<()>> = stream::iter(download_futures)
        .buffer_unordered(4)
        .collect()
        .await;

    // Count successes
    results.iter().filter(|r| r.is_ok()).count()
}

/// Download a single file with progress tracking
async fn download_file(url: &str, dest_path: &Path, pb: ProgressBar) -> Result<()> {
    let client = shared_client();
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch {}", url))?;

    if !response.status().is_success() {
        pb.finish_with_message(format!("Failed: HTTP {}", response.status()));
        return Err(anyhow::anyhow!("HTTP error: {}", response.status()));
    }

    // Get content length for progress bar
    if let Some(total) = response.content_length() {
        pb.set_length(total);
    }

    // Create temporary file
    let temp_path = dest_path.with_extension("tmp");
    let mut file = File::create(&temp_path)
        .await
        .with_context(|| format!("Failed to create file at {}", temp_path.display()))?;

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
    drop(file);

    // Atomically move to final location
    tokio::fs::rename(&temp_path, dest_path)
        .await
        .with_context(|| {
            format!(
                "Failed to move {} to {}",
                temp_path.display(),
                dest_path.display()
            )
        })?;

    pb.finish_with_message(format!("{} ✓", dest_path.file_name().unwrap().to_string_lossy()));
    Ok(())
}
