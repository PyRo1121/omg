//! Parallel source downloading for AUR packages
//!
//! Parses `.SRCINFO` and downloads HTTP sources concurrently before makepkg runs.

use std::collections::{HashMap, HashSet};
use std::path::Path;

const MAX_AUR_SOURCE_BYTES: u64 = 1024 * 1024 * 1024;

use alpm_srcinfo::SourceInfoV1;
use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{debug, warn};

use crate::cli::progress::{Accent, Outcome, ProgressTask, TaskKind, TaskSpec};

/// Represents a source file that can be downloaded
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Full URL to download from
    pub url: String,
    /// Base filename (may be renamed via :: syntax)
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
    if let Some(arch) = super::aur::utils::current_arch()
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
    // Handle PKGBUILD rename syntax: "newname::url".
    // https://man.archlinux.org/PKGBUILD.5#sources_and_checksums
    let (custom_filename, url) = match source_url.rsplit_once("::") {
        Some((name, url)) => (Some(name.to_string()), url),
        None => (None, source_url),
    };

    // Plain HTTP sources are rejected: checksums are verified at build
    // time, but transport must still be authenticated (defense in depth).
    if !url.starts_with("https://") {
        return None;
    }

    // Use custom filename if provided, otherwise derive it from the URL by
    // stripping any query string / fragment. `split` always yields at least
    // one element, so a trailing-slash URL simply produces an empty name
    // (rejected later as unsafe) rather than a fabricated fallback.
    let filename = custom_filename.unwrap_or_else(|| {
        let base = url.split('?').next().unwrap_or_default();
        let base = base.split('#').next().unwrap_or_default();
        // rsplit on '/' always yields at least one element
        base.rsplit('/').next().unwrap_or_default().to_string()
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

fn validate_unique_destinations(sources: &[SourceFile]) -> Result<()> {
    let mut urls_by_filename = HashMap::with_capacity(sources.len());
    for source in sources {
        if let Some(previous_url) = urls_by_filename.insert(&source.filename, &source.url) {
            anyhow::ensure!(
                previous_url == &source.url,
                "AUR sources map filename {:?} to multiple URLs",
                source.filename
            );
        }
    }
    Ok(())
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
    if let Err(error) = validate_unique_destinations(&sources) {
        warn!("Skipping ambiguous AUR source pre-download: {error}");
        return SourceDownloadSummary {
            succeeded: 0,
            failed: sources.len(),
        };
    }
    let mut seen = HashSet::with_capacity(sources.len());
    let sources: Vec<SourceFile> = sources
        .into_iter()
        .filter(|source| seen.insert(source.filename.clone()))
        .collect();

    // Ensure SRCDEST exists
    if let Err(e) = tokio::fs::create_dir_all(srcdest).await {
        warn!("Failed to create SRCDEST directory: {e}");
        return SourceDownloadSummary {
            succeeded: 0,
            failed: sources.len(),
        };
    }

    let download_futures = sources.into_iter().map(|source| {
        // Filename captured for the security check inside the async block.
        let filename = source.filename.clone();
        let dest_path = srcdest.join(&source.filename);

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

            let task = ProgressTask::start(&TaskSpec {
                label: source.filename.clone(),
                kind: TaskKind::Bytes { total: None },
                accent: Accent::Network,
            });

            download_file(&source.url, &dest_path, task).await
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

/// Resolve and pin public addresses on every hop. Disable ambient proxies and
/// automatic redirects so neither DNS rebinding nor a redirect reaches the LAN.
fn public_source_address(address: std::net::IpAddr) -> bool {
    match address {
        std::net::IpAddr::V4(ip) => {
            let [a, b, _, _] = ip.octets();
            !ip.is_private()
                && !ip.is_loopback()
                && !ip.is_link_local()
                && !ip.is_unspecified()
                && !ip.is_documentation()
                && !ip.is_broadcast()
                && a != 0
                && a < 224
                && !(a == 100 && (64..=127).contains(&b))
                && !(a == 192 && b == 0)
                && !(a == 198 && (18..=19).contains(&b))
        }
        std::net::IpAddr::V6(ip) => {
            let segments = ip.segments();
            segments[0] & 0xe000 == 0x2000
                && !(segments[0] == 0x2001 && segments[1] == 0x0db8)
                && !(segments[0] == 0x2001 && segments[1] == 0)
                && segments[0] != 0x2002
        }
    }
}

async fn fetch_public_source(value: &str) -> Result<reqwest::Response> {
    let mut url = reqwest::Url::parse(value)?;
    for _ in 0..=10 {
        anyhow::ensure!(
            url.scheme() == "https" && url.username().is_empty() && url.password().is_none(),
            "AUR sources require HTTPS without URL credentials"
        );
        let host = url.host_str().context("Source URL has no host")?.to_owned();
        let port = url
            .port_or_known_default()
            .context("Source URL has no port")?;
        let addresses: Vec<_> = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            tokio::net::lookup_host((host.as_str(), port)),
        )
        .await??
        .collect();
        anyhow::ensure!(
            !addresses.is_empty()
                && addresses
                    .iter()
                    .all(|address| public_source_address(address.ip())),
            "AUR source resolves to a non-public address"
        );
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_mins(5))
            .resolve_to_addrs(&host, &addresses)
            .build()?;
        let response = client.get(url.clone()).send().await?;
        if !response.status().is_redirection() {
            return Ok(response);
        }
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .context("Source redirect has no location")?
            .to_str()?;
        url = url.join(location)?;
    }
    anyhow::bail!("Too many AUR source redirects")
}

/// Download a single file with progress tracking
async fn download_file(url: &str, dest_path: &Path, task: ProgressTask) -> Result<()> {
    let response = fetch_public_source(url).await?;

    if !response.status().is_success() {
        task.set_message(&format!("HTTP {}", response.status()));
        task.finish(Outcome::Failed);
        return Err(anyhow::anyhow!("HTTP error: {}", response.status()));
    }

    // Get content length for progress bar and reject an oversized response
    // before creating a large temporary file.
    let expected_length = response.content_length();
    if let Some(total) = expected_length {
        anyhow::ensure!(
            total <= MAX_AUR_SOURCE_BYTES,
            "AUR source declares {total} bytes, exceeding the {MAX_AUR_SOURCE_BYTES}-byte limit"
        );
        task.set_total(Some(total));
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
        &task,
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
    task: &ProgressTask,
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

        downloaded = downloaded
            .checked_add(u64::try_from(chunk.len()).context("AUR source chunk is too large")?)
            .context("AUR source byte count overflowed")?;
        anyhow::ensure!(
            downloaded <= MAX_AUR_SOURCE_BYTES,
            "AUR source exceeded the {MAX_AUR_SOURCE_BYTES}-byte limit"
        );
        task.set_position(downloaded);
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
        task.finish(Outcome::Failed);
        return Err(anyhow::anyhow!(
            "Download incomplete: got {downloaded} bytes, expected {expected}"
        ));
    }

    temporary_path
        .persist(dest_path)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to persist AUR source at {}", dest_path.display()))?;

    task.finish(Outcome::Done);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflicting_source_destinations_are_rejected() {
        let sources = vec![
            SourceFile {
                url: "https://example.com/base.tar.gz".to_string(),
                filename: "source.tar.gz".to_string(),
            },
            SourceFile {
                url: "https://arch.example.com/arch.tar.gz".to_string(),
                filename: "source.tar.gz".to_string(),
            },
        ];

        let error = validate_unique_destinations(&sources)
            .expect_err("different URLs must not race for one destination");
        assert!(error.to_string().contains("multiple URLs"), "{error}");
    }

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

#[cfg(test)]
mod public_source_tests {
    use super::*;
    #[test]
    fn private_and_transition_destinations_are_rejected() {
        for value in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "0.0.0.0",
            "224.0.0.1",
            "::1",
            "fd00::1",
            "fe80::1",
            "::ffff:127.0.0.1",
            "2002:7f00:1::",
            "2001:db8::1",
        ] {
            assert!(!public_source_address(value.parse().unwrap()), "{value}");
        }
        for value in ["1.1.1.1", "2606:4700:4700::1111"] {
            assert!(public_source_address(value.parse().unwrap()), "{value}");
        }
    }
    #[tokio::test]
    async fn prefetch_refuses_loopback_before_connecting() {
        assert!(
            fetch_public_source("https://127.0.0.1/source")
                .await
                .unwrap_err()
                .to_string()
                .contains("non-public")
        );
        assert!(
            fetch_public_source("http://example.org/source")
                .await
                .is_err()
        );
    }
}
