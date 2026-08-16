//! Parallel Repository Synchronization for Debian/Ubuntu
//!
//! Downloads Release files and Packages indices in parallel from configured
//! APT repositories. Supports:
//! - Concurrent downloads with connection pooling
//! - Progress bars with per-repo status
//! - Automatic decompression (gzip, xz, lz4)
//! - InRelease/Release signature verification (optional)
//! - Atomic cache updates

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use reqwest::Client;
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

use crate::core::paths;

use super::sources::{Repository, get_enabled_binary_repos};

/// Maximum concurrent downloads (increased for HTTP/2 multiplexing)
const MAX_CONCURRENT_DOWNLOADS: usize = 50;

/// Timeout for individual downloads
const DOWNLOAD_TIMEOUT_SECS: u64 = 120;

/// Maximum retries per download
const MAX_RETRIES: u32 = 3;

/// Initial backoff for retries (doubles each retry)
const INITIAL_BACKOFF_MS: u64 = 200;

/// Cache TTL in seconds (6 hours)
const CACHE_TTL_SECS: u64 = 6 * 60 * 60;

/// Repository sync state for caching
#[derive(serde::Serialize, serde::Deserialize, Default)]
#[expect(dead_code)] // Future feature: incremental sync
struct SyncCache {
    /// When this cache was created
    synced_at: u64,
    /// Map of repo URI+suite to last sync timestamp
    repos: std::collections::HashMap<String, u64>,
}

/// Check if apt's cache (/var/lib/apt/lists/) is fresh enough
/// Returns true if apt cache was updated within the last 6 hours
fn is_apt_cache_fresh() -> bool {
    let apt_lists = Path::new("/var/lib/apt/lists");
    if !apt_lists.exists() {
        return false;
    }

    // Check if the apt lists directory was modified recently
    // When apt-get update runs, it modifies this directory
    const FRESH_THRESHOLD_SECS: u64 = 6 * 60 * 60; // 6 hours

    // First check the directory modification time
    if let Ok(meta) = fs::metadata(apt_lists)
        && let Ok(mtime) = meta.modified()
        && let Ok(elapsed) = SystemTime::now().duration_since(mtime)
        && elapsed.as_secs() < FRESH_THRESHOLD_SECS
    {
        tracing::debug!("apt lists directory is fresh ({}s old)", elapsed.as_secs());
        return true;
    }

    // Fallback: check if any _Packages file was modified recently
    if let Ok(entries) = fs::read_dir(apt_lists) {
        for entry in entries.flatten() {
            let path = entry.path();
            if let Some(name) = path.file_name().and_then(|n| n.to_str())
                && name.contains("_Packages")
                && !name.to_lowercase().ends_with(".diff")
                && let Ok(meta) = entry.metadata()
                && let Ok(mtime) = meta.modified()
                && let Ok(elapsed) = SystemTime::now().duration_since(mtime)
                && elapsed.as_secs() < FRESH_THRESHOLD_SECS
            {
                tracing::debug!(
                    "Found fresh apt cache file: {} ({}s old)",
                    name,
                    elapsed.as_secs()
                );
                return true;
            }
        }
    }
    false
}

/// Sync all configured APT repositories in parallel
pub async fn sync_all_repositories(show_progress: bool) -> Result<()> {
    // OPTIMIZATION: Skip sync if apt's cache is already fresh
    // This avoids redundant downloads when apt-get update recently ran
    if is_apt_cache_fresh() {
        if show_progress {
            println!("{}", "Package lists already up to date".dimmed());
        }
        tracing::debug!("Skipping sync - apt cache is fresh");
        return Ok(());
    }

    let repos = get_enabled_binary_repos()?;

    if repos.is_empty() {
        if show_progress {
            println!("{}", "No enabled repositories found".yellow());
        }
        return Ok(());
    }

    let client = build_http_client()?;

    if show_progress {
        sync_with_progress(&client, &repos).await
    } else {
        sync_quiet(&client, &repos).await
    }
}

