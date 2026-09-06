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

const GITHUB_OWNER: &str = "PyRo1121";
const GITHUB_REPO: &str = "omg";
const GITHUB_RELEASES_PAGE: &str = "https://github.com/PyRo1121/omg/releases";
const GITHUB_API_LATEST_RELEASE: &str = "https://api.github.com/repos/PyRo1121/omg/releases/latest";

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

/// Upper bound for the update archive download.
///
/// Release tarballs are a few MiB. The cap bounds both `Vec` preallocation
/// driven by the server-reported `Content-Length` and streaming growth, so a
/// hostile release server cannot trigger runaway allocation.
const MAX_DOWNLOAD_BYTES: usize = 256 * 1024 * 1024;

/// Cap on `Vec::with_capacity` preallocation before streaming proves the size.
const MAX_PREALLOC_BYTES: usize = 16 * 1024 * 1024;

/// Update OMG to the latest version
///
/// Fails closed: the pinned SHA-256 sidecar published next to each release
/// archive must be fetched and the archive digest verified before extraction;
/// any missing or malformed checksum aborts the update. The archive's
/// Sigstore build-provenance attestation must also verify. When the GitHub
/// CLI is absent, provenance cannot be checked, so the update is refused
/// unless the user explicitly opts in via the
/// `OMG_SELF_UPDATE_ALLOW_UNVERIFIED_PROVENANCE=1` escape hatch.
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
        artifact.file_name
    );

    // Integrity gate: fetch the pinned digest before downloading the archive.
    let expected_digest = fetch_checksum(&artifact.checksum_url()).await?;

    let bytes = download_verified(
        &artifact.download_url(),
        artifact.file_name.clone(),
        &expected_digest,
    )
    .await?;

    let archive_name = artifact.file_name.clone();

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

        let cursor = std::io::Cursor::new(bytes);
        let decoder = flate2::read::GzDecoder::new(cursor);
        let mut archive = tar::Archive::new(decoder);

        let temp_dir = tempfile::tempdir().context("Failed to create temp directory for update")?;
        archive
            .unpack(temp_dir.path())
            .context("Failed to extract update archive")?;

        let new_binary = locate_binary(temp_dir.path(), &archive_name)
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

/// Locate the `omg` binary inside an unpacked release archive.
///
/// CI wraps the payload in a directory named after the archive
/// (`omg-v1.2.3-x86_64-linux-debian/omg`); a flat root-level `omg` layout is
/// accepted as a fallback.
fn locate_binary(extract_dir: &std::path::Path, archive_name: &str) -> Option<std::path::PathBuf> {
    if let Some(wrapper) = archive_name.strip_suffix(".tar.gz") {
        let wrapped = extract_dir.join(wrapper).join("omg");
        if wrapped.is_file() {
            return Some(wrapped);
        }
    }
    let flat = extract_dir.join("omg");
    flat.is_file().then_some(flat)
}

/// A platform-specific release archive published by CI.
#[derive(Debug)]
struct ReleaseArtifact {
    /// Git tag for the release, e.g. `v1.2.3`.
    tag: String,
    /// Archive file name on the release server, e.g.
    /// `omg-v1.2.3-x86_64-linux-debian.tar.gz`.
    file_name: String,
}

impl ReleaseArtifact {
    /// Archive URL on GitHub Releases.
    fn download_url(&self) -> String {
        format!(
            "https://github.com/{GITHUB_OWNER}/{GITHUB_REPO}/releases/download/{}/{}",
            self.tag, self.file_name
        )
    }

    /// URL of the SHA-256 sidecar published next to the archive.
    fn checksum_url(&self) -> String {
        format!("{}.sha256", self.download_url())
    }
}

/// Parse a semantic version, tolerating a leading `v` and surrounding
/// whitespace.
///
/// `Version` only renders `[0-9A-Za-z-.]` characters, so interpolating it into
/// artifact URLs can never break out of the path segment — unlike the raw
/// server-provided string that was previously used verbatim.
fn parse_version(raw: &str) -> Result<Version> {
    let trimmed = raw.trim().trim_start_matches('v');
    Version::parse(trimmed).with_context(|| format!("invalid semantic version: {raw:?}"))
}

