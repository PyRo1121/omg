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

    let sources = source_entries(&srcinfo)
        .iter()
        .filter_map(|source| extract_http_source(source))
        .collect::<Vec<_>>();

    debug!("Parsed {} HTTP/HTTPS sources from .SRCINFO", sources.len());
    Ok(sources)
}

/// Every declared source string, including architecture-specific entries.
///
/// Both the HTTP and the VCS prefetch paths work from this list so they can
/// never disagree about which sources a PKGBUILD declares.
fn source_entries(srcinfo: &SourceInfoV1) -> Vec<String> {
    let mut entries = srcinfo
        .base
        .sources
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if let Some(arch) = super::aur::utils::current_arch()
        && let Some(arch_props) = srcinfo.base.architecture_properties.get(&arch)
    {
        entries.extend(arch_props.sources.iter().map(ToString::to_string));
    }

    entries
}

/// VCS clients makepkg can delegate to. Mirrors makepkg's `get_protocol`.
const VCS_PROTOCOLS: [&str; 5] = ["git", "svn", "hg", "bzr", "fossil"];

/// A source that makepkg fetches with a VCS client rather than over plain HTTP.
///
/// The offline build sandbox (`bwrap --unshare-net`) makes these sources
/// unfetchable during the build, so they must be mirrored into SRCDEST first.
/// `filename` and `url` reproduce makepkg's own `get_filename`/`get_url`
/// helpers so a prefetched copy lands exactly where makepkg looks for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VcsSource {
    /// makepkg protocol name: `git`, `svn`, `hg`, `bzr`, or `fossil`.
    pub protocol: String,
    /// URL handed to the VCS client: the `proto+` prefix and any `?query` /
    /// `#fragment` removed, matching makepkg's `download_<proto>`.
    pub url: String,
    /// SRCDEST entry name, matching makepkg's `get_filename`.
    pub filename: String,
    /// The `.SRCINFO` entry as written, for diagnostics.
    pub raw: String,
}

/// Parse `.SRCINFO` for VCS sources (`git+`, `svn+`, `hg+`, `bzr+`, `fossil+`).
pub fn parse_vcs_sources(pkg_dir: &Path) -> Result<Vec<VcsSource>> {
    let srcinfo_path = pkg_dir.join(".SRCINFO");
    if !srcinfo_path.exists() {
        return Ok(Vec::new());
    }

    let content = std::fs::read_to_string(&srcinfo_path)
        .with_context(|| format!("Failed to read .SRCINFO at {}", srcinfo_path.display()))?;
    let srcinfo = SourceInfoV1::from_string(&content).context("Failed to parse .SRCINFO")?;

    let sources = source_entries(&srcinfo)
        .iter()
        .filter_map(|source| extract_vcs_source(source))
        .collect::<Vec<_>>();

    debug!("Parsed {} VCS sources from .SRCINFO", sources.len());
    Ok(sources)
}

/// Extract a VCS source from a `.SRCINFO` source entry.
///
/// Reproduces makepkg's `get_protocol`, `get_url` and `get_filename` so the
/// prefetched mirror is found by makepkg without any special casing.
fn extract_vcs_source(source_url: &str) -> Option<VcsSource> {
    // PKGBUILD rename syntax: `newname::url`
    // https://man.archlinux.org/PKGBUILD.5#sources_and_checksums
    let (custom_filename, netfile) = match source_url.split_once("::") {
        Some((name, rest)) => (Some(name), rest),
        None => (None, source_url),
    };

    // makepkg's get_protocol: text before "://", then before any '+'.
    let (before_scheme, _) = netfile.split_once("://")?;
    let protocol = before_scheme.split('+').next().unwrap_or(before_scheme);
    if !VCS_PROTOCOLS.contains(&protocol) {
        return None;
    }

    let filename = custom_filename.map_or_else(
        || makepkg_vcs_filename(netfile, protocol),
        ToString::to_string,
    );

    // makepkg's download_<proto> strips the `proto+` prefix, then the fragment,
    // then the query string, before handing the URL to the VCS client.
    let url = netfile
        .strip_prefix(&format!("{protocol}+"))
        .unwrap_or(netfile)
        .split('#')
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default()
        .to_string();

    Some(VcsSource {
        protocol: protocol.to_string(),
        url,
        filename,
        raw: source_url.to_string(),
    })
}

