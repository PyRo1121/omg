//! `omg self-update` - Update OMG to the latest version

use anyhow::{Context, Result};
use futures::StreamExt;
use owo_colors::OwoColorize;
use semver::Version;
use sha2::{Digest, Sha256};
use std::env;
use std::fs;

use crate::cli::progress::{Accent, Outcome, ProgressTask, TaskKind, TaskSpec};
use crate::cli::style;
use crate::core::env::distro::{Distro, detect_distro};

const GITHUB_RELEASES_PAGE: &str = "https://github.com/PyRo1121/omg/releases";

const RELEASES_BASE_URL: &str = "https://releases.omg.latham.cloud";

// GitHub's latest release cannot replace this pointer because rollback changes
// only the R2 marker.
const LATEST_VERSION_URL: &str = "https://releases.omg.latham.cloud/latest-version";
const MAX_LATEST_VERSION_BYTES: usize = 256;
const MAX_CHECKSUM_BYTES: usize = 1024;

/// Repository used to verify Sigstore build-provenance attestations.
const ATTESTATION_REPO: &str = "PyRo1121/omg";

/// Explicit opt-in that downgrades the provenance gate from fail-closed to
/// warning-only.
///
/// By default `omg self-update` refuses to install when Sigstore provenance
/// cannot be verified (typically because the GitHub CLI is not installed).
/// Setting this variable to `1`, `true`, or `yes` accepts an unverified
/// update deliberately; any other value (including unset) keeps the gate
/// closed.
const ALLOW_UNVERIFIED_PROVENANCE_ENV: &str = "OMG_SELF_UPDATE_ALLOW_UNVERIFIED_PROVENANCE";

/// Hard cap on the update archive download: bounds both `Vec` preallocation
/// driven by the server-reported `Content-Length` and streaming growth, so a
/// hostile release server cannot trigger runaway allocation.
const MAX_DOWNLOAD_BYTES: usize = 256 * 1024 * 1024;

/// Cap on `Vec::with_capacity` preallocation before streaming proves the size.
const MAX_PREALLOC_BYTES: usize = 16 * 1024 * 1024;

/// Update OMG to the latest version.
///
/// The latest version, archive, and checksum come from R2. The archive must
/// also pass GitHub's Sigstore attestation check before installation.
///
/// # Errors
///
/// Returns an error when the update check fails, the target version is not
/// newer (without `--force`), the artifact checksum sidecar is missing or
/// malformed, the download exceeds the size cap or its digest mismatches,
/// the attestation fails to verify, provenance cannot be verified (no `gh`)
/// without the explicit opt-in, or extraction / binary replacement fails.
pub async fn run(force: bool, version: Option<String>) -> Result<()> {
    let current_version = parse_version(env!("CARGO_PKG_VERSION"))
        .context("built-in CARGO_PKG_VERSION is not valid semver")?;
    println!(
        "{} Checking for updates... (current: v{current_version})",
        style::runtime("OMG"),
    );

    #[cfg(feature = "arch")]
    if !force
        && detect_distro() == Distro::Arch
        && let Ok(exe) = env::current_exe()
        && exe.starts_with("/usr/bin")
    {
        println!(
            "  {} Note: OMG is installed in system path ({})",
            style::maybe_color("ℹ", |t| t.blue().to_string()),
            exe.display()
        );
        println!(
            "     Updating via self-update may conflict with system package-managed files.\n\
             Recommended: update via your package manager: {}\n",
            style::command("omg update omg")
        );
    }

    let target_version = match version {
        Some(raw) => parse_version(&raw)
            .with_context(|| format!("`--version {raw}` is not valid semver (e.g. 1.2.3)"))?,
        None => fetch_latest_version().await?,
    };

    // Downgrade protection (audit25 aud-dep-installer): without --force,
    // only strictly newer releases may be installed. Equality and older
    // versions both stop here so a compromised/misconfigured release feed
    // cannot roll a user back to a vulnerable version.
    if !force {
        if target_version == current_version {
            println!(
                "  {} You are already on the latest version.",
                style::maybe_color("✓", |t| t.green().to_string())
            );
            return Ok(());
        }
        if target_version < current_version {
            anyhow::bail!(
                "Refusing to downgrade from {current_version} to {target_version} \
                 (use --force to override)"
            );
        }
    }

    let artifact = release_artifact(&target_version)?;

    println!(
        "  {} Downloading {}...",
        style::maybe_color("⬇", |t| t.blue().to_string()),
        artifact.object_name()
    );

    let bytes = fetch_release_archive(&artifact).await?;

    let archive_name = artifact.object_name();

    // Perform blocking extraction and binary replacement in a separate thread
    // to avoid blocking the tokio async runtime
    let attestation_tag = format!("v{target_version}");
    tokio::task::spawn_blocking(move || -> Result<()> {
        // Provenance gate: `gh attestation verify` requires a file on disk, so
        // stage the digest-verified bytes for both the verification and the
        // subsequent extraction.
        let attestation_file = tempfile::NamedTempFile::new()
            .context("Failed to stage archive for attestation verification")?;
        fs::write(attestation_file.path(), &bytes)
            .context("Failed to write archive for attestation verification")?;
        if !verify_attestation(attestation_file.path(), &attestation_tag)? {
            refuse_unverified_provenance()?;
        }

        let temp_dir = tempfile::tempdir().context("Failed to create temp directory for update")?;
        let new_binary = extract_update_binary(&bytes, temp_dir.path(), &archive_name)
            .context("Failed to extract update binary")?
            .ok_or_else(|| anyhow::anyhow!("update archive did not contain an 'omg' binary"))?;

        let current_exe = env::current_exe().context("Failed to find current executable path")?;
        install_binary_atomically(&new_binary, &current_exe)
            .context("Failed to install updated binary")
    })
    .await??;

    println!(
        "  {} Update successful!",
        style::maybe_color("✓", |t| t.green().to_string())
    );
    println!(
        "  {} is now installed.",
        style::maybe_color(&format!("v{target_version}"), |t| t.cyan().to_string())
    );

    Ok(())
}

