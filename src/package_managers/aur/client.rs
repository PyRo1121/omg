//! AUR (Arch User Repository) client with build support

use ahash::AHashSet;
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::FromStr;
use std::time::Duration;

use alpm_pkginfo::{PackageInfoV1, PackageInfoV2};
use alpm_srcinfo::SourceInfoV1;
use alpm_types::{Architecture, SystemArchitecture, Version};
use anyhow::{Context, Result};
use dialoguer::Confirm;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio::process::Command;
use tracing::{instrument, warn};
use which::which;

use super::error::AurError;
use super::utils::{
    build_user, create_dir_as_user, create_dir_as_user_sync, has_word_boundary_match,
    is_root_owned, is_symlink, original_user, original_user_home, remove_dir_as_user,
    validate_build_dir,
};

use super::super::aur_deps::check_dependencies;
use super::super::aur_index::AurIndex;
use super::super::aur_metadata::{
    AurJsonPackage, index_path, metadata_path, read_metadata_archive, sync_aur_metadata,
};
use super::super::aur_sources::{download_sources, parse_sources};
#[cfg(feature = "pgp")]
use super::super::pkgbuild::PkgBuild;
use crate::config::{AurBuildMethod, Settings};
use crate::core::http::shared_client;
use crate::core::{Package, PackageSource, paths};
use crate::package_managers::{get_potential_aur_packages, pacman_db};

const AUR_RPC_URL: &str = "https://aur.archlinux.org/rpc";
const AUR_GIT_URL: &str = "https://aur.archlinux.org";
const AUR_RPC_MAX_URI: usize = 4400;

/// Process-wide lock around pacman database mutations (`pacman -U` or a
/// direct ALPM transaction). Pacman serializes installs on
/// `/var/lib/pacman/db.lck`, so concurrent installs — e.g. parallel AUR build
/// waves finishing together — either fail spuriously on the lock or race the
/// ALPM database. Builds stay parallel; installs are applied one at a time.
static INSTALL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
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
///   - `"ABCDEF12"` or any 1–15 hex chars → `ShortKeyId` (rejected)
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
    extra_env: Vec<(String, String)>,
}

#[derive(Debug, Deserialize)]
struct AurResponse {
    results: Vec<AurJsonPackage>,
}

impl AurClient {
    pub fn new() -> Result<Self> {
        let settings = Settings::load().context("Failed to load OMG settings for AUR")?;
        let build_dir = paths::cache_dir().join("aur");

        Ok(Self {
            build_dir,
            settings,
        })
    }

    #[must_use]
    pub fn build_concurrency(&self) -> usize {
        self.settings.aur.build_concurrency.max(1)
    }