/// Reproduce makepkg's `get_filename` for a VCS source entry.
fn makepkg_vcs_filename(netfile: &str, protocol: &str) -> String {
    // ${filename%%#*} then ${filename%%\?*} then ${filename%/} then ${filename##*/}
    let without_fragment = netfile.split('#').next().unwrap_or_default();
    let without_query = without_fragment.split('?').next().unwrap_or_default();
    let base = without_query
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();

    match protocol {
        // ${filename%%.git*} — strips from the first `.git` occurrence.
        "git" => base
            .find(".git")
            .map_or_else(|| base.to_string(), |index| base[..index].to_string()),
        // ${filename#*lp:}
        "bzr" => base
            .split_once("lp:")
            .map_or_else(|| base.to_string(), |(_, rest)| rest.to_string()),
        // ${filename}.fossil
        "fossil" => format!("{base}.fossil"),
        _ => base.to_string(),
    }
}

/// Outcome of the VCS prefetch pass.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VcsPrefetchSummary {
    /// SRCDEST entries that were already populated.
    pub cached: Vec<String>,
    /// Sources newly mirrored by omg.
    pub prefetched: Vec<String>,
    /// Sources that still require network during the build because no cached
    /// copy exists. omg can only mirror `git`; other VCS clients (and git URLs
    /// on unauthenticated transports) stay listed here.
    pub needs_network: Vec<String>,
}

/// Reject SRCDEST entry names that are not a single plain path component.
fn validate_source_filename(filename: &str) -> Result<()> {
    anyhow::ensure!(
        !filename.is_empty()
            && filename != "."
            && filename != ".."
            && !filename.contains('/')
            && !filename.contains('\\')
            && !Path::new(filename).is_absolute(),
        "unsafe source filename: {filename:?}"
    );
    Ok(())
}

/// Mirror every VCS source into `srcdest` so the network-less sandbox can build.
///
/// makepkg's `extract_git` creates the working copy with a *local* `git clone`
/// from `$SRCDEST` whenever `--cleanbuild` cleared `$SRCDIR`, and its
/// `download_git` only warns when the mirror's `git fetch` fails offline. A
/// populated mirror is therefore sufficient for a fully offline git build.
///
/// Non-git sources are reported in [`VcsPrefetchSummary::needs_network`]: omg
/// cannot reproduce `svn`/`hg`/`bzr`/`fossil` fetches, so those still need
/// `aur.allow_network = true` or a pre-populated SRCDEST.
pub async fn prefetch_vcs_sources(sources: &[VcsSource], srcdest: &Path) -> VcsPrefetchSummary {
    let mut summary = VcsPrefetchSummary::default();

    for source in sources {
        if validate_source_filename(&source.filename).is_err() {
            warn!(
                "Rejecting VCS source with an unsafe SRCDEST filename: {:?}",
                source.raw
            );
            summary.needs_network.push(source.raw.clone());
            continue;
        }

        let destination = srcdest.join(&source.filename);
        if is_populated_directory(&destination).await {
            summary.cached.push(source.raw.clone());
            continue;
        }

        if source.protocol != "git" || !is_prefetchable_git_url(&source.url) {
            summary.needs_network.push(source.raw.clone());
            continue;
        }

        match mirror_git_repository(&source.url, &destination).await {
            Ok(()) => {
                debug!("Mirrored VCS source {} into SRCDEST", source.url);
                summary.prefetched.push(source.raw.clone());
            }
            Err(error) => {
                // A concurrent prefetch may have won the race; treat a now
                // populated destination as success rather than failing the
                // build for an already-satisfied source.
                if is_populated_directory(&destination).await {
                    summary.cached.push(source.raw.clone());
                } else {
                    warn!("Failed to mirror VCS source {}: {error:#}", source.url);
                    summary.needs_network.push(source.raw.clone());
                }
            }
        }
    }

    summary
}

/// True when `path` is a directory holding at least one entry.
///
/// Symlinks are deliberately not followed: an SRCDEST entry planted as a
/// symlink must never be treated as a valid cache.
async fn is_populated_directory(path: &Path) -> bool {
    match tokio::fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.is_dir() => {
            std::fs::read_dir(path).is_ok_and(|mut entries| entries.next().is_some())
        }
        _ => false,
    }
}

/// Schemes omg is willing to mirror on its own.
///
/// Plain `http://` and `git://` are excluded because the transport is
/// unauthenticated; those keep requiring the explicit `aur.allow_network`
/// opt-in (makepkg then fetches them inside the sandbox).
fn is_prefetchable_git_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("ssh://") || url.starts_with("file://")
}