fn install_binary_atomically(
    new_binary: &std::path::Path,
    destination: &std::path::Path,
) -> Result<()> {
    let parent = destination
        .parent()
        .context("Current executable has no parent directory")?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)
        .with_context(|| format!("Failed to stage update in {}", parent.display()))?;
    let mut source = fs::File::open(new_binary)
        .with_context(|| format!("Failed to open update payload {}", new_binary.display()))?;
    std::io::copy(&mut source, staged.as_file_mut())
        .context("Failed to copy update payload into executable directory")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        staged
            .as_file_mut()
            .set_permissions(fs::Permissions::from_mode(0o755))
            .context("Failed to set updated binary permissions")?;
    }
    staged
        .as_file_mut()
        .sync_all()
        .context("Failed to sync updated binary")?;
    staged
        .persist(destination)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to replace {}", destination.display()))?;
    crate::core::safe_ops::sync_parent_directory_sync(destination)?;
    Ok(())
}

/// Cap on decompressed update payload: a release tarball holds one binary
/// (~10-30 MiB), so anything larger is a decompression bomb.
const MAX_UPDATE_DECOMPRESSED_BYTES: u64 = 64 * 1024 * 1024;

/// Cap on entry count: a release archive holds a wrapper dir plus binaries.
const MAX_UPDATE_ARCHIVE_ENTRIES: usize = 16;

/// Cap on a single extracted update binary.
const MAX_UPDATE_BINARY_BYTES: u64 = 64 * 1024 * 1024;