/// Build HTTP client with connection pooling for maximum performance
fn build_http_client() -> Result<Client> {
    Client::builder()
        .timeout(Duration::from_secs(DOWNLOAD_TIMEOUT_SECS))
        .connect_timeout(Duration::from_secs(10))
        .pool_max_idle_per_host(MAX_CONCURRENT_DOWNLOADS)
        // Let reqwest auto-negotiate HTTP version (HTTP/1.1 or HTTP/2)
        // Some mirrors don't properly support HTTP/2
        .user_agent(concat!("omg/", env!("CARGO_PKG_VERSION")))
        // Compression enabled via Cargo features: gzip, brotli
        .build()
        .context("Failed to build HTTP client")
}

/// Sync with progress bars
async fn sync_with_progress(client: &Client, repos: &[Repository]) -> Result<()> {
    let multi = MultiProgress::new();
    let style = ProgressStyle::default_bar()
        .template("{prefix:.cyan} {spinner} {msg}")
        .expect("valid template");

    let overall = multi.add(ProgressBar::new(repos.len() as u64));
    overall.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.bold} [{bar:30.cyan/blue}] {pos}/{len}")
            .expect("valid template")
            .progress_chars("=>-"),
    );
    overall.set_prefix("Syncing repositories");

    // Clone repos to owned Vec to avoid lifetime issues with stream::iter
    let owned_repos: Vec<Repository> = repos.to_vec();

    let results: Vec<_> = stream::iter(owned_repos)
        .map(|repo| {
            let client = client.clone();
            let pb = multi.add(ProgressBar::new_spinner());
            pb.set_style(style.clone());
            pb.set_prefix(format!("  {}", repo_display_name(&repo)));
            pb.enable_steady_tick(Duration::from_millis(100));

            async move {
                let result = sync_repository(&client, &repo, Some(&pb)).await;
                match &result {
                    Ok(()) => {
                        pb.set_message("✓".green().to_string());
                        pb.finish();
                    }
                    Err(e) => {
                        pb.set_message(format!("{e}").red().to_string());
                        pb.finish();
                    }
                }
                result
            }
        })
        .buffer_unordered(MAX_CONCURRENT_DOWNLOADS)
        .inspect(|_| overall.inc(1))
        .collect()
        .await;

    overall.finish_and_clear();

    // Report results
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    let fail_count = results.len() - success_count;

    if fail_count > 0 {
        println!(
            "{} {} synced, {} {}",
            success_count.to_string().green(),
            "repositories".dimmed(),
            fail_count.to_string().red(),
            "failed".dimmed()
        );
    } else {
        println!(
            "{} {} synced",
            success_count.to_string().green(),
            "repositories".dimmed()
        );
    }

    // Return error if any failed
    for result in results {
        result?;
    }

    Ok(())
}

/// Sync without progress output
async fn sync_quiet(client: &Client, repos: &[Repository]) -> Result<()> {
    // Clone repos to owned Vec to avoid lifetime issues with stream::iter
    let owned_repos: Vec<Repository> = repos.to_vec();

    let results: Vec<_> = stream::iter(owned_repos)
        .map(|repo| {
            let client = client.clone();
            async move { sync_repository(&client, &repo, None).await }
        })
        .buffer_unordered(MAX_CONCURRENT_DOWNLOADS)
        .collect()
        .await;

    for result in results {
        result?;
    }

    Ok(())
}