/// `git clone --mirror` a source into its SRCDEST entry, atomically.
async fn mirror_git_repository(url: &str, destination: &Path) -> Result<()> {
    let parent = destination.parent().unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(parent).await.ok();

    // Clear a stale non-directory or empty-directory occupant, but never a
    // symlink: a planted link could redirect the mirror outside SRCDEST.
    match tokio::fs::symlink_metadata(destination).await {
        Ok(metadata) => {
            anyhow::ensure!(
                !metadata.file_type().is_symlink(),
                "refusing to mirror into a symlinked SRCDEST entry: {}",
                destination.display()
            );
            if metadata.is_dir() {
                tokio::fs::remove_dir(destination)
                    .await
                    .with_context(|| format!("Failed to clear {}", destination.display()))?;
            } else {
                tokio::fs::remove_file(destination)
                    .await
                    .with_context(|| format!("Failed to clear {}", destination.display()))?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("Failed to inspect {}", destination.display()));
        }
    }

    // Clone into a same-filesystem staging path so an interrupted clone never
    // leaves a half-populated mirror that makepkg would accept as a cache.
    let staging = tempfile::Builder::new()
        .prefix(".vcs-")
        .tempdir_in(parent)
        .with_context(|| format!("Failed to stage a mirror in {}", parent.display()))?;
    let staging_path = staging.path().join("mirror");

    let mut command = tokio::process::Command::new("git");
    command
        .arg("clone")
        .arg("--mirror")
        .arg("--origin=origin")
        .arg("--")
        .arg(url)
        .arg(&staging_path)
        // The URL comes from an untrusted PKGBUILD: never read ambient or
        // repository configuration and never prompt for credentials.
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .env_remove("GIT_ASKPASS")
        .env_remove("SSH_ASKPASS");

    let output = command
        .output()
        .await
        .context("Failed to run git clone --mirror")?;
    anyhow::ensure!(
        output.status.success(),
        "git clone --mirror failed for {url}: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    );

    tokio::fs::rename(&staging_path, destination)
        .await
        .with_context(|| format!("Failed to publish mirror at {}", destination.display()))?;
    Ok(())
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
            if validate_source_filename(&filename).is_err() {
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
        // Redirects stay manual (Policy::none): each hop must re-resolve and
        // re-validate as public-only for SSRF protection, which reqwest's
        // automatic redirect policy cannot do. Total `.timeout()` is avoided
        // here because it covers the whole body download and aborts large
        // files; `.read_timeout()` only fires on stalled reads, matching
        // core::http::download_client.
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(std::time::Duration::from_secs(10))
            .read_timeout(std::time::Duration::from_mins(1))
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

    // --- VCS sources ---------------------------------------------------

    #[test]
    fn git_source_matches_makepkg_filename_and_url() {
        // The exact shape reported in the field: `?signed` plus a `#tag=`
        // fragment must not change the SRCDEST entry name, and the URL handed
        // to git must drop both, because makepkg compares the mirror's
        // `remote.origin.url` against the stripped form.
        let source = extract_vcs_source(
            "git+https://gitlab.freedesktop.org/gstreamer/orc.git?signed#tag=0.4.44",
        )
        .expect("git+ sources must be recognised");

        assert_eq!(source.protocol, "git");
        assert_eq!(source.filename, "orc");
        assert_eq!(
            source.url,
            "https://gitlab.freedesktop.org/gstreamer/orc.git"
        );
    }

    #[test]
    fn vcs_source_honours_rename_syntax() {
        let source = extract_vcs_source("custom::git+https://example.com/real-repo.git#tag=v1")
            .expect("renamed git+ sources must be recognised");

        assert_eq!(source.filename, "custom");
        assert_eq!(source.url, "https://example.com/real-repo.git");
    }

    #[test]
    fn other_vcs_protocols_are_recognised_without_a_git_suffix() {
        for (entry, protocol, filename) in [
            ("svn+https://svn.example.com/project/trunk", "svn", "trunk"),
            ("hg+https://hg.example.com/repo", "hg", "repo"),
            (
                "bzr+https://launchpad.net/bzr-project",
                "bzr",
                "bzr-project",
            ),
            (
                "fossil+https://fossil.example.com/repo",
                "fossil",
                "repo.fossil",
            ),
        ] {
            let source = extract_vcs_source(entry).expect("VCS entry must be recognised");
            assert_eq!(source.protocol, protocol, "{entry}");
            assert_eq!(source.filename, filename, "{entry}");
        }
    }

    #[test]
    fn plain_https_and_local_sources_are_not_vcs() {
        assert!(extract_vcs_source("https://example.com/file.tar.gz").is_none());
        assert!(extract_vcs_source("local-file.patch").is_none());
        assert!(extract_vcs_source("https://example.com/repo.git").is_none());
    }

    #[test]
    fn only_authenticated_transports_are_mirrored_by_omg() {
        assert!(is_prefetchable_git_url("https://example.com/repo.git"));
        assert!(is_prefetchable_git_url("ssh://git@example.com/repo.git"));
        assert!(is_prefetchable_git_url("file:///srv/repo.git"));
        // Unauthenticated transports keep requiring `aur.allow_network`.
        assert!(!is_prefetchable_git_url("http://example.com/repo.git"));
        assert!(!is_prefetchable_git_url("git://example.com/repo.git"));
    }

    #[test]
    fn source_filenames_may_not_escape_srcdest() {
        for rejected in ["", ".", "..", "a/b", "a\\b", "/absolute"] {
            assert!(
                validate_source_filename(rejected).is_err(),
                "{rejected:?} must be rejected"
            );
        }
        for accepted in ["orc", "source.tar.gz", ".hidden"] {
            assert!(
                validate_source_filename(accepted).is_ok(),
                "{accepted:?} must be accepted"
            );
        }
    }

    #[tokio::test]
    async fn git_sources_are_mirrored_into_srcdest_and_then_cached() {
        let temp = tempfile::tempdir().expect("tempdir");
        let origin = temp.path().join("origin");
        std::fs::create_dir_all(&origin).expect("create origin");

        // Build the fixture with a single shell invocation so this test forks
        // the test process once instead of once per Git command. Other suites
        // rely on `flock` timing, and every fork in this process briefly
        // duplicates the whole descriptor table.
        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(
                "set -e\n\
                 git init --quiet --initial-branch=main\n\
                 printf hello > README\n\
                 git add README\n\
                 git commit --quiet -m init\n\
                 git tag v1\n",
            )
            .current_dir(&origin)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.com")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.com")
            .status()
            .expect("git available");
        assert!(status.success(), "failed to build the origin fixture");

        let srcdest = temp.path().join("srcdest");
        std::fs::create_dir_all(&srcdest).expect("create srcdest");
        let source = VcsSource {
            protocol: "git".to_string(),
            url: format!("file://{}", origin.display()),
            filename: "mirror".to_string(),
            raw: "git+file:///origin#tag=v1".to_string(),
        };

        let first = prefetch_vcs_sources(std::slice::from_ref(&source), &srcdest).await;
        assert_eq!(
            first.prefetched.len(),
            1,
            "mirror must be created: {first:?}"
        );
        assert!(first.needs_network.is_empty(), "{first:?}");
        // A bare mirror keeps the tag makepkg checks out for `#tag=`.
        assert!(srcdest.join("mirror").join("objects").is_dir());

        // A second pass must reuse the mirror instead of re-cloning.
        let second = prefetch_vcs_sources(std::slice::from_ref(&source), &srcdest).await;
        assert_eq!(second.cached.len(), 1, "mirror must be reused: {second:?}");
        assert!(second.prefetched.is_empty(), "{second:?}");
    }

    #[tokio::test]
    async fn unprefetchable_vcs_sources_are_reported_not_silently_skipped() {
        let temp = tempfile::tempdir().expect("tempdir");
        let srcdest = temp.path().join("srcdest");
        std::fs::create_dir_all(&srcdest).expect("create srcdest");

        let sources = vec![
            VcsSource {
                protocol: "svn".to_string(),
                url: "https://svn.example.com/project/trunk".to_string(),
                filename: "trunk".to_string(),
                raw: "svn+https://svn.example.com/project/trunk".to_string(),
            },
            VcsSource {
                protocol: "git".to_string(),
                url: "http://example.com/repo.git".to_string(),
                filename: "repo".to_string(),
                raw: "git+http://example.com/repo.git".to_string(),
            },
        ];

        let summary = prefetch_vcs_sources(&sources, &srcdest).await;
        assert_eq!(summary.needs_network.len(), 2, "{summary:?}");
        assert!(summary.prefetched.is_empty(), "{summary:?}");
    }
}