/// Extract only the `omg` binary from a release archive.
///
/// CI wraps the payload in a directory named after the archive
/// (`omg-v1.2.3-x86_64-linux-debian/omg`); a flat root-level `omg` layout is
/// accepted as a fallback. Unlike a general extractor this allowlists those
/// two paths and materializes a single regular file: symlinks, hard links,
/// and special entries fail closed instead of being created, so a malicious
/// archive cannot stage link-traversal writes or plant entries outside the
/// temp dir, and the decompression budget stops bombs.
fn extract_update_binary(
    bytes: &[u8],
    extract_dir: &std::path::Path,
    archive_name: &str,
) -> Result<Option<std::path::PathBuf>> {
    use std::io::Read as _;

    let wrapper: Option<std::path::PathBuf> = archive_name
        .strip_suffix(".tar.gz")
        .map(|stem| std::path::PathBuf::from(stem).join("omg"));
    let flat = std::path::PathBuf::from("omg");

    let cursor = std::io::Cursor::new(bytes);
    let decoder = flate2::read::GzDecoder::new(cursor);
    let budgeted =
        crate::runtimes::common::BudgetedReader::new(decoder, MAX_UPDATE_DECOMPRESSED_BYTES);
    let mut archive = tar::Archive::new(budgeted);

    let mut found: Option<std::path::PathBuf> = None;
    let mut entries = 0usize;
    for entry in archive.entries().context("Failed to read update archive")? {
        entries += 1;
        anyhow::ensure!(
            entries <= MAX_UPDATE_ARCHIVE_ENTRIES,
            "Update archive contains too many entries"
        );
        let mut entry = entry.context("Failed to read update archive entry")?;
        let path = entry.path().context("Update archive entry has no path")?;
        let Some(relative) = crate::core::archive::stripped_archive_path(&path, 0)
            .context("Unsafe path in update archive")?
        else {
            continue;
        };
        let wanted = Some(&relative) == wrapper.as_ref() || relative == flat;
        let entry_type = entry.header().entry_type();
        if entry_type.is_dir() {
            continue;
        }
        if !wanted {
            // Release archives carry exactly one payload binary; anything
            // else is ignored rather than published.
            continue;
        }
        anyhow::ensure!(
            entry_type.is_file(),
            "Update binary entry is not a regular file: {}",
            relative.display()
        );
        anyhow::ensure!(
            found.is_none(),
            "Update archive contains a duplicate binary entry: {}",
            relative.display()
        );
        let dest_path = extract_dir.join(&relative);
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create {}", parent.display()))?;
        }
        let mut content = Vec::new();
        entry
            .read_to_end(&mut content)
            .context("Failed to read update binary from archive")?;
        anyhow::ensure!(
            u64::try_from(content.len()).is_ok_and(|len| len <= MAX_UPDATE_BINARY_BYTES),
            "Update binary exceeds the size bound"
        );
        fs::write(&dest_path, &content)
            .with_context(|| format!("Failed to stage update binary {}", dest_path.display()))?;
        found = Some(dest_path);
    }
    Ok(found)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LatestVersion(Version);

impl LatestVersion {
    fn parse(raw: &str) -> Result<Self> {
        if raw.len() > MAX_LATEST_VERSION_BYTES {
            anyhow::bail!(
                "latest-version marker exceeded the {MAX_LATEST_VERSION_BYTES} byte bound"
            );
        }
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            anyhow::bail!("latest-version marker is empty");
        }
        if trimmed.starts_with(['v', 'V']) {
            anyhow::bail!(
                "latest-version marker must be bare semantic version text without a 'v' tag prefix"
            );
        }
        let version =
            Version::parse(trimmed).context("latest-version marker is not valid semantic version");
        Ok(Self(version?))
    }

    fn into_version(self) -> Version {
        self.0
    }
}

#[derive(Debug)]
struct ReleaseArtifact {
    version: Version,
    arch: &'static str,
    target: &'static str,
}

impl ReleaseArtifact {
    fn object_name(&self) -> String {
        format!("omg-v{}-{}-{}.tar.gz", self.version, self.arch, self.target)
    }

    fn archive_url(&self) -> String {
        format!("{RELEASES_BASE_URL}/{}", self.object_name())
    }
}

fn parse_version(raw: &str) -> Result<Version> {
    let trimmed = raw.trim().trim_start_matches('v');
    Version::parse(trimmed).with_context(|| format!("invalid semantic version: {raw:?}"))
}

async fn fetch_latest_version() -> Result<Version> {
    let safe_url = crate::core::http::redact_url(LATEST_VERSION_URL);
    let response = send_get(LATEST_VERSION_URL, &safe_url).await?;
    let body = read_bounded_body(response, MAX_LATEST_VERSION_BYTES, &safe_url).await?;
    LatestVersion::parse(&body).map(LatestVersion::into_version)
}

async fn read_bounded_body(
    response: reqwest::Response,
    max: usize,
    safe_url: &str,
) -> Result<String> {
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk =
            item.with_context(|| format!("Failed to read response body from {safe_url}"))?;
        if body.len().saturating_add(chunk.len()) > max {
            anyhow::bail!("response body from {safe_url} exceeded the {max} byte bound");
        }
        body.extend_from_slice(&chunk);
    }
    String::from_utf8(body)
        .with_context(|| format!("response body from {safe_url} was not UTF-8 text"))
}