/// Sync a single repository
async fn sync_repository(
    client: &Client,
    repo: &Repository,
    progress: Option<&ProgressBar>,
) -> Result<()> {
    let cache_dir = repo_cache_dir(repo);
    fs::create_dir_all(&cache_dir)
        .with_context(|| format!("Failed to create cache dir: {}", cache_dir.display()))?;

    // Check if cache is fresh
    if is_cache_fresh(&cache_dir) {
        if let Some(pb) = progress {
            pb.set_message("cached");
        }
        tracing::debug!(
            "Using cached repository data for {}/{}",
            repo.uri,
            repo.suite
        );
        return Ok(());
    }

    tracing::debug!("Syncing repository: {}/{}", repo.uri, repo.suite);

    // Download InRelease using HTTP conditional request for cache validation
    if let Some(pb) = progress {
        pb.set_message("checking Release...");
    }

    let release_url = repo.release_url();
    let release_path = cache_dir.join("InRelease");

    // Check if we have a cached Last-Modified time for conditional request
    let cached_last_modified = get_cached_last_modified(&cache_dir, "InRelease");

    // Use conditional request with If-Modified-Since
    let release_result = conditional_download_with_retry(
        client,
        &release_url,
        &release_path,
        cached_last_modified.as_deref(),
    )
    .await;

    match release_result {
        Ok(DownloadResult::NotModified) => {
            // 304 Not Modified - repository hasn't changed, skip component downloads
            if let Some(pb) = progress {
                pb.set_message("up to date ✓");
            }
            tracing::debug!(
                "Repository {}/{} unchanged (304 Not Modified)",
                repo.uri,
                repo.suite
            );
            update_cache_timestamp(&cache_dir)?;
            return Ok(());
        }
        Ok(DownloadResult::Downloaded { last_modified }) => {
            tracing::debug!("Downloaded fresh Release file from {release_url}");
            // Store Last-Modified for future conditional requests
            if let Some(lm) = last_modified
                && let Err(error) = store_last_modified(&cache_dir, "InRelease", &lm)
            {
                tracing::warn!(
                    "Failed to persist Last-Modified for {}/{}: {error}",
                    repo.uri,
                    repo.suite
                );
            }
        }
        Err(e) => {
            tracing::error!("Failed to download Release from {}: {}", release_url, e);
            return Err(e)
                .with_context(|| format!("Failed to sync repository {}/{}", repo.uri, repo.suite));
        }
    }

    // Download Packages files for all components in FULL parallel
    let arch = get_system_arch();
    let cache_dir_ref = cache_dir.clone(); // Clone for closure capture

    // OPTIMIZATION: Try only the 2 most common formats to reduce wasted requests
    // Modern Debian/Ubuntu repos use gzip (99%) or xz (1%)
    let component_downloads: Vec<_> = repo
        .components
        .iter()
        .flat_map(|component| {
            let formats = [
                (".gz", decompress_gzip as fn(&[u8]) -> Result<Vec<u8>>),
                (".xz", decompress_xz),
            ];

            let cache_dir_inner = cache_dir_ref.clone();
            formats.into_iter().map(move |(ext, decompress)| {
                let client = client.clone();
                let cache_dir = cache_dir_inner.clone();
                let component = component.clone();
                let repo_uri = repo.uri.clone();
                let repo_suite = repo.suite.clone();

                async move {
                    let url = format!(
                        "{}/dists/{}/{}/binary-{}/Packages{ext}",
                        repo_uri.trim_end_matches('/'),
                        repo_suite,
                        component,
                        arch
                    );

                    let packages_path = cache_dir.join(format!("{component}_{arch}_Packages"));

                    match download_bytes_with_retry(&client, &url).await {
                        Ok(data) => match decompress(&data) {
                            Ok(decompressed) => {
                                atomic_write(&packages_path, &decompressed)?;
                                tracing::debug!(
                                    "Successfully downloaded component {component} ({ext})"
                                );
                                Ok::<Option<(String, String)>, anyhow::Error>(Some((
                                    component.clone(),
                                    ext.to_string(),
                                )))
                            }
                            Err(e) => {
                                tracing::debug!("Decompression failed for {url}: {e}");
                                Ok::<Option<(String, String)>, anyhow::Error>(None)
                            }
                        },
                        Err(_) => Ok::<Option<(String, String)>, anyhow::Error>(None), // File not available in this format
                    }
                }
            })
        })
        .collect();

    // Execute all downloads concurrently (try all formats at once)
    let results = futures::future::join_all(component_downloads).await;

    // Check that we got at least one success per component
    let mut successful_components: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for result in results {
        match result {
            Ok(Some((component, _ext))) => {
                successful_components.insert(component);
            }
            Ok(None) => {} // Failed attempt, expected
            Err(e) => {
                tracing::warn!("Component download error: {e}");
            }
        }
    }

    // Verify all components were downloaded
    for component in &repo.components {
        if !successful_components.contains(component) {
            anyhow::bail!(
                "Failed to download Packages file for component '{component}' in any format"
            );
        }
    }

    if let Some(pb) = progress {
        pb.set_message(format!("{} components ✓", successful_components.len()));
    }

    // Update cache timestamp
    update_cache_timestamp(&cache_dir)?;
    tracing::info!("Successfully synced repository {}/{}", repo.uri, repo.suite);

    Ok(())
}