/// Fetch the latest published version from GitHub Releases and validate it.
async fn fetch_latest_version() -> Result<Version> {
    #[derive(serde::Deserialize)]
    struct GithubLatestRelease {
        tag_name: String,
    }

    let response = crate::core::http::shared_client()
        .get(GITHUB_API_LATEST_RELEASE)
        .send()
        .await
        .context("Failed to check for updates")?;
    if !response.status().is_success() {
        anyhow::bail!("Failed to fetch version info: {}", response.status());
    }
    let body = response
        .text()
        .await
        .context("Failed to read GitHub latest-release response body")?;
    let release: GithubLatestRelease =
        serde_json::from_str(&body).context("GitHub latest-release metadata was not valid JSON")?;
    parse_version(&release.tag_name).context("GitHub latest-release tag was not valid semver")
}

/// The `(arch, target)` fragment of the artifacts built by
/// `.github/workflows/release.yml`, for each distro this updater supports.
///
/// Returns `None` when no matching artifact exists.
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

/// Select the release artifact for the running platform.
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
        tag: format!("v{version}"),
        file_name: format!("omg-v{version}-{release_arch}-{target}.tar.gz"),
    })
}

/// Fetch the pinned SHA-256 digest for the artifact from its sidecar file.
async fn fetch_checksum(url: &str) -> Result<String> {
    let safe_url = crate::core::http::redact_url(url);
    let response = crate::core::http::shared_client()
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to fetch checksum sidecar {safe_url}"))?;
    if !response.status().is_success() {
        anyhow::bail!(
            "Checksum sidecar request failed: {} ({safe_url})",
            response.status()
        );
    }
    let body = response
        .text()
        .await
        .context("Failed to read checksum sidecar body")?;
    parse_checksum(&body)
}

/// Parse a `sha256sum`-style sidecar (`<hex>  <file name>`) into the pinned
/// lowercase-hex SHA-256 digest.
///
/// CI emits these with `sha256sum` (Linux), `shasum -a 256` (macOS), and
/// `Get-FileHash` (Windows, which writes CRLF line endings), so `\r` and
/// case differences must be tolerated.
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

/// Stream `url` into memory while hashing it, enforcing the download size
/// cap, then verify the pinned digest before the bytes reach extraction.
///
/// Fails closed: any mismatch aborts the update instead of installing
/// unverified bytes.
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

    // Preallocate from Content-Length only up to the prealloc cap, so a
    // hostile or buggy server cannot force a huge allocation up front; the
    // hard cap below still bounds streaming growth.
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
        if bytes.len() + chunk.len() > MAX_DOWNLOAD_BYTES {
            anyhow::bail!(
                "Update download exceeded the {} MiB size cap",
                MAX_DOWNLOAD_BYTES / (1024 * 1024)
            );
        }
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

    #[test]
    fn locate_binary_finds_wrapped_ci_layout() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let wrapper = tmp.path().join("omg-v1.2.3-x86_64-linux-arch");
        std::fs::create_dir_all(&wrapper).expect("wrapper dir");
        std::fs::write(wrapper.join("omg"), b"#!/bin/sh\n").expect("binary");
        assert_eq!(
            locate_binary(tmp.path(), "omg-v1.2.3-x86_64-linux-arch.tar.gz"),
            Some(wrapper.join("omg"))
        );
    }

    #[test]
    fn locate_binary_finds_flat_layout_fallback() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let flat = tmp.path().join("omg");
        std::fs::write(&flat, b"#!/bin/sh\n").expect("binary");
        assert_eq!(
            locate_binary(tmp.path(), "omg-v1.2.3-x86_64-linux-arch.tar.gz"),
            Some(flat)
        );
    }

    #[test]
    fn locate_binary_returns_none_when_archive_has_no_binary() {
        let tmp = tempfile::tempdir().expect("temp dir");
        std::fs::create_dir_all(tmp.path().join("omg-v1.2.3-x86_64-linux-arch"))
            .expect("empty wrapper dir");
        assert_eq!(
            locate_binary(tmp.path(), "omg-v1.2.3-x86_64-linux-arch.tar.gz"),
            None
        );
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