async fn send_get(url: &str, safe_url: &str) -> Result<reqwest::Response> {
    let response = crate::core::http::shared_client()
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch {safe_url}"))?;
    if !response.status().is_success() {
        anyhow::bail!("Request failed: {} ({safe_url})", response.status());
    }
    Ok(response)
}

fn release_target(distro: Distro) -> Option<(&'static str, &'static str)> {
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => return None,
    };
    match distro {
        Distro::Arch => Some((arch, "linux-arch")),
        Distro::Debian => Some((arch, "linux-debian")),
        Distro::Ubuntu => Some((arch, "linux-ubuntu")),
        Distro::Fedora => Some((arch, "linux-fedora")),
        Distro::MacOS => Some(("aarch64", "darwin")),
        Distro::Unknown => None,
    }
}

fn release_artifact(version: &Version) -> Result<ReleaseArtifact> {
    let Some((release_arch, target)) = release_target(detect_distro()) else {
        anyhow::bail!(
            "self-update has no release artifact for this platform; \
             download the archive manually from {GITHUB_RELEASES_PAGE}"
        );
    };
    let host_arch = std::env::consts::ARCH;
    if host_arch != release_arch {
        anyhow::bail!(
            "self-update publishes no {target} artifact for {host_arch}; \
             download the archive manually from {GITHUB_RELEASES_PAGE}"
        );
    }
    Ok(ReleaseArtifact {
        version: version.clone(),
        arch: release_arch,
        target,
    })
}

async fn fetch_release_archive(artifact: &ReleaseArtifact) -> Result<Vec<u8>> {
    let archive_url = artifact.archive_url();
    let checksum_url = format!("{archive_url}.sha256");
    let expected_digest = fetch_checksum(&checksum_url).await?;
    download_verified(&archive_url, artifact.object_name(), &expected_digest).await
}

async fn fetch_checksum(url: &str) -> Result<String> {
    let safe_url = crate::core::http::redact_url(url);
    let response = send_get(url, &safe_url).await?;
    let body = read_bounded_body(response, MAX_CHECKSUM_BYTES, &safe_url).await?;
    parse_checksum(&body)
}

fn parse_checksum(body: &str) -> Result<String> {
    let line = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| anyhow::anyhow!("checksum sidecar is empty"))?;
    let digest = line
        .split_whitespace()
        .next()
        .ok_or_else(|| anyhow::anyhow!("checksum sidecar has no digest field"))?;
    if digest.len() != 64 || hex::decode(digest).is_err() {
        anyhow::bail!("checksum sidecar does not contain a valid SHA-256 digest");
    }
    Ok(digest.to_ascii_lowercase())
}

fn check_download_size(streamed: usize, chunk: usize) -> Result<()> {
    if streamed.saturating_add(chunk) > MAX_DOWNLOAD_BYTES {
        anyhow::bail!(
            "Update download exceeded the {} MiB size cap",
            MAX_DOWNLOAD_BYTES / (1024 * 1024)
        );
    }
    Ok(())
}

async fn download_verified(
    url: &str,
    archive_name: String,
    expected_digest: &str,
) -> Result<Vec<u8>> {
    let safe_url = crate::core::http::redact_url(url);
    let response = crate::core::http::download_client()
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to download update archive from {safe_url}"))?;
    if !response.status().is_success() {
        anyhow::bail!("Update download failed: {} ({safe_url})", response.status());
    }

    let prealloc = response
        .content_length()
        .and_then(|len| usize::try_from(len).ok())
        .map_or(0, |len| len.min(MAX_PREALLOC_BYTES));

    let task = ProgressTask::start(&TaskSpec {
        label: archive_name,
        kind: TaskKind::Bytes {
            total: response.content_length().filter(|len| *len > 0),
        },
        accent: Accent::Network,
    });

    let mut bytes = Vec::with_capacity(prealloc);
    let mut hasher = Sha256::new();

    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = item.context("Failed to read update download chunk")?;
        check_download_size(bytes.len(), chunk.len())?;
        hasher.update(&chunk);
        bytes.extend_from_slice(&chunk);
        task.inc(chunk.len() as u64);
    }
    task.finish(Outcome::Done);

    let actual_digest = hex::encode(hasher.finalize());
    if actual_digest != expected_digest {
        anyhow::bail!(
            "Update archive failed integrity verification: \
             expected SHA-256 {expected_digest}, got {actual_digest}"
        );
    }
    Ok(bytes)
}