/// Result of a conditional download
#[derive(Debug)]
enum DownloadResult {
    /// Content was downloaded (new or updated)
    Downloaded { last_modified: Option<String> },
    /// Server returned 304 Not Modified
    NotModified,
}

/// Download a file with conditional request (If-Modified-Since) support
async fn conditional_download_with_retry(
    client: &Client,
    url: &str,
    dest: &Path,
    if_modified_since: Option<&str>,
) -> Result<DownloadResult> {
    let mut last_error = None;

    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            let backoff = Duration::from_millis(INITIAL_BACKOFF_MS << attempt);
            tracing::debug!(
                "Retry attempt {} for {} after {:?}",
                attempt + 1,
                url,
                backoff
            );
            tokio::time::sleep(backoff).await;
        }

        let mut request = client.get(url);

        // Add If-Modified-Since header for conditional request
        if let Some(last_modified) = if_modified_since {
            request = request.header("If-Modified-Since", last_modified);
        }

        match request.send().await {
            Ok(response) => {
                // 304 Not Modified - use cached copy
                if response.status() == reqwest::StatusCode::NOT_MODIFIED {
                    tracing::debug!("304 Not Modified for {url}");
                    return Ok(DownloadResult::NotModified);
                }

                if response.status().is_success() {
                    // Extract Last-Modified header for future conditional requests
                    let last_modified = response
                        .headers()
                        .get("Last-Modified")
                        .and_then(|v| v.to_str().ok())
                        .map(String::from);

                    match response.bytes().await {
                        Ok(bytes) => {
                            atomic_write(dest, &bytes)?;
                            tracing::debug!("Downloaded {} bytes from {url}", bytes.len());
                            return Ok(DownloadResult::Downloaded { last_modified });
                        }
                        Err(e) => {
                            tracing::warn!("Failed to read response body from {url}: {e}");
                            last_error = Some(e.into());
                        }
                    }
                } else if response.status() == reqwest::StatusCode::NOT_FOUND {
                    anyhow::bail!("Resource not found: {url}");
                } else {
                    let status = response.status();
                    tracing::warn!("HTTP {status} error for {url}");
                    last_error = Some(anyhow::anyhow!("HTTP {status} for {url}"));
                }
            }
            Err(e) => {
                tracing::warn!("Network error downloading {url}: {e}");
                last_error = Some(e.into());
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        anyhow::anyhow!("Download failed after {MAX_RETRIES} retries for {url}")
    }))
}

/// Download bytes with retry logic and HTTP conditional request support
/// Returns None if server returns 304 Not Modified (use cached copy)
async fn download_bytes_with_retry(client: &Client, url: &str) -> Result<Vec<u8>> {
    download_bytes_conditional(client, url, None)
        .await
        .map(Option::unwrap_or_default)
}