    /// Search AUR packages
    pub async fn search(&self, query: &str) -> Result<Vec<Package>> {
        // Basic length check for search query
        if query.len() > 100 {
            anyhow::bail!("Search query too long (max 100 chars)");
        }

        // Prevent control characters
        if query.chars().any(char::is_control) {
            anyhow::bail!("Search query contains invalid control characters");
        }

        // Try fast binary index first if enabled and available
        if self.settings.aur.use_metadata_archive {
            let index_path = Self::metadata_index_path();
            if index_path.exists() {
                let query_owned = query.to_string();
                let result = tokio::task::spawn_blocking(move || -> Result<Vec<Package>> {
                    let index = AurIndex::open(&index_path)?;
                    let entries = index.search(&query_owned, 50)?;
                    Ok(entries
                        .into_iter()
                        .map(|e| Package {
                            name: e.name.as_str().to_string(),
                            version: crate::package_managers::parse_version_or_zero(
                                e.version.as_str(),
                            ),
                            description: e
                                .description
                                .as_ref()
                                .map(|s| s.as_str().to_string())
                                .unwrap_or_default(),
                            source: PackageSource::Aur,
                            installed: false,
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
        }

        let url = format!(
            "{AUR_RPC_URL}?v=5&type=search&arg={}",
            urlencoding::encode(query)
        );

        let response: AurResponse = shared_client()
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("AUR search network error: {}", e);
                anyhow::anyhow!("Failed to connect to AUR. Check your internet connection.")
            })?
            .json()
            .await
            .context("Failed to parse AUR response")?;

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
            .map(|p| Package {
                name: p.name,
                version: crate::package_managers::parse_version_or_zero(&p.version),
                description: p.description.unwrap_or_default(),
                source: PackageSource::Aur,
                installed: false,
            })
            .collect();

        // Sort by relevance: exact name match > prefix match > word boundary > substring > alphabetical
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

        // Try fast binary index first
        let index_path = Self::metadata_index_path();
        if index_path.exists() {
            let package_owned = package.to_string();
            let result = tokio::task::spawn_blocking(move || -> Result<Option<Package>> {
                let index = AurIndex::open(&index_path)?;
                if let Some(entry) = index.get(&package_owned)? {
                    return Ok(Some(Package {
                        name: entry.name.as_str().to_string(),
                        version: crate::package_managers::parse_version_or_zero(
                            entry.version.as_str(),
                        ),
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

            if let Ok(Some(pkg)) = result {
                return Ok(Some(pkg));
            }
        }

        let url = format!(
            "{AUR_RPC_URL}?v=5&type=info&arg={}",
            urlencoding::encode(package)
        );

        let response: AurResponse = shared_client()
            .get(&url)
            .send()
            .await
            .map_err(|e| {
                tracing::warn!("AUR info network error: {}", e);
                anyhow::anyhow!("Failed to connect to AUR. Check your internet connection.")
            })?
            .json()
            .await
            .context("Failed to parse AUR response")?;

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

        Ok(Some(Package {
            name: p.name,
            version: crate::package_managers::parse_version_or_zero(&p.version),
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
        let foreign_packages = get_potential_aur_packages()?;

        if foreign_packages.is_empty() {
            return Ok(Vec::new());
        }

        let mut local_pkgs = Vec::with_capacity(foreign_packages.len());
        for name in &foreign_packages {
            if let Some(pkg) = pacman_db::get_local_package(name)? {
                local_pkgs.push((name.clone(), pkg.version));
            }
        }

        // 2. Try fast binary index first
        let index_path = Self::metadata_index_path();
        if index_path.exists() {
            let result = tokio::task::spawn_blocking(
                move || -> Result<Option<(Vec<(String, Version, Version)>, Vec<String>)>> {
                    let index = match AurIndex::open(&index_path) {
                        Ok(idx) => idx,
                        Err(e) => {
                            warn!("Failed to open AUR index: {}. Will fallback to JSON.", e);
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

        // 3. Fallback to metadata archive (slower JSON)
        if let Some(archive) = self.load_metadata_archive().await? {
            let mut updates = Vec::new();
            let names: AHashSet<&str> = foreign_packages.iter().map(String::as_str).collect();
            let mut seen_names = AHashSet::new();

            for p in archive.results {
                if !names.contains(p.name.as_str()) {
                    continue;
                }
                seen_names.insert(p.name.clone());
                if let Some(local_pkg) = pacman_db::get_local_package(&p.name)? {
                    let p_ver = crate::package_managers::parse_version_or_zero(&p.version);
                    if p_ver > local_pkg.version {
                        updates.push((p.name, local_pkg.version, p_ver));
                    }
                }
            }

            // Query remaining packages not in archive via RPC
            let remaining: Vec<String> = foreign_packages
                .iter()
                .filter(|name| !seen_names.contains(*name))
                .cloned()
                .collect();

            if !remaining.is_empty() {
                let rpc_updates = self.query_aur_updates(&remaining).await?;
                updates.extend(rpc_updates);
            }

            return Ok(updates);
        }

        // 4. Fallback: Query AUR RPC directly
        self.query_aur_updates(&foreign_packages).await
    }

    /// Query AUR RPC for package updates (parallel chunked requests)
    /// Query the AUR RPC `type=info` endpoint for one chunk of package names,
    /// retrying transient failures with exponential backoff.
    async fn rpc_info_chunk(chunk: &[String]) -> Result<AurResponse> {
        let mut url = format!("{AUR_RPC_URL}?v=5&type=info");
        for name in chunk {
            url.push_str("&arg[]=");
            url.push_str(name);
        }

        let mut last_error = None;
        for retry in 0..3u32 {
            if retry > 0 {
                tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(retry - 1))).await;
            }

            match shared_client().get(&url).send().await {
                Ok(resp) => {
                    if resp.status().is_server_error() {
                        last_error = Some(anyhow::anyhow!("AUR server error: {}", resp.status()));
                        continue;
                    }
                    return resp.json::<AurResponse>().await.map_err(Into::into);
                }
                Err(e) if e.is_timeout() || e.is_connect() => {
                    last_error = Some(anyhow::anyhow!("Network error: {e}"));
                }
                Err(e) => return Err(e.into()),
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
            for p in response.results {
                // SECURITY: Validate package name from RPC response
                if let Err(e) = crate::core::security::validate_package_name(&p.name) {
                    tracing::warn!(
                        "Rejecting invalid package name from AUR update check: {} ({})",
                        p.name,
                        e
                    );
                    continue;
                }

                if let Some(local_pkg) = pacman_db::get_local_package(&p.name)? {
                    let p_ver = crate::package_managers::parse_version_or_zero(&p.version);
                    if p_ver > local_pkg.version {
                        updates.push((p.name, local_pkg.version, p_ver));
                    }
                }
            }
        }

        Ok(updates)
    }

    async fn load_metadata_archive(&self) -> Result<Option<AurResponse>> {
        if !self.settings.aur.use_metadata_archive {
            return Ok(None);
        }

        // Sync metadata (this will be fast if already fresh)
        sync_aur_metadata(shared_client(), &self.settings, false).await?;

        let path = metadata_path();
        if path.exists() {
            let results =
                tokio::task::spawn_blocking(move || read_metadata_archive(&path)).await??;
            Ok(Some(AurResponse { results }))
        } else {
            Ok(None)
        }
    }

    fn metadata_index_path() -> PathBuf {
        index_path()
    }

    #[must_use]
    fn chunk_aur_names(names: &[String]) -> Vec<Vec<String>> {
        let mut chunks: Vec<Vec<String>> = Vec::with_capacity((names.len() / 100) + 1);
        let mut current: Vec<String> = Vec::with_capacity(100);
        let mut current_len = AUR_RPC_INFO_BASE_LEN;

        for name in names {
            let arg_len = "&arg[]=".len() + name.len();
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

    pub async fn install(&self, package: &str) -> Result<()> {
        crate::core::security::validate_package_name(package)?;

        require_unprivileged_builder(package, crate::core::is_root())?;

        // Pre-acquire sudo credentials before starting the build.
        // This ensures the sudoloop has a valid timestamp to refresh,
        // and the user is prompted for their password upfront rather
        // than mid-build when it would be confusing.
        if !crate::core::caps::can_write_pacman_db() && !crate::core::is_root() {
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
                        "AUR builds need sudo to install build dependencies, but this is a non-interactive session without passwordless sudo.\n  \
                         → Run from a real terminal, or configure sudo NOPASSWD for build operations.\n  \
                         → Then retry: omg install {package}"
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
                .context("Failed to acquire sudo credentials for AUR build")?;
            if !status.success() {
                anyhow::bail!(
                    "Failed to acquire sudo credentials required for AUR dependency installation.\n  \
                     → Re-run in an interactive terminal and authenticate when prompted.\n  \
                     → Then retry: omg install {package}"
                );
            }
        }

        // Start sudoloop for long build operations.
        // Now that credentials are pre-acquired, the loop will keep
        // them alive throughout the entire build+install cycle.
        let sudoloop = if crate::core::sudoloop::can_use_sudoloop() {
            tracing::debug!("Starting sudoloop for AUR build");
            Some(crate::core::sudoloop::SudoLoop::start())
        } else {
            None
        };

        create_dir_as_user(&self.build_dir).await?;

        // SECURITY: Validate package directory is safe (prevents symlink attacks)
        let pkg_dir = validate_build_dir(&self.build_dir, package)?;

        if pkg_dir.exists() {
            let pull_pb =
                crate::cli::modern_ui::modern_spinner("Updating", &format!("{package} source"));
            if let Err(e) = self.git_pull(&pkg_dir).await {
                crate::cli::modern_ui::finish_clear(&pull_pb);
                tracing::warn!(
                    "Git pull failed for {}: {}. Recovering by recloning package repository.",
                    package,
                    e
                );

                remove_dir_as_user(&pkg_dir).await.map_err(|cleanup_err| {
                    tracing::warn!(
                        "Failed to remove stale AUR cache for {}: {}",
                        package,
                        cleanup_err
                    );
                    AurError::GitPullFailed(package.to_string())
                })?;

                let recover_pb = crate::cli::modern_ui::modern_spinner(
                    "Recovering",
                    &format!("{package} source checkout"),
                );
                self.git_clone(package).await.map_err(|clone_err| {
                    crate::cli::modern_ui::finish_clear(&recover_pb);
                    tracing::warn!("Recovery clone failed for {}: {}", package, clone_err);
                    AurError::GitPullFailed(package.to_string())
                })?;
                crate::cli::modern_ui::finish_success(&recover_pb, "Recovered", "source checkout");
            } else {
                crate::cli::modern_ui::finish_success(&pull_pb, "Updated", "source from AUR");
            }
        } else {
            let clone_pb =
                crate::cli::modern_ui::modern_spinner("Cloning", &format!("{package} from AUR"));
            self.git_clone(package).await.map_err(|e| {
                crate::cli::modern_ui::finish_clear(&clone_pb);
                tracing::warn!("Git clone failed for {}: {}", package, e);
                // Single source of user guidance lives in AurError; the
                // underlying failure is logged above, not duplicated here.
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

        Self::fetch_missing_pgp_keys(&pkgbuild_path).await?;

        let env = self.makepkg_env(&pkg_dir)?;

        let aur_deps = self.missing_aur_dependencies(&pkg_dir, package).await?;
        for dep in aur_deps {
            crate::cli::modern_ui::print_info(&format!(
                "Installing AUR dependency for {package}: {dep}"
            ));
            let dep_pkg = self.build_only(&dep).await?;
            Self::install_built_package(&dep_pkg, sudoloop.as_ref()).await?;
            crate::cli::modern_ui::print_success(&format!("Installed dependency: {dep}"));
        }

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

        if self.settings.aur.review_pkgbuild {
            Self::review_pkgbuild(&pkgbuild_path).await?;
        }

        let pkg_file = if let Some(cached) = self
            .cached_package(package, &env.pkgdest, &cache_key)
            .await?
        {
            crate::cli::modern_ui::print_info(&format!("Using cached build for {package}"));
            cached
        } else {
            let log_path = self.build_dir.join("_logs").join(format!("{package}.log"));

            // Note: run_build() shows its own real-time output, no spinner needed
            let build_start = std::time::Instant::now();
            let status = self
                .run_build(&pkg_dir, &env)
                .await
                .with_context(|| format!("Failed to run makepkg for '{package}'"))?;
            let build_elapsed = build_start.elapsed();

            if !status.success() {
                println!();
                println!("  {} Build failed for {}", "✗".red(), package);
                println!("  {} Check log: {}", "→".dimmed(), log_path.display());
                return Err(AurError::BuildFailed {
                    package: package.to_string(),
                    log_path: log_path.display().to_string(),
                }
                .into());
            }

            println!();
            println!(
                "  {} Built {} in {:.1}s",
                "✓".green().bold(),
                package.bold(),
                build_elapsed.as_secs_f64()
            );

            let pkg_file = Self::find_built_package(&pkg_dir, &env.pkgdest)
                .await
                .map_err(|_| AurError::PackageArchiveNotFound(package.to_string()))?;
            self.write_cache_key(package, &cache_key).await?;
            pkg_file
        };

        println!();
        let install_pb = crate::cli::modern_ui::modern_spinner("Installing", package);
        Self::install_built_package(&pkg_file, sudoloop.as_ref()).await?;
        crate::cli::modern_ui::finish_success(&install_pb, "Installed", package);

        Ok(())
    }

    #[instrument(skip(self))]
    async fn build_only(&self, package: &str) -> Result<PathBuf> {
        crate::core::security::validate_package_name(package)?;

        create_dir_as_user(&self.build_dir).await?;

        // SECURITY: Validate package directory is safe (prevents symlink attacks)
        let pkg_dir = validate_build_dir(&self.build_dir, package)?;
        let pkgbuild_path = pkg_dir.join("PKGBUILD");

        if pkg_dir.exists() && pkgbuild_path.exists() {
            if let Err(e) = self.git_pull(&pkg_dir).await {
                tracing::warn!(
                    "Git pull failed for {}: {}. Recovering by recloning package repository.",
                    package,
                    e
                );
                remove_dir_as_user(&pkg_dir).await.map_err(|cleanup_err| {
                    tracing::warn!(
                        "Failed to remove stale AUR cache for {}: {}",
                        package,
                        cleanup_err
                    );
                    AurError::GitPullFailed(package.to_string())
                })?;
                self.git_clone(package).await.map_err(|clone_err| {
                    tracing::warn!("Recovery clone failed for {}: {}", package, clone_err);
                    AurError::GitPullFailed(package.to_string())
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
            self.git_clone(package).await.map_err(|e| {
                tracing::warn!("Git clone failed for {}: {}", package, e);
                AurError::GitCloneFailed(package.to_string())
            })?;
        }

        if !pkgbuild_path.exists() {
            return Err(AurError::PkgbuildNotFound(package.to_string()).into());
        }

        Self::fetch_missing_pgp_keys(&pkgbuild_path).await?;

        let env = self.makepkg_env(&pkg_dir)?;
        let cache_key = self.cache_key(&pkg_dir, &env.makeflags)?;
        if self.settings.aur.review_pkgbuild {
            Self::review_pkgbuild(&pkgbuild_path).await?;
        }
        if let Some(cached) = self
            .cached_package(package, &env.pkgdest, &cache_key)
            .await?
        {
            return Ok(cached);
        }

        let log_path = self.build_dir.join("_logs").join(format!("{package}.log"));
        let status = self
            .run_build(&pkg_dir, &env)
            .await
            .with_context(|| format!("Failed to run makepkg for '{package}'"))?;

        if !status.success() {
            return Err(AurError::BuildFailed {
                package: package.to_string(),
                log_path: log_path.display().to_string(),
            }
            .into());
        }

        let pkg_file = Self::find_built_package(&pkg_dir, &env.pkgdest)
            .await
            .map_err(|_| AurError::PackageArchiveNotFound(package.to_string()))?;
        self.write_cache_key(package, &cache_key).await?;
        Ok(pkg_file)
    }

    async fn find_built_package(pkg_dir: &Path, pkgdest: &Path) -> Result<PathBuf> {
        let pkg_dir = pkg_dir.to_path_buf();
        let pkgdest = pkgdest.to_path_buf();

        tokio::task::spawn_blocking(move || {
            let mut expected_names = Self::expected_pkg_names(&pkg_dir);
            if expected_names.is_empty() {
                let fallback = pkg_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !fallback.is_empty() {
                    expected_names.push(fallback.to_string());
                }
            }

            // First try pkgdest (shared cache), filtering by expected package names
            let pkg_path = Self::find_package_in_dir(&pkgdest, &expected_names)
                .or_else(|| Self::find_package_in_dir(&pkg_dir, &expected_names));

            pkg_path.ok_or_else(|| {
                anyhow::anyhow!(
                    "No package archive found for '{expected_names:?}' after makepkg. Check ~/.cache/omg/aur/_logs/{}.log",
                    pkg_dir.file_name().and_then(|n| n.to_str()).unwrap_or("unknown")
                )
            })
        })
        .await?
    }

    fn expected_pkg_names(pkg_dir: &Path) -> Vec<String> {
        let srcinfo_path = pkg_dir.join(".SRCINFO");
        let Ok(content) = std::fs::read_to_string(&srcinfo_path) else {
            return Vec::new();
        };
        let Ok(source_info) = SourceInfoV1::from_string(&content) else {
            return Vec::new();
        };

        let mut packages: Vec<_> = source_info
            .packages_for_architecture(SystemArchitecture::X86_64)
            .collect();
        if packages.is_empty() {
            packages = source_info
                .packages_for_architecture(Architecture::Any)
                .collect();
        }

        packages
            .into_iter()
            .map(|pkg| pkg.name.to_string())
            .collect()
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

                // Confirm exact pkgname via .PKGINFO when available
                if let Ok(Some(parsed_name)) = Self::pkg_name_from_archive(&entry.path())
                    && !expected_names.iter().any(|name| name == &parsed_name)
                {
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
        let file = File::open(path)?;
        let reader: Box<dyn Read> = if path.extension().is_some_and(|ext| ext == "zst") {
            let decoder = ruzstd::decoding::StreamingDecoder::new(file)
                .map_err(|e| anyhow::anyhow!("zstd: {e}"))?;
            Box::new(decoder)
        } else if path.extension().is_some_and(|ext| ext == "xz") {
            let mut decompressed = Vec::new();
            lzma_rs::xz_decompress(&mut BufReader::new(file), &mut decompressed)
                .map_err(|e| anyhow::anyhow!("xz: {e}"))?;
            Box::new(Cursor::new(decompressed))
        } else {
            let decoder = flate2::read::GzDecoder::new(file);
            Box::new(decoder)
        };

        let mut archive: tar::Archive<Box<dyn Read>> = tar::Archive::new(reader);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let entry_path = entry.path()?;
            if entry_path.components().count() <= 2
                && let Some(file_name) = entry_path.file_name().and_then(|n| n.to_str())
                && matches!(file_name, ".PKGINFO" | "PKGINFO")
            {
                let mut content = String::new();
                entry.read_to_string(&mut content)?;
                return Ok(Self::parse_pkginfo_name(&content));
            }
        }

        Ok(None)
    }

    fn parse_pkginfo_name(content: &str) -> Option<String> {
        PackageInfoV2::from_str(content)
            .map(|info| info.pkgname.to_string())
            .or_else(|_| PackageInfoV1::from_str(content).map(|info| info.pkgname.to_string()))
            .ok()
    }

    async fn missing_aur_dependencies(&self, pkg_dir: &Path, package: &str) -> Result<Vec<String>> {
        let dep_info = check_dependencies(pkg_dir).unwrap_or_else(|e| {
            tracing::warn!("Unable to inspect dependencies for {}: {}", package, e);
            crate::package_managers::aur_deps::DependencyInfo {
                missing: Vec::new(),
                satisfied: Vec::new(),
                total: 0,
            }
        });

        if dep_info.missing.is_empty() {
            return Ok(Vec::new());
        }

        let mut aur_deps = Vec::new();
        for dep in dep_info.missing {
            let dep_name = dependency_name(&dep);
            if dep_name.is_empty() || dep_name == package {
                continue;
            }

            if crate::package_managers::get_sync_pkg_info(dep_name)
                .ok()
                .flatten()
                .is_some()
            {
                continue;
            }

            let is_aur = self
                .search(dep_name)
                .await
                .map(|results| results.iter().any(|pkg| pkg.name == dep_name))
                .unwrap_or(false);
            if is_aur {
                aur_deps.push(dep_name.to_string());
            }
        }

        aur_deps.sort();
        aur_deps.dedup();
        Ok(aur_deps)
    }

    async fn git_clone(&self, package: &str) -> Result<()> {
        let url = format!("{AUR_GIT_URL}/{package}.git");
        let dest = self.build_dir.join(package);

        let spinner = create_spinner("Cloning repository...");

        if let Some(user) = original_user() {
            let home = original_user_home();
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

            let status = cmd
                .stdin(std::process::Stdio::null())
                .status()
                .await
                .with_context(|| format!("Failed to run git clone as user '{user}'"))?;

            if !status.success() {
                anyhow::bail!("git clone failed for {url}");
            }

            spinner.finish_and_clear();
        } else {
            let status = Command::new("git")
                .args(["clone", "--depth=1", "--filter=blob:none", "--"])
                .arg(&url)
                .arg(&dest)
                .env("GIT_TERMINAL_PROMPT", "0")
                .stdin(std::process::Stdio::null())
                .status()
                .await
                .with_context(|| format!("Failed to run git clone for {url}"))?;
            spinner.finish_and_clear();
            if !status.success() {
                anyhow::bail!("git clone failed for {url}");
            }
        }
        Ok(())
    }

    async fn git_pull(&self, pkg_dir: &Path) -> Result<()> {
        if !crate::core::is_root() && is_root_owned(pkg_dir) {
            let current_user = std::env::var("USER")
                .or_else(|_| whoami::username())
                .unwrap_or_else(|_| "nobody".to_string());
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
            let home = original_user_home();
            let pkg_dir_str = pkg_dir.to_string_lossy();

            let mut cmd = Command::new("sudo");
            cmd.args(["-u", &user]);

            if let Some(ref home_path) = home {
                cmd.arg("-H");
                cmd.env("HOME", home_path);
            }

            cmd.args(["git", "-C", pkg_dir_str.as_ref(), "pull", "--ff-only"]);
            cmd.env("GIT_TERMINAL_PROMPT", "0");

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
            let status = Command::new("git")
                .arg("-C")
                .arg(pkg_dir)
                .args(["pull", "--ff-only"])
                .env("GIT_TERMINAL_PROMPT", "0")
                .stdin(std::process::Stdio::null())
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
    ) -> Result<std::process::ExitStatus> {
        match self.settings.aur.build_method {
            AurBuildMethod::Bubblewrap => self.run_sandboxed_makepkg(pkg_dir, env).await,
            AurBuildMethod::Chroot => self.run_chroot_build(pkg_dir, env).await,
            AurBuildMethod::Native => {
                if !self.settings.aur.allow_unsafe_builds {
                    anyhow::bail!(
                        "Native AUR builds are disabled. Enable 'aur.allow_unsafe_builds' or use bubblewrap/chroot."
                    );
                }
                self.run_native_makepkg(pkg_dir, env).await
            }
        }
    }

    /// Run makepkg with bubblewrap sandboxing if available
    /// Falls back to regular makepkg if bwrap is not installed and unsafe builds are allowed
    async fn run_sandboxed_makepkg(
        &self,
        pkg_dir: &Path,
        env: &MakepkgEnv,
    ) -> Result<std::process::ExitStatus> {
        let package_name = pkg_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("package");
        let spinner = create_spinner(&format!("Building {package_name}..."));

        let bwrap_available = which("bwrap").is_ok();

        if bwrap_available {
            tracing::info!("Using bubblewrap sandbox for secure AUR build");
            println!("{} Building in sandbox (bubblewrap)...", "🔒".green());

            // Install dependencies BEFORE entering sandbox (requires sudo)
            // If running as root, drop to original user or nobody
            let build_user = build_user();

            let mut dep_cmd = if crate::core::is_root() {
                let user = build_user.as_deref().unwrap_or("nobody");

                // Get the original user's home directory
                let user_home = if let Some(ref username) = build_user {
                    // Try to get home directory from passwd
                    std::env::var("SUDO_HOME").ok().or_else(|| {
                        // Fallback: construct from /home/<username>
                        Some(format!("/home/{username}"))
                    })
                } else {
                    None
                };

                let mut c = Command::new("sudo");
                c.args(["-E", "-u", user, "makepkg"]);

                // Set HOME to original user's home directory
                if let Some(home) = user_home {
                    c.env("HOME", &home);
                    // Also set XDG_CACHE_HOME to ensure cache goes to user's directory
                    c.env("XDG_CACHE_HOME", format!("{home}/.cache"));
                }

                c
            } else {
                Command::new("makepkg")
            };

            // Check dependencies using .SRCINFO before running makepkg
            let dep_info = check_dependencies(pkg_dir).unwrap_or_else(|e| {
                tracing::debug!("Failed to check dependencies: {e}");
                // Fallback: empty info means we'll run makepkg --syncdeps
                super::super::aur_deps::DependencyInfo {
                    missing: Vec::new(),
                    satisfied: Vec::new(),
                    total: 0,
                }
            });

            if dep_info.total > 0 {
                if dep_info.missing.is_empty() {
                    println!(
                        "{} All {} dependencies already installed",
                        "✓".green(),
                        dep_info.total
                    );
                } else {
                    println!(
                        "{} Installing {} missing dependencies ({} already satisfied)...",
                        "→".cyan().bold(),
                        dep_info.missing.len(),
                        dep_info.satisfied.len()
                    );
                }
            } else {
                println!("{} No dependencies required", "✓".green());
            }

            // Only run makepkg --syncdeps if there are missing dependencies
            if !dep_info.missing.is_empty() || dep_info.total == 0 {
                println!(
                    "{} Checking and installing dependencies...",
                    "→".cyan().bold()
                );

                let dep_status = dep_cmd
                    .args(["--syncdeps", "--noconfirm", "--nobuild"])
                    .current_dir(pkg_dir)
                    .stdin(Stdio::inherit()) // Allow sudo password prompt
                    .stdout(Stdio::inherit()) // Show makepkg output
                    .stderr(Stdio::inherit()) // Show errors
                    .status()
                    .await;

                match dep_status {
                    Err(e) => {
                        tracing::warn!("Failed to install dependencies: {e}");
                        println!("{} Dependency installation failed: {}", "⚠".yellow(), e);
                        println!(
                            "{} Continuing with build - may fail if deps are missing",
                            "→".dimmed()
                        );
                    }
                    Ok(status) => {
                        if status.success() {
                            println!("{} Dependencies ready", "✓".green());
                        } else {
                            println!(
                                "{} Some dependencies may have failed to install",
                                "⚠".yellow()
                            );
                            println!("{} Continuing with build...", "→".dimmed());
                        }
                    }
                }
            }

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
            let cache_base = paths::cache_dir();
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

            let pkg_dir_str = pkg_dir_canonical.to_string_lossy();
            let home = home::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
            let gnupg_dir = home.join(".gnupg");

            let pkgdest_str = pkgdest_canonical.to_string_lossy();
            let srcdest_str = srcdest_canonical.to_string_lossy();
            let builddir_str = builddir_canonical.to_string_lossy();
            let pacman_db_dir = paths::pacman_db_dir();
            let pacman_db_dir_str = pacman_db_dir.to_string_lossy();
            let pacman_cache_root = paths::pacman_cache_root_dir();
            let pacman_cache_root_str = pacman_cache_root.to_string_lossy();
            let home_str = home.to_string_lossy();
            let gnupg_str = gnupg_dir.to_string_lossy();

            let mut cmd = Command::new("bwrap");
            cmd.args([
                "--share-net",
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

            if gnupg_dir.exists() {
                cmd.args(["--ro-bind"]);
                cmd.arg(&*gnupg_str);
                cmd.arg(&*gnupg_str);
            }

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
            cmd.args(["--die-with-parent", "--chdir"]);
            cmd.arg(&*pkg_dir_str);
            cmd.args(["--setenv", "MAKEFLAGS"]);
            cmd.arg(&env.makeflags);
            cmd.args(["--setenv", "PKGDEST"]);
            cmd.arg(&*pkgdest_str);
            cmd.args(["--setenv", "SRCDEST"]);
            cmd.arg(&*srcdest_str);
            cmd.args(["--setenv", "BUILDDIR"]);
            cmd.arg(&*builddir_str);

            for (key, value) in &env.extra_env {
                cmd.args(["--setenv", key, value]);
            }

            // Use sandbox-safe args (no -s since deps installed above)
            let makepkg_args = self.makepkg_args_sandbox();
            cmd.args(["--", "makepkg"]);
            cmd.args(makepkg_args);

            spinner.finish_and_clear();
            println!(
                "{} Building {} (this may take several minutes for source packages)...",
                "→".cyan().bold(),
                package_name
            );

            // Stream output to both terminal AND log file
            let status = cmd
                .stdin(Stdio::null())
                .stdout(Stdio::inherit()) // Show output in real-time
                .stderr(Stdio::inherit()) // Show errors in real-time
                .status()
                .await
                .context("Failed to run sandboxed makepkg")?;

            if !status.success() {
                println!("  {} Build failed", "✗".red());
            }
            Ok(status)
        } else {
            if !self.settings.aur.allow_unsafe_builds {
                spinner.finish_and_clear();
                return Err(AurError::SandboxUnavailable.into());
            }

            spinner.finish_and_clear();
            tracing::debug!("bubblewrap not found, using regular makepkg");
            println!(
                "{} Building without sandbox (install 'bubblewrap' for isolation)...",
                "→".dimmed()
            );
            self.run_native_makepkg(pkg_dir, env).await
        }
    }

    async fn run_native_makepkg(
        &self,
        pkg_dir: &Path,
        env: &MakepkgEnv,
    ) -> Result<std::process::ExitStatus> {
        let spinner = create_spinner(&format!(
            "Building {}...",
            pkg_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("package")
        ));
        // Get build user (original user from sudo/doas, or fallback to nobody)
        let build_user = build_user();

        // If running as root, drop privileges to original user or nobody
        // makepkg refuses to run as root for security reasons
        let mut cmd = if crate::core::is_root() {
            let user = build_user.as_deref().unwrap_or("nobody");

            // Get the original user's home directory for proper path resolution
            let user_home = if let Some(ref username) = build_user {
                std::env::var("SUDO_HOME")
                    .ok()
                    .or_else(|| Some(format!("/home/{username}")))
            } else {
                None
            };

            tracing::debug!(
                "Running makepkg as user '{}' (de-escalated from root), HOME={:?}",
                user,
                user_home
            );
            let mut c = Command::new("sudo");
            c.args(["-E", "-u", user, "makepkg"]);

            // Set HOME to original user's home directory so paths resolve correctly
            if let Some(home) = user_home {
                c.env("HOME", &home);
                c.env("XDG_CACHE_HOME", format!("{home}/.cache"));
            }

            c
        } else {
            Command::new("makepkg")
        };

        cmd.args(self.makepkg_args())
            .env("MAKEFLAGS", &env.makeflags)
            .env("PKGDEST", &env.pkgdest)
            .env("SRCDEST", &env.srcdest)
            .env("BUILDDIR", &env.builddir);

        for (key, value) in &env.extra_env {
            cmd.env(key, value);
        }

        spinner.finish_and_clear();

        let package_name = pkg_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("package");
        println!(
            "{} Building {} (this may take several minutes for source packages)...",
            "→".cyan().bold(),
            package_name
        );

        // Stream output to both terminal AND log file
        let status = cmd
            .current_dir(pkg_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit()) // Show output in real-time
            .stderr(Stdio::inherit()) // Show errors in real-time
            .status()
            .await
            .context("Failed to run makepkg")?;

        Ok(status)
    }

    async fn run_chroot_build(
        &self,
        pkg_dir: &Path,
        env: &MakepkgEnv,
    ) -> Result<std::process::ExitStatus> {
        let package_name = pkg_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("package");
        let spinner = create_spinner(&format!("Building {package_name} (chroot)..."));

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
            spinner.finish_and_clear();
            anyhow::bail!(
                "Chroot build requires devtools (pkgctl/makechrootpkg). Install devtools or choose bubblewrap/native."
            );
        };

        cmd.current_dir(pkg_dir)
            .env("MAKEFLAGS", &env.makeflags)
            .env("PKGDEST", &env.pkgdest)
            .env("SRCDEST", &env.srcdest)
            .env("BUILDDIR", &env.builddir)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        spinner.finish_and_clear();
        println!(
            "{} Building {} in chroot (this may take several minutes for source packages)...",
            "→".cyan().bold(),
            package_name
        );

        let status = cmd.status().await.context("Failed to run chroot build")?;
        if !status.success() {
            println!("  {} Build failed", "✗".red());
        }
        Ok(status)
    }

    fn makepkg_args(&self) -> Vec<String> {
        let mut args = vec![
            "-s".to_string(),
            "--noconfirm".to_string(),
            "-f".to_string(),
            "--needed".to_string(),
        ];
        if self.settings.aur.secure_makepkg {
            args.push("--cleanbuild".to_string());
        }
        args
    }

    /// Makepkg args for sandboxed builds (no -s since deps are pre-installed)
    fn makepkg_args_sandbox(&self) -> Vec<String> {
        let mut args = vec!["--noconfirm".to_string(), "-f".to_string()];
        if self.settings.aur.secure_makepkg {
            args.push("--cleanbuild".to_string());
        }
        args
    }

    /// Prompt the user to review the PKGBUILD before building.
    ///
    /// Runs the blocking `dialoguer` prompt inside `spawn_blocking` so a
    /// user thinking at the confirmation prompt cannot stall the async
    /// runtime while other parallel builds are in flight.
    async fn review_pkgbuild(pkgbuild_path: &Path) -> Result<()> {
        println!(
            "{} Review PKGBUILD before building: {}",
            "→".blue(),
            pkgbuild_path.display()
        );
        let proceed = tokio::task::spawn_blocking(|| {
            Confirm::new()
                .with_prompt("Proceed with build?")
                .default(false)
                .interact()
        })
        .await
        .context("PKGBUILD review prompt task failed")??;
        if !proceed {
            anyhow::bail!("Build aborted by user after PKGBUILD review.");
        }
        Ok(())
    }

    #[cfg(feature = "pgp")]
    async fn fetch_missing_pgp_keys(pkgbuild_path: &Path) -> Result<()> {
        use crate::core::security::keyserver;

        let pkgbuild = PkgBuild::parse(pkgbuild_path).with_context(|| {
            format!(
                "Failed to parse PKGBUILD at {} for PGP keys",
                pkgbuild_path.display()
            )
        })?;

        if pkgbuild.validpgpkeys.is_empty() {
            return Ok(());
        }

        let gnupg_home = dirs::home_dir()
            .map(|home| home.join(".gnupg"))
            .context("Cannot determine home directory for GnuPG keyring")?;
        let keyring_path = gnupg_home.join("pubring.kbx");

        let mut missing_keys = Vec::with_capacity(pkgbuild.validpgpkeys.len());
        for key_id in &pkgbuild.validpgpkeys {
            require_fetchable_pgp_key_id(key_id)?;
            match keyserver::is_key_in_keyring(key_id, &keyring_path) {
                Ok(true) => {}
                Ok(false) => missing_keys.push(key_id.clone()),
                Err(error) => {
                    anyhow::bail!("Failed to read PGP keyring while checking {key_id}: {error}")
                }
            }
        }

        if missing_keys.is_empty() {
            return Ok(());
        }

        tracing::info!("Fetching {} missing PGP key(s)...", missing_keys.len());

        let results = keyserver::fetch_keys(&missing_keys).await;
        for (key_id, result) in results {
            match result {
                Ok(cert) => {
                    let info = keyserver::get_key_info(&cert);
                    tracing::debug!("Fetched PGP key: {info}");
                    keyserver::append_to_keyring(&cert, &keyring_path)
                        .with_context(|| format!("Failed to save key {key_id} to keyring"))?;
                }
                Err(error) => anyhow::bail!("Failed to fetch PGP key {key_id}: {error}"),
            }
        }
        Ok(())
    }

    #[cfg(not(feature = "pgp"))]
    #[expect(clippy::unused_async)]
    async fn fetch_missing_pgp_keys(_pkgbuild_path: &Path) -> Result<()> {
        tracing::debug!("PGP feature disabled, skipping key fetch");
        Ok(())
    }

    fn makepkg_env(&self, pkg_dir: &Path) -> Result<MakepkgEnv> {
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

        let mut extra_env = Vec::new();

        if self.settings.aur.enable_ccache {
            let ccache_dir = self
                .settings
                .aur
                .ccache_dir
                .clone()
                .unwrap_or_else(|| self.build_dir.join("_ccache"));
            create_dir_as_user_sync(&ccache_dir)?;
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
            extra_env,
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

    async fn cached_package(
        &self,
        package: &str,
        pkgdest: &Path,
        cache_key: &str,
    ) -> Result<Option<PathBuf>> {
        if !self.settings.aur.cache_builds {
            return Ok(None);
        }

        let package = package.to_string();
        let pkgdest = pkgdest.to_path_buf();
        let cache_key = cache_key.to_string();
        let cache_path = self.cache_path(&package);

        tokio::task::spawn_blocking(move || {
            let Some(cached) = Self::read_text_if_exists(&cache_path)? else {
                return Ok(None);
            };
            if cached.trim() != cache_key {
                return Ok(None);
            }

            Ok(Self::find_package_in_dir(&pkgdest, &[package]))
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

    /// Install the built package via direct ALPM or `sudo pacman -U`
    ///
    /// Uses direct `sudo pacman -U` instead of re-executing the omg binary
    /// via `run_self_sudo`. This is critical because `run_self_sudo` spawns
    /// a new sudo session that may not share the parent's cached credentials,
    /// causing a second password prompt. Direct `pacman -U` reuses the same
    /// sudo timestamp that the sudoloop is maintaining (matching yay/paru behavior).
    ///
    /// If a `SudoLoop` is active, refreshes credentials immediately before
    /// the install attempt. Retries once on failure in case credentials
    /// expired during a long build.
    async fn install_built_package(
        pkg_path: &Path,
        sudoloop: Option<&crate::core::sudoloop::SudoLoop>,
    ) -> Result<()> {
        // Serialize database mutations across all concurrent builds.
        let _install_guard = INSTALL_LOCK.lock().await;

        println!("{} Installing built package...", "→".blue());

        // Use direct ALPM if we have capabilities (turbo mode) or running as root
        if crate::core::caps::can_write_pacman_db() {
            let pkg_path_str = pkg_path.to_string_lossy();
            crate::package_managers::execute_transaction(
                vec![pkg_path_str.into_owned()],
                false,
                false,
                None,
            )?;
        } else {
            // Refresh sudo credentials right before install to prevent timeout
            if let Some(sl) = sudoloop {
                sl.refresh_now().await;
            }

            // Use direct `sudo pacman -U` instead of re-executing omg.
            // This stays in the same sudo session the sudoloop is refreshing,
            // avoiding a second authentication prompt.
            const MAX_INSTALL_RETRIES: u32 = 1;

            for attempt in 0..=MAX_INSTALL_RETRIES {
                if attempt > 0 {
                    tracing::warn!(
                        "Retrying package install (attempt {}/{})",
                        attempt + 1,
                        MAX_INSTALL_RETRIES + 1
                    );
                    // Refresh credentials before retry
                    if let Some(sl) = sudoloop {
                        sl.refresh_now().await;
                    }
                }

                let result = tokio::process::Command::new("sudo")
                    .args(["pacman", "-U", "--noconfirm", "--"])
                    .arg(pkg_path)
                    .stdin(std::process::Stdio::inherit())
                    .stdout(std::process::Stdio::inherit())
                    .stderr(std::process::Stdio::inherit())
                    .status()
                    .await
                    .context("Failed to run sudo pacman -U")?;

                if result.success() {
                    return Ok(());
                }

                // On last attempt, report failure
                if attempt == MAX_INSTALL_RETRIES {
                    anyhow::bail!("pacman -U failed with exit code {:?}", result.code());
                }

                tracing::warn!(
                    "pacman -U failed with exit code {:?}, will retry",
                    result.code()
                );
            }
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
            println!("{} Cleaned all AUR build directories", "✓".green());
        }
        Ok(())
    }
}

/// Create a spinner
#[expect(clippy::literal_string_with_formatting_args, clippy::expect_used)] // Static indicatif template is always valid; braces are template syntax not Rust format args
fn create_spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .expect("valid template"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    pb
}

fn dependency_name(dep: &str) -> &str {
    for (idx, ch) in dep.char_indices() {
        if matches!(ch, '>' | '<' | '=') {
            return &dep[..idx];
        }
    }

    dep
}

/// Search AUR with detailed info
pub async fn search_detailed(query: &str) -> Result<Vec<AurPackageDetail>> {
    // SECURITY: Basic validation for search query
    if query.len() > 100 {
        anyhow::bail!("Search query too long");
    }

    let client = shared_client().clone();
    let url = format!(
        "{AUR_RPC_URL}?v=5&type=search&arg={}",
        urlencoding::encode(query)
    );

    let response: AurDetailedResponse = client
        .get(&url)
        .send()
        .await
        .context("Failed to connect to AUR RPC. Check your internet connection.")?
        .json()
        .await
        .context("Failed to parse AUR RPC response")?;

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

    // Sort by relevance: exact name match > prefix match > word boundary > substring > popularity
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
    use std::path::PathBuf;

    /// Verify that bubblewrap's read-only root bind blocks writes outside
    /// the writable mounts our sandbox configures.
    ///
    /// This tests bwrap itself with representative args, NOT the production
    /// argument list built by `run_sandboxed_makepkg` (which cannot be run
    /// hermetically in a unit test). The production args are covered by the
    /// path-validation unit tests (`validate_path_inside`, `is_symlink`).
    #[tokio::test]
    async fn bwrap_readonly_root_blocks_arbitrary_writes() {
        let _client = AurClient::new().expect("test settings must load");
        let _pkg_dir = PathBuf::from("/tmp/pkg");

        let _env = MakepkgEnv {
            makeflags: String::new(),
            pkgdest: PathBuf::from("/tmp/pkgdest"),
            srcdest: PathBuf::from("/tmp/srcdest"),
            builddir: PathBuf::from("/tmp/builddir"),
            extra_env: Vec::new(),
        };

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

    #[test]
    fn test_makepkg_env_sanitization() {
        let client = AurClient::new().expect("test settings must load");
        let dir = tempfile::tempdir().expect("temp dir");
        let pkg_dir = dir.path().join("mypkg");
        std::fs::create_dir(&pkg_dir).expect("pkg dir");

        let env = client
            .makepkg_env(&pkg_dir)
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
                url_len += "&arg[]=".len() + name.len();
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
                url_len += "&arg[]=".len() + name.len();
            }
            assert!(url_len <= AUR_RPC_MAX_URI);
        }

        let total: usize = chunks.iter().map(Vec::len).sum();
        assert_eq!(total, 4);
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

    #[test]
    fn aur_builds_reject_root_and_accept_an_unprivileged_user() {
        require_unprivileged_builder("example", false).expect("regular user");
        let error = require_unprivileged_builder("example", true)
            .expect_err("root builds must be rejected");
        assert!(error.to_string().contains("must not be built as root"));
        assert!(error.to_string().contains("omg install example"));
    }

    #[test]
    fn test_dependency_name_parses_constraints() {
        assert_eq!(dependency_name("simdutf-git"), "simdutf-git");
        assert_eq!(dependency_name("fast_float>=7.0"), "fast_float");
        assert_eq!(dependency_name("foo<2.0"), "foo");
        assert_eq!(dependency_name("bar=1.2.3"), "bar");
    }
}