/// Verify the Sigstore build-provenance attestation of `archive_path` using
/// the GitHub CLI (`gh attestation verify`).
///
/// Release archives carry SLSA provenance attestations generated by GitHub
/// Actions (see `release.yml`); verifying them proves the archive was built
/// by this repository's CI at the pinned commit — closing the trust gap where
/// a compromise of the release bucket could rewrite both binaries and
/// checksum sidecars together.
///
/// Returns `Ok(true)` when the attestation verified, `Ok(false)` when no
/// attestation-capable tool (`gh`) is installed locally, and an error when an
/// attestation tool IS present but rejects the archive (fail closed).
///
/// # Errors
///
/// Returns an error when `gh` is installed and the attestation does not
/// verify (tampered or non-CI-built archive), or when `gh` itself fails to
/// execute the verification.
fn verify_attestation(archive_path: &std::path::Path, tag: &str) -> Result<bool> {
    let output = std::process::Command::new("gh")
        .args(["attestation", "verify"])
        .arg(archive_path)
        .args([
            "-R",
            ATTESTATION_REPO,
            "--source-ref",
            &format!("refs/tags/{tag}"),
            "--signer-workflow",
            "PyRo1121/omg/.github/workflows/release.yml",
        ])
        .stdin(std::process::Stdio::null())
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error).context("Failed to execute GitHub CLI attestation check"),
    };

    if output.status.success() {
        println!(
            "  {} build provenance verified",
            style::maybe_color("🔒", |t| t.green().to_string())
        );
        Ok(true)
    } else {
        Err(anyhow::anyhow!(
            "Sigstore attestation verification FAILED for {}. Possible \\
             supply-chain tampering. Run manually to inspect:\n                 gh attestation verify {} -R {ATTESTATION_REPO}",
            archive_path.display(),
            archive_path.display(),
        ))
    }
}

/// Decide what to do when Sigstore provenance could not be verified.
///
/// Fails closed by default: the update is refused with instructions for
/// verifying the artifact manually. The only way past the gate is the
/// explicit `OMG_SELF_UPDATE_ALLOW_UNVERIFIED_PROVENANCE` opt-in, which
/// downgrades the refusal to a loud warning.
///
/// # Errors
///
/// Returns an error (refusal) unless `allow_unverified` is set.
fn decide_unverified_provenance(allow_unverified: bool) -> Result<()> {
    if allow_unverified {
        println!(
            "  {} PROVENANCE NOT VERIFIED: continuing because \
             {ALLOW_UNVERIFIED_PROVENANCE_ENV} is set",
            style::maybe_color("⚠", |t| t.yellow().to_string())
        );
        return Ok(());
    }
    anyhow::bail!(
        "Refusing to install: Sigstore build provenance could not be verified \
         because no attestation tool (the GitHub CLI, `gh`) is installed. \
         The checksum gate alone cannot detect a compromised release origin \
         that rewrites binaries and sidecars together.\n\
         \nVerify the artifact manually, then retry:\n\
         \x20 1. Install the GitHub CLI (https://cli.github.com)\n\
         \x20 2. Re-run `omg self-update`, or verify by hand:\n\
         \x20    gh attestation verify <archive.tar.gz> -R {ATTESTATION_REPO}\n\
         \nTo accept an unverified update deliberately, re-run with:\n\
         \x20 {ALLOW_UNVERIFIED_PROVENANCE_ENV}=1 omg self-update"
    )
}

/// Evaluate the opt-in escape hatch from the environment only. `--force`
/// no longer bypasses provenance: it governs version and downgrade policy,
/// never trust.
fn refuse_unverified_provenance() -> Result<()> {
    let raw = env::var(ALLOW_UNVERIFIED_PROVENANCE_ENV).ok();
    decide_unverified_provenance(parse_allow_unverified(raw.as_deref()))
}