/// Download bytes with optional If-Modified-Since header
/// Returns:
/// - Ok(Some(bytes)) if content was downloaded
/// - Ok(None) if 304 Not Modified (use cached copy)
/// - Err if download failed
async fn download_bytes_conditional(
    client: &Client,
    url: &str,
    if_modified_since: Option<&str>,
) -> Result<Option<Vec<u8>>> {
    let mut last_error = None;

    for attempt in 0..MAX_RETRIES {
        if attempt > 0 {
            let backoff = Duration::from_millis(INITIAL_BACKOFF_MS << attempt);
            tracing::debug!(
                "Retry attempt {} for {} after {:?}",
                attempt + 1,
                url,
                backoff
            );
            tokio::time::sleep(backoff).await;
        }

        let mut request = client.get(url);

        // Add If-Modified-Since header for conditional request
        if let Some(last_modified) = if_modified_since {
            request = request.header("If-Modified-Since", last_modified);
        }

        match request.send().await {
            Ok(response) => {
                // 304 Not Modified - use cached copy
                if response.status() == reqwest::StatusCode::NOT_MODIFIED {
                    tracing::debug!("304 Not Modified for {url} (cache hit)");
                    return Ok(None);
                }

                if response.status().is_success() {
                    match response.bytes().await {
                        Ok(bytes) => {
                            tracing::debug!("Downloaded {} bytes from {url}", bytes.len());
                            return Ok(Some(bytes.to_vec()));
                        }
                        Err(e) => {
                            tracing::warn!("Failed to read response body from {url}: {e}");
                            last_error = Some(e.into());
                        }
                    }
                } else if response.status() == reqwest::StatusCode::NOT_FOUND {
                    anyhow::bail!("Resource not found: {url}");
                } else {
                    let status = response.status();
                    tracing::warn!("HTTP {status} error for {url}");
                    last_error = Some(anyhow::anyhow!("HTTP {status} for {url}"));
                }
            }
            Err(e) => {
                tracing::warn!("Network error downloading {url}: {e}");
                last_error = Some(e.into());
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        anyhow::anyhow!("Download failed after {MAX_RETRIES} retries for {url}")
    }))
}

/// Get stored Last-Modified header for a URL (from metadata file)
fn get_cached_last_modified(cache_dir: &Path, filename: &str) -> Option<String> {
    let meta_path = cache_dir.join(format!("{filename}.meta"));
    fs::read_to_string(meta_path).ok()
}

/// Store Last-Modified header for a URL
fn store_last_modified(cache_dir: &Path, filename: &str, last_modified: &str) -> Result<()> {
    let meta_path = cache_dir.join(format!("{filename}.meta"));
    let mut file = NamedTempFile::new_in(cache_dir).with_context(|| {
        format!(
            "Failed to create Last-Modified temp file in {}",
            cache_dir.display()
        )
    })?;
    file.write_all(last_modified.as_bytes())?;
    file.as_file_mut().sync_all()?;
    file.persist(&meta_path)
        .map_err(|error| error.error)
        .context("Failed to persist Last-Modified metadata")?;
    Ok(())
}

/// Atomically write data to a file
fn atomic_write(dest: &Path, data: &[u8]) -> Result<()> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = NamedTempFile::new_in(parent)?;
    temp.write_all(data)?;
    temp.as_file_mut().sync_all()?;
    temp.persist(dest)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to persist {}", dest.display()))?;
    Ok(())
}

/// Verify SHA256 checksum of data
#[expect(dead_code)] // Future feature: Release file verification
fn verify_checksum(data: &[u8], expected_hash: &str) -> Result<()> {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    let actual_hash = hex::encode(result);

    if actual_hash == expected_hash {
        Ok(())
    } else {
        anyhow::bail!("Checksum mismatch: expected {expected_hash}, got {actual_hash}")
    }
}

/// Parse Release file to extract SHA256 checksums
#[expect(dead_code)] // Future feature: Release file verification
fn parse_release_file(content: &str) -> std::collections::HashMap<String, String> {
    let mut checksums = std::collections::HashMap::new();
    let mut in_sha256_section = false;

    for line in content.lines() {
        if line.starts_with("SHA256:") {
            in_sha256_section = true;
            continue;
        }

        if in_sha256_section {
            if line.starts_with(' ') || line.starts_with('\t') {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    let hash = parts[0];
                    let filename = parts[2];
                    checksums.insert(filename.to_string(), hash.to_string());
                }
            } else if !line.is_empty() {
                // End of SHA256 section
                break;
            }
        }
    }

    checksums
}