/// Parse the escape-hatch environment variable.
///
/// Only `1`, `true`, and `yes` (case-sensitive, matching common CI idiom)
/// count as explicit opt-in; unset, empty, misspelled, or falsy values all
/// keep the gate closed.
fn parse_allow_unverified(value: Option<&str>) -> bool {
    matches!(value, Some("1" | "true" | "yes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_accepts_v_prefix_and_surrounding_whitespace() {
        let expected = Version::new(1, 2, 3);
        assert_eq!(
            parse_version("1.2.3").expect("bare semver must parse"),
            expected
        );
        assert_eq!(
            parse_version("v1.2.3").expect("v-prefixed semver must parse"),
            expected
        );
        assert_eq!(
            parse_version("  1.2.3\n").expect("whitespace must be trimmed"),
            expected
        );
        assert_eq!(
            parse_version("v1.2.3-rc.1+b5").expect("pre-release must parse"),
            Version::parse("1.2.3-rc.1+b5").expect("baseline pre-release")
        );
    }

    #[test]
    fn parse_version_rejects_malformed_and_hostile_input() {
        for raw in [
            "",
            "v",
            "not-a-version",
            "1.2",
            "1.2.3.4",
            "1.2.3/../../evil",
            "1.2.3?redirect=x",
            "https://evil.example/1.2.3",
        ] {
            assert!(
                parse_version(raw).is_err(),
                "input {raw:?} must not parse as a version"
            );
        }
    }

    #[test]
    fn latest_version_parses_bare_semver_marker_text() {
        let expected = Version::new(1, 2, 3);
        assert_eq!(
            LatestVersion::parse("1.2.3")
                .expect("bare semver must parse")
                .into_version(),
            expected
        );
        assert_eq!(
            LatestVersion::parse("  1.2.3\n")
                .expect("surrounding whitespace must be tolerated")
                .into_version(),
            expected
        );
        assert_eq!(
            LatestVersion::parse("1.2.3-rc.1+b5")
                .expect("pre-release must parse")
                .into_version(),
            Version::parse("1.2.3-rc.1+b5").expect("baseline pre-release")
        );
    }

    #[test]
    fn latest_version_rejects_malformed_and_hostile_marker_bodies() {
        for raw in [
            "",
            "   \n\n",
            "v1.2.3",
            "V1.2.3",
            "not-a-version",
            "1.2",
            "1.2.3.4",
            "1.2.3/../../evil",
            "https://evil.example/1.2.3",
        ] {
            assert!(
                LatestVersion::parse(raw).is_err(),
                "marker body {raw:?} must be rejected"
            );
        }
    }

    #[test]
    fn latest_version_rejects_marker_bodies_beyond_the_byte_bound() {
        let padded = format!("1.2.3\n{}", " ".repeat(MAX_LATEST_VERSION_BYTES));
        assert!(padded.len() > MAX_LATEST_VERSION_BYTES);
        assert!(LatestVersion::parse(&padded).is_err());
    }

    #[test]
    fn release_artifact_names_match_the_github_release_asset_contract() {
        let artifact = ReleaseArtifact {
            version: Version::parse("1.2.3").expect("baseline version"),
            arch: "x86_64",
            target: "linux-arch",
        };
        assert_eq!(
            artifact.object_name(),
            "omg-v1.2.3-x86_64-linux-arch.tar.gz"
        );
        assert_eq!(
            artifact.archive_url(),
            "https://releases.omg.latham.cloud/omg-v1.2.3-x86_64-linux-arch.tar.gz"
        );
    }

    #[test]
    fn release_artifact_names_include_pre_release_and_build_metadata() {
        let artifact = ReleaseArtifact {
            version: Version::parse("1.2.3-rc.1+b5").expect("baseline pre-release"),
            arch: "aarch64",
            target: "darwin",
        };
        assert_eq!(
            artifact.object_name(),
            "omg-v1.2.3-rc.1+b5-aarch64-darwin.tar.gz"
        );
    }

    #[test]
    fn parse_checksum_accepts_sha256sum_sidecar_format() {
        let digest = "a".repeat(64);
        let body = format!("{digest}  omg-v1.2.3-x86_64-linux-arch.tar.gz\n");
        assert_eq!(
            parse_checksum(&body).expect("sha256sum format must parse"),
            digest
        );
    }

    #[test]
    fn parse_checksum_accepts_crlf_and_uppercase_hex() {
        let body = format!(
            "{}  omg-v1.2.3-x86_64-linux-arch.tar.gz.sha256...\r\n",
            "B".repeat(64)
        );
        assert_eq!(
            parse_checksum(&body).expect("Get-FileHash format must parse"),
            "b".repeat(64)
        );
    }

    #[test]
    fn parse_checksum_skips_leading_blank_lines() {
        let digest = "c".repeat(64);
        let body = format!("\n\n  \n{digest}  omg.tar.gz\n");
        assert_eq!(
            parse_checksum(&body).expect("blank lines must be skipped"),
            digest
        );
    }

    #[test]
    fn parse_checksum_rejects_invalid_payloads() {
        for body in ["", "   \n\n", "no digest field long enough to matter here"] {
            assert!(
                parse_checksum(body).is_err(),
                "sidecar body {body:?} must be rejected"
            );
        }
        assert!(
            parse_checksum(&"g".repeat(64)).is_err(),
            "non-hex characters must be rejected"
        );
        assert!(
            parse_checksum(&format!("{}  omg.tar.gz", "a".repeat(63))).is_err(),
            "truncated digests must be rejected"
        );
    }

    #[test]
    fn download_size_accepts_the_limit_and_rejects_larger_payloads() {
        assert!(check_download_size(MAX_DOWNLOAD_BYTES, 0).is_ok());
        assert!(check_download_size(MAX_DOWNLOAD_BYTES - 1, 1).is_ok());
        assert!(check_download_size(MAX_DOWNLOAD_BYTES, 1).is_err());
        assert!(check_download_size(usize::MAX, 1).is_err());
    }

    #[test]
    fn release_target_matches_ci_artifact_names() {
        let linux_arch = std::env::consts::ARCH;
        assert_eq!(
            release_target(Distro::Arch),
            Some((linux_arch, "linux-arch"))
        );
        assert_eq!(
            release_target(Distro::Debian),
            Some((linux_arch, "linux-debian"))
        );
        assert_eq!(
            release_target(Distro::Ubuntu),
            Some((linux_arch, "linux-ubuntu"))
        );
        assert_eq!(
            release_target(Distro::Fedora),
            Some((linux_arch, "linux-fedora"))
        );
        assert_eq!(release_target(Distro::MacOS), Some(("aarch64", "darwin")));
        assert_eq!(release_target(Distro::Unknown), None);
    }

    #[test]
    fn install_binary_stages_replacement_in_destination_directory() {
        let temp = tempfile::tempdir().expect("temp dir");
        let source_dir = temp.path().join("download");
        let destination_dir = temp.path().join("bin");
        std::fs::create_dir_all(&source_dir).expect("source dir");
        std::fs::create_dir_all(&destination_dir).expect("destination dir");
        let source = source_dir.join("omg");
        let destination = destination_dir.join("omg");
        std::fs::write(&source, b"new binary").expect("source");
        std::fs::write(&destination, b"old binary").expect("destination");

        install_binary_atomically(&source, &destination).expect("install binary");

        assert_eq!(
            std::fs::read(&destination).expect("installed"),
            b"new binary"
        );
        assert_eq!(
            std::fs::read(&source).expect("source remains"),
            b"new binary"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(destination)
                    .expect("metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o755
            );
        }
    }

    fn update_test_tar(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (name, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, name, *content)
                .expect("append tar entry");
        }
        builder.into_inner().expect("finish tar")
    }

    fn gzip_bytes(raw: &[u8]) -> Vec<u8> {
        use std::io::Write as _;
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(raw).expect("gzip tar");
        encoder.finish().expect("finish gzip")
    }

    fn update_test_archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
        gzip_bytes(&update_test_tar(entries))
    }

    /// Rewrite the first entry name of a raw tar archive, recomputing the
    /// header checksum. The safe builder API refuses `..` names, but hostile
    /// archives in the wild are not built with it.
    fn retarget_first_entry(raw: &mut [u8], name: &[u8]) {
        assert!(name.len() < 100, "entry name must fit the tar name field");
        raw[0..100].fill(0);
        raw[0..name.len()].copy_from_slice(name);
        raw[148..156].fill(b' ');
        let checksum: u32 = raw[0..512].iter().map(|byte| u32::from(*byte)).sum();
        let encoded = format!("{checksum:06o}\0 ");
        raw[148..156].copy_from_slice(encoded.as_bytes());
    }

    #[test]
    fn update_extraction_finds_wrapped_ci_layout() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let bytes = update_test_archive(&[("omg-v1.2.3-x86_64-linux-arch/omg", b"#!/bin/sh\n")]);
        let found =
            extract_update_binary(&bytes, tmp.path(), "omg-v1.2.3-x86_64-linux-arch.tar.gz")
                .expect("extraction must succeed");
        let expected = tmp.path().join("omg-v1.2.3-x86_64-linux-arch/omg");
        assert_eq!(found, Some(expected.clone()));
        assert_eq!(std::fs::read(&expected).expect("binary"), b"#!/bin/sh\n");
    }

    #[test]
    fn update_extraction_finds_flat_layout_fallback() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let bytes = update_test_archive(&[("omg", b"#!/bin/sh\n")]);
        let found =
            extract_update_binary(&bytes, tmp.path(), "omg-v1.2.3-x86_64-linux-arch.tar.gz")
                .expect("extraction must succeed");
        let expected = tmp.path().join("omg");
        assert_eq!(found, Some(expected.clone()));
        assert_eq!(std::fs::read(&expected).expect("binary"), b"#!/bin/sh\n");
    }

    #[test]
    fn update_extraction_returns_none_when_archive_has_no_binary() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let bytes = update_test_archive(&[("omg-v1.2.3-x86_64-linux-arch/README", b"docs")]);
        assert_eq!(
            extract_update_binary(&bytes, tmp.path(), "omg-v1.2.3-x86_64-linux-arch.tar.gz",)
                .expect("extraction must succeed"),
            None
        );
    }

    #[test]
    fn update_extraction_refuses_traversal_entry() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut raw = update_test_tar(&[("omg-v1.2.3-x86_64-linux-arch/omg", b"pwned")]);
        retarget_first_entry(&mut raw, b"omg-v1.2.3-x86_64-linux-arch/../../evil");
        let bytes = gzip_bytes(&raw);
        extract_update_binary(&bytes, tmp.path(), "omg-v1.2.3-x86_64-linux-arch.tar.gz")
            .expect_err("traversal entry must fail closed");
        assert!(!tmp.path().join("evil").exists());
    }

    #[test]
    fn update_extraction_refuses_symlink_binary() {
        use std::io::Write as _;
        let raw = {
            let mut builder = tar::Builder::new(Vec::new());
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_cksum();
            builder
                .append_link(
                    &mut header,
                    "omg-v1.2.3-x86_64-linux-arch/omg",
                    "/etc/hostname",
                )
                .expect("append symlink");
            builder.into_inner().expect("finish tar")
        };
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(&raw).expect("gzip tar");
        let bytes = encoder.finish().expect("finish gzip");
        let tmp = tempfile::tempdir().expect("temp dir");
        extract_update_binary(&bytes, tmp.path(), "omg-v1.2.3-x86_64-linux-arch.tar.gz")
            .expect_err("symlink binary must fail closed");
    }

    #[test]
    fn unverified_provenance_refuses_by_default() {
        // SEC-R1-02: without `gh` and without the opt-in, self-update must
        // refuse rather than downgrade to a warning.
        let err = decide_unverified_provenance(false)
            .expect_err("unverified provenance must refuse by default");
        let message = format!("{err:#}");
        assert!(
            message.contains("Refusing to install"),
            "refusal must state it is refusing, got: {message}"
        );
        assert!(
            message.contains("gh attestation verify"),
            "refusal must explain manual verification, got: {message}"
        );
        assert!(
            message.contains(ALLOW_UNVERIFIED_PROVENANCE_ENV),
            "refusal must name the explicit opt-in, got: {message}"
        );
    }

    #[test]
    fn unverified_provenance_proceeds_only_with_explicit_opt_in() {
        decide_unverified_provenance(true)
            .expect("explicit opt-in must be the only path past the gate");
    }

    #[test]
    fn allow_unverified_opt_in_requires_explicit_truthy_value() {
        for value in [
            None,
            Some(""),
            Some("0"),
            Some("false"),
            Some("no"),
            Some("yes "),
            Some("TRUE"),
            Some("on"),
        ] {
            assert!(
                !parse_allow_unverified(value),
                "value {value:?} must not open the escape hatch"
            );
        }
        for value in [Some("1"), Some("true"), Some("yes")] {
            assert!(
                parse_allow_unverified(value),
                "value {value:?} must be an explicit opt-in"
            );
        }
    }
}