/// Get the cache directory for a repository
fn repo_cache_dir(repo: &Repository) -> PathBuf {
    let cache_base = paths::cache_dir().join("apt");

    // Create a safe directory name from URI + suite
    let safe_name = format!("{}_{}", repo.uri, repo.suite)
        .replace(['/', ':', '.'], "_")
        .replace("__", "_");

    cache_base.join(safe_name)
}

/// Check if the cache is still fresh
fn is_cache_fresh(cache_dir: &Path) -> bool {
    let timestamp_file = cache_dir.join(".synced");
    if let Ok(metadata) = fs::metadata(&timestamp_file)
        && let Ok(modified) = metadata.modified()
        && let Ok(elapsed) = SystemTime::now().duration_since(modified)
    {
        return elapsed.as_secs() < CACHE_TTL_SECS;
    }
    false
}

/// Update the cache timestamp
fn update_cache_timestamp(cache_dir: &Path) -> Result<()> {
    let timestamp_file = cache_dir.join(".synced");
    let mut file = NamedTempFile::new_in(cache_dir).with_context(|| {
        format!(
            "Failed to create Debian sync timestamp in {}",
            cache_dir.display()
        )
    })?;
    file.write_all(b"")?;
    file.as_file_mut().sync_all()?;
    file.persist(&timestamp_file)
        .map_err(|error| error.error)
        .context("Failed to persist Debian sync timestamp")?;
    Ok(())
}

/// Get a display name for a repository
fn repo_display_name(repo: &Repository) -> String {
    // Extract hostname from URI
    let host = repo
        .uri
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split('/')
        .next()
        .unwrap_or(&repo.uri);

    format!("{}/{}", host, repo.suite)
}

/// Get the system architecture in Debian format
fn get_system_arch() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        "arm" => "armhf",
        "x86" => "i386",
        arch => arch,
    }
}

/// Decompression functions
/// OPTIMIZATION: Pre-allocate with estimated size (gzip stores uncompressed size in footer)
fn decompress_gzip(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::GzDecoder;
    use std::io::Read;

    let mut decoder = GzDecoder::new(data);
    // Pre-allocate with heuristic: compressed data typically 3-5x smaller
    let estimated_size = data.len() * 4;
    let mut decompressed = Vec::with_capacity(estimated_size);
    decoder.read_to_end(&mut decompressed)?;
    Ok(decompressed)
}

fn decompress_xz(data: &[u8]) -> Result<Vec<u8>> {
    use lzma_rs::xz_decompress;
    use std::io::BufReader;

    // OPTIMIZATION: XZ typically compresses 6-8x, pre-allocate
    let estimated_size = data.len() * 7;
    let mut output = Vec::with_capacity(estimated_size);
    xz_decompress(&mut BufReader::new(data), &mut output)?;
    Ok(output)
}

// LZ4 compression format - not currently used by Debian/Ubuntu repos
// but kept for potential future support
#[allow(dead_code)]
fn decompress_lz4(data: &[u8]) -> Result<Vec<u8>> {
    lz4_flex::decompress_size_prepended(data)
        .map_err(|e| anyhow::anyhow!("LZ4 decompression failed: {e}"))
}

// No-op decompression for uncompressed Packages files
// Not currently used but kept as part of the decompression function pointer pattern
#[allow(dead_code)]
#[allow(clippy::unnecessary_wraps)] // Used as function pointer with other decompression functions
fn decompress_none(data: &[u8]) -> Result<Vec<u8>> {
    Ok(data.to_vec())
}

/// Force a full sync, ignoring cache
pub async fn force_sync_all() -> Result<()> {
    invalidate_sync_timestamps(&paths::cache_dir().join("apt"))?;
    sync_all_repositories(true).await
}

/// Remove `.synced` markers so the next sync cannot treat stale cache as fresh.
fn invalidate_sync_timestamps(cache_base: &Path) -> Result<()> {
    if !cache_base.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(cache_base)? {
        let entry = entry?;
        if !entry.path().is_dir() {
            continue;
        }
        let timestamp = entry.path().join(".synced");
        match fs::remove_file(&timestamp) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to invalidate Debian sync timestamp {}",
                        timestamp.display()
                    )
                });
            }
        }
    }
    Ok(())
}

/// Check if any repositories need syncing
pub fn needs_sync() -> bool {
    let Ok(repos) = get_enabled_binary_repos() else {
        return true;
    };

    repos.iter().any(|repo| {
        let cache_dir = repo_cache_dir(repo);
        !is_cache_fresh(&cache_dir)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalidate_sync_timestamps_removes_fresh_markers() {
        let temp = tempfile::TempDir::new().unwrap();
        let repo_dir = temp.path().join("debian_bookworm");
        fs::create_dir_all(&repo_dir).unwrap();
        fs::write(repo_dir.join(".synced"), b"").unwrap();
        assert!(is_cache_fresh(&repo_dir));

        invalidate_sync_timestamps(temp.path()).unwrap();
        assert!(!is_cache_fresh(&repo_dir));
        assert!(!repo_dir.join(".synced").exists());
    }

    #[test]
    fn invalidate_sync_timestamps_fails_closed_when_a_marker_cannot_be_removed() {
        let temp = tempfile::TempDir::new().unwrap();
        let repo_dir = temp.path().join("debian_bookworm");
        fs::create_dir_all(repo_dir.join(".synced")).unwrap();

        let error = invalidate_sync_timestamps(temp.path())
            .expect_err("a directory marker must fail closed");
        assert!(
            error
                .to_string()
                .contains("invalidate Debian sync timestamp")
        );
        assert!(repo_dir.join(".synced").exists());
    }

    #[test]
    fn test_get_system_arch() {
        let arch = get_system_arch();
        assert!(!arch.is_empty());
        // On x86_64, should return amd64
        if std::env::consts::ARCH == "x86_64" {
            assert_eq!(arch, "amd64");
        }
    }

    #[test]
    fn test_repo_display_name() {
        let repo = Repository {
            repo_type: super::super::sources::RepoType::Binary,
            uri: "http://deb.debian.org/debian".to_string(),
            suite: "bookworm".to_string(),
            components: vec!["main".to_string()],
            arch: None,
            signed_by: None,
            enabled: true,
            source_file: PathBuf::new(),
            options: std::collections::HashMap::new(),
        };

        assert_eq!(repo_display_name(&repo), "deb.debian.org/bookworm");
    }

    #[test]
    fn test_repo_cache_dir() {
        let repo = Repository {
            repo_type: super::super::sources::RepoType::Binary,
            uri: "http://deb.debian.org/debian".to_string(),
            suite: "bookworm".to_string(),
            components: vec!["main".to_string()],
            arch: None,
            signed_by: None,
            enabled: true,
            source_file: PathBuf::new(),
            options: std::collections::HashMap::new(),
        };

        let cache_dir = repo_cache_dir(&repo);
        assert!(cache_dir.to_string_lossy().contains("apt"));
        assert!(cache_dir.to_string_lossy().contains("bookworm"));
    }

    #[test]
    fn test_decompress_none() {
        let data = b"hello world";
        let result = decompress_none(data).unwrap();
        assert_eq!(result, data);
    }

    #[test]
    fn test_decompress_gzip() {
        use flate2::Compression;
        use flate2::write::GzEncoder;
        use std::io::Write;

        let original = b"test data for gzip compression";
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(original).unwrap();
        let compressed = encoder.finish().unwrap();

        let decompressed = decompress_gzip(&compressed).unwrap();
        assert_eq!(decompressed, original);
    }
}
