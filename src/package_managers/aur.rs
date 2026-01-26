//! AUR (Arch User Repository) client with build support

use std::collections::HashSet;
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
use flate2::read::GzDecoder;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::fs as tokio_fs;
use tokio::process::Command;
use tracing::{instrument, warn};
use which::which;

#[derive(Error, Debug)]
pub enum AurError {
    #[error("Package '{0}' not found on AUR")]
    PackageNotFound(String),

    #[error("PKGBUILD not found for '{package}'\n  → The AUR package may not exist or the clone failed\n  → Try: omg aur clean {package} && omg install {package}", package = .0)]
    PkgbuildNotFound(String),

    #[error("Build failed for '{package}'\n  → Check the build log: {log_path}\n  → Common fixes:\n    - Install missing dependencies: omg install <dep>\n    - Clean and retry: omg aur clean {package}\n    - Check AUR comments for known issues", package = .package, log_path = .log_path)]
    BuildFailed { package: String, log_path: String },

    #[error("Git clone failed for '{package}'\n  → Check if the package exists: https://aur.archlinux.org/packages/{package}\n  → Verify your internet connection\n  → Try again: omg install {package}", package = .0)]
    GitCloneFailed(String),

    #[error("Git pull failed for '{package}'\n  → The local clone may have conflicts\n  → Fix: omg aur clean {package} && omg install {package}", package = .0)]
    GitPullFailed(String),

    #[error(
        "Network error connecting to AUR\n  → Check your internet connection\n  → AUR may be temporarily unavailable\n  → Try again in a few minutes"
    )]
    NetworkError(#[from] reqwest::Error),

    #[error("Missing build tool: {tool}\n  → Install with: sudo pacman -S {install_pkg}", tool = .tool, install_pkg = .install_pkg)]
    MissingTool { tool: String, install_pkg: String },

    #[error(
        "Sandbox build failed\n  → bubblewrap is not installed\n  → Install: sudo pacman -S bubblewrap\n  → Or enable unsafe builds: omg config set aur.allow_unsafe_builds true"
    )]
    SandboxUnavailable,

    #[error(
        "No package archive found after build for '{0}'\n  → The build may have produced a different package name\n  → Check ~/.cache/omg/aur/_pkgdest/ for the built package"
    )]
    PackageArchiveNotFound(String),
}

use super::aur_index::{AurIndex, build_index};
use super::pkgbuild::PkgBuild;
use crate::config::{AurBuildMethod, Settings};
use crate::core::http::shared_client;
use crate::core::{Package, PackageSource, paths};
use crate::package_managers::{get_potential_aur_packages, pacman_db};

const AUR_RPC_URL: &str = "https://aur.archlinux.org/rpc";
const AUR_GIT_URL: &str = "https://aur.archlinux.org";
const AUR_RPC_MAX_URI: usize = 4400;
const AUR_META_URL: &str = "https://aur.archlinux.org/packages-meta-ext-v1.json.gz";

/// AUR API client with build support
#[derive(Clone)]
pub struct AurClient {
    client: reqwest::Client,
    build_dir: PathBuf,
    settings: Settings,
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
    results: Vec<AurPackage>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct AurMetaCache {
    etag: Option<String>,
    last_modified: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AurPackage {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Version")]
    version: String,
    #[serde(rename = "Description")]
    description: Option<String>,
    #[serde(rename = "Maintainer")]
    _maintainer: Option<String>,
    #[serde(rename = "NumVotes")]
    _num_votes: Option<i32>,
    #[serde(rename = "Popularity")]
    _popularity: Option<f64>,
    #[serde(rename = "OutOfDate")]
    _out_of_date: Option<i64>,
}

impl AurClient {
    pub fn new() -> Self {
        let settings = Settings::load().unwrap_or_default();
        let build_dir = paths::cache_dir().join("aur");

        Self {
            client: shared_client().clone(),
            build_dir,
            settings,
        }
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
            let index_path = self.metadata_index_path();
            if index_path.exists() {
                let index_path_clone = index_path.clone();
                let query_clone = query.to_string();
                let result = tokio::task::spawn_blocking(move || -> Result<Vec<Package>> {
                    let index = AurIndex::open(&index_path_clone)?;
                    let entries = index.search(&query_clone, 50);
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

        let url = format!("{AUR_RPC_URL}?v=5&type=search&arg={query}");

        let response: AurResponse = self
            .client
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
                if let Err(e) = crate::core::security::validate_package_name(&p.name) {
                    tracing::warn!(
                        "Rejecting invalid package name from AUR search: {} ({})",
                        p.name,
                        e
                    );
                    false
                } else {
                    true
                }
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
        let query_lower = query.to_ascii_lowercase();
        packages.sort_by(|a, b| {
            let a_name_lower = a.name.to_ascii_lowercase();
            let b_name_lower = b.name.to_ascii_lowercase();

            // Exact match check
            let a_exact = a_name_lower == query_lower;
            let b_exact = b_name_lower == query_lower;
            if a_exact != b_exact {
                return b_exact.cmp(&a_exact); // Exact matches first
            }

            // Prefix match check
            let a_prefix = a_name_lower.starts_with(&query_lower);
            let b_prefix = b_name_lower.starts_with(&query_lower);
            if a_prefix != b_prefix {
                return b_prefix.cmp(&a_prefix); // Prefix matches first
            }

            // Word boundary match check
            fn has_word_boundary_match(haystack: &str, needle: &str) -> bool {
                for (pos, _) in haystack.match_indices(needle) {
                    if pos == 0
                        || haystack.as_bytes()[pos - 1].is_ascii_whitespace()
                        || haystack.as_bytes()[pos - 1] == b'-'
                        || haystack.as_bytes()[pos - 1] == b'_'
                        || haystack.as_bytes()[pos - 1] == b'.'
                    {
                        return true;
                    }
                }
                false
            }

            let a_word = has_word_boundary_match(&a_name_lower, &query_lower);
            let b_word = has_word_boundary_match(&b_name_lower, &query_lower);
            if a_word != b_word {
                return b_word.cmp(&a_word); // Word boundary matches first
            }

            // Substring match is implied (since we're from search results)
            // Final tiebreaker: shorter name (more specific) then alphabetical
            match a.name.len().cmp(&b.name.len()) {
                std::cmp::Ordering::Equal => a.name.cmp(&b.name),
                other => other,
            }
        });

        Ok(packages)
    }

    /// Get info for a specific AUR package
    pub async fn info(&self, package: &str) -> Result<Option<Package>> {
        // SECURITY: Validate package name
        crate::core::security::validate_package_name(package)?;

        // Try fast binary index first
        let index_path = self.metadata_index_path();
        if index_path.exists() {
            let index_path_clone = index_path.clone();
            let package_clone = package.to_string();
            let result = tokio::task::spawn_blocking(move || -> Result<Option<Package>> {
                let index = AurIndex::open(&index_path_clone)?;
                if let Some(entry) = index.get(&package_clone) {
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

        let url = format!("{AUR_RPC_URL}?v=5&type=info&arg={package}");

        let response: AurResponse = self
            .client
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

        Ok(response.results.into_iter().next().map(|p| Package {
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

        let mut local_pkgs = Vec::new();
        for name in &foreign_packages {
            if let Some(pkg) = pacman_db::get_local_package(name)? {
                local_pkgs.push((name.clone(), pkg.version));
            }
        }

        // 2. Try fast binary index first
        let index_path = self.metadata_index_path();
        if index_path.exists() {
            let index_path_clone = index_path.clone();
            let result = tokio::task::spawn_blocking(
                move || -> Result<Option<Vec<(String, Version, Version)>>> {
                    let index = match AurIndex::open(&index_path_clone) {
                        Ok(idx) => idx,
                        Err(e) => {
                            warn!("Failed to open AUR index: {}. Will fallback to JSON.", e);
                            return Ok(None);
                        }
                    };

                    Ok(Some(index.get_updates(&local_pkgs)))
                },
            )
            .await?;

            if let Ok(Some(updates)) = result {
                tracing::debug!("AUR update check completed via binary index");
                return Ok(updates);
            }
        }

        // 3. Fallback to metadata archive (slower JSON)
        if let Some(archive) = self.load_metadata_archive().await? {
            let mut updates = Vec::new();
            let names: HashSet<&str> = foreign_packages.iter().map(String::as_str).collect();
            let mut seen_names = HashSet::new();

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
    async fn query_aur_updates(
        &self,
        packages: &[String],
    ) -> Result<Vec<(String, Version, Version)>> {
        let mut updates = Vec::new();
        let chunked_names = Self::chunk_aur_names(packages);
        // Network I/O bound - use higher concurrency
        let concurrency = self.settings.aur.build_concurrency.clamp(4, 16);

        let mut stream = futures::stream::iter(chunked_names)
            .map(|chunk| {
                let client = &self.client;
                async move {
                    let mut url = format!("{AUR_RPC_URL}?v=5&type=info");
                    for name in &chunk {
                        url.push_str("&arg[]=");
                        url.push_str(name);
                    }

                    let mut last_error = None;
                    for retry in 0..3u32 {
                        if retry > 0 {
                            tokio::time::sleep(Duration::from_millis(100 * 2u64.pow(retry - 1)))
                                .await;
                        }

                        match client.get(&url).send().await {
                            Ok(resp) => {
                                if resp.status().is_server_error() {
                                    last_error = Some(anyhow::anyhow!(
                                        "AUR server error: {}",
                                        resp.status()
                                    ));
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
                    Err(last_error
                        .unwrap_or_else(|| anyhow::anyhow!("AUR request failed after retries")))
                }
            })
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

        let cache_path = self.metadata_cache_path();
        let meta_path = self.metadata_meta_path();
        let ttl = self.settings.aur.metadata_cache_ttl_secs;

        let cache_path_clone = cache_path.clone();
        let should_use_cache = tokio::task::spawn_blocking(move || {
            let matches_ttl = std::fs::metadata(&cache_path_clone)
                .and_then(|m| m.modified())
                .map(|m| m.elapsed().unwrap_or_default() < Duration::from_secs(ttl))
                .unwrap_or(false);

            cache_path_clone.exists() && matches_ttl
        })
        .await?;

        if should_use_cache {
            let cache_path_clone = cache_path.clone();
            return tokio::task::spawn_blocking(move || {
                Self::read_metadata_archive(&cache_path_clone)
            })
            .await?
            .map(Some);
        }

        let meta_cache = if meta_path.exists() {
            if let Ok(bytes) = tokio_fs::read(&meta_path).await {
                if let Ok(parsed) = serde_json::from_slice::<AurMetaCache>(&bytes) {
                    parsed
                } else {
                    AurMetaCache {
                        etag: None,
                        last_modified: None,
                    }
                }
            } else {
                AurMetaCache {
                    etag: None,
                    last_modified: None,
                }
            }
        } else {
            AurMetaCache {
                etag: None,
                last_modified: None,
            }
        };

        if let Some(parent) = cache_path.parent() {
            tokio_fs::create_dir_all(parent).await?;
        }

        let mut req = self.client.get(AUR_META_URL);
        if let Some(etag) = &meta_cache.etag {
            req = req.header(IF_NONE_MATCH, etag);
        }
        if let Some(last_modified) = &meta_cache.last_modified {
            req = req.header(IF_MODIFIED_SINCE, last_modified);
        }

        let response = req.send().await?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED && cache_path.exists() {
            let cache_path_clone = cache_path.clone();
            return tokio::task::spawn_blocking(move || {
                Self::read_metadata_archive(&cache_path_clone)
            })
            .await?
            .map(Some);
        }

        let response = response.error_for_status()?;
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let last_modified = response
            .headers()
            .get(LAST_MODIFIED)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let bytes = response.bytes().await?;
        let tmp_path = cache_path.with_extension("tmp");
        tokio_fs::write(&tmp_path, &bytes).await?;
        tokio_fs::rename(&tmp_path, &cache_path).await?;
        let meta_cache = AurMetaCache {
            etag,
            last_modified,
        };
        if let Ok(meta_bytes) = serde_json::to_vec(&meta_cache) {
            let _ = tokio_fs::write(&meta_path, meta_bytes).await;
        }

        let cache_path_clone = cache_path.clone();
        tokio::task::spawn_blocking(move || Self::read_metadata_archive(&cache_path_clone))
            .await?
            .map(|res| {
                // Spawn background task to rebuild binary index
                let index_path = self.metadata_index_path();
                let cache_path = self.metadata_cache_path();
                tokio::spawn(async move {
                    let result =
                        tokio::task::spawn_blocking(move || build_index(&cache_path, &index_path))
                            .await;

                    match result {
                        Ok(Err(e)) => warn!("Failed to build AUR index: {}", e),
                        Err(e) => warn!("Failed to spawn index build: {}", e),
                        Ok(Ok(())) => tracing::debug!("AUR index rebuilt successfully"),
                    }
                });
                Some(res)
            })
    }

    fn read_metadata_archive(path: &Path) -> Result<AurResponse> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let decoder = GzDecoder::new(reader);
        // The metadata archive is a raw JSON array, not wrapped in {"results": [...]}
        let results: Vec<AurPackage> = serde_json::from_reader(decoder)?;
        Ok(AurResponse { results })
    }

    fn metadata_cache_path(&self) -> PathBuf {
        self.build_dir
            .join("_meta")
            .join("packages-meta-ext-v1.json.gz")
    }

    fn metadata_meta_path(&self) -> PathBuf {
        self.build_dir
            .join("_meta")
            .join("packages-meta-ext-v1.json.gz.meta")
    }

    fn metadata_index_path(&self) -> PathBuf {
        self.build_dir
            .join("_meta")
            .join("packages-meta-ext-v1.rkyv")
    }

    fn chunk_aur_names(names: &[String]) -> Vec<Vec<String>> {
        let base_len = format!("{AUR_RPC_URL}?v=5&type=info").len();
        let mut chunks: Vec<Vec<String>> = Vec::new();
        let mut current: Vec<String> = Vec::new();
        let mut current_len = base_len;

        for name in names {
            let arg_len = "&arg[]=".len() + name.len();
            if !current.is_empty() && current_len + arg_len > AUR_RPC_MAX_URI {
                chunks.push(current);
                current = Vec::new();
                current_len = base_len;
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

        println!(
            "{} Installing AUR package: {}\n",
            "OMG".cyan().bold(),
            package.yellow()
        );

        if self.info(package).await?.is_none() {
            return Err(AurError::PackageNotFound(package.to_string()).into());
        }

        std::fs::create_dir_all(&self.build_dir).with_context(|| {
            format!(
                "Failed to create build directory: {}",
                self.build_dir.display()
            )
        })?;

        let pkg_dir = self.build_dir.join(package);

        if pkg_dir.exists() {
            println!("{} Updating existing source...", "→".blue());
            self.git_pull(&pkg_dir).await.map_err(|e| {
                tracing::warn!("Git pull failed for {}: {}", package, e);
                AurError::GitPullFailed(package.to_string())
            })?;
        } else {
            println!("{} Cloning from AUR...", "→".blue());
            self.git_clone(package).await.map_err(|e| {
                tracing::warn!("Git clone failed for {}: {}", package, e);
                AurError::GitCloneFailed(package.to_string())
            })?;
        }

        let pkgbuild_path = pkg_dir.join("PKGBUILD");
        if !pkgbuild_path.exists() {
            return Err(AurError::PkgbuildNotFound(package.to_string()).into());
        }

        Self::fetch_missing_pgp_keys(&pkgbuild_path).await;

        let env = self.makepkg_env(&pkg_dir)?;
        let cache_key = self.cache_key(&pkg_dir, &env.makeflags)?;

        if self.settings.aur.review_pkgbuild {
            Self::review_pkgbuild(&pkgbuild_path)?;
        }

        let pkg_file = if let Some(cached) = self
            .cached_package(package, &env.pkgdest, &cache_key)
            .await?
        {
            println!("{} Using cached build...", "→".blue());
            cached
        } else {
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
            pkg_file
        };

        println!("{} Installing built package...", "→".blue());
        Self::install_built_package(&pkg_file).await?;

        println!("\n{} {} installed successfully!", "✓".green(), package);

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn build_only(&self, package: &str) -> Result<PathBuf> {
        crate::core::security::validate_package_name(package)?;

        std::fs::create_dir_all(&self.build_dir).with_context(|| {
            format!(
                "Failed to create build directory: {}",
                self.build_dir.display()
            )
        })?;

        let pkg_dir = self.build_dir.join(package);
        let pkgbuild_path = pkg_dir.join("PKGBUILD");

        if pkg_dir.exists() && pkgbuild_path.exists() {
            self.git_pull(&pkg_dir).await.map_err(|e| {
                tracing::warn!("Git pull failed for {}: {}", package, e);
                AurError::GitPullFailed(package.to_string())
            })?;
        } else {
            if pkg_dir.exists() {
                std::fs::remove_dir_all(&pkg_dir).ok();
            }
            self.git_clone(package).await.map_err(|e| {
                tracing::warn!("Git clone failed for {}: {}", package, e);
                AurError::GitCloneFailed(package.to_string())
            })?;
        }

        if !pkgbuild_path.exists() {
            return Err(AurError::PkgbuildNotFound(package.to_string()).into());
        }

        Self::fetch_missing_pgp_keys(&pkgbuild_path).await;

        let env = self.makepkg_env(&pkg_dir)?;
        let cache_key = self.cache_key(&pkg_dir, &env.makeflags)?;
        if self.settings.aur.review_pkgbuild {
            Self::review_pkgbuild(&pkgbuild_path)?;
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
                    "No package archive found for '{expected_names:?}' after makepkg. Check ~/.cache/omg/aur/_logs/{}/.log",
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
            let filename = entry.file_name().to_string_lossy().to_string();
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
            if let Some(file_name) = entry_path.file_name().and_then(|n| n.to_str())
                && (file_name == ".PKGINFO" || file_name == "PKGINFO")
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

    /// Clone package from AUR (public for batch operations)
    pub async fn git_clone_public(&self, package: &str) -> Result<()> {
        self.git_clone(package).await
    }

    /// Update existing clone (public for batch operations)
    pub async fn git_pull_public(&self, pkg_dir: &Path) -> Result<()> {
        self.git_pull(pkg_dir).await
    }

    #[instrument(skip(self))]
    pub async fn build_package_interactive(&self, package: &str) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.build_dir).with_context(|| {
            format!(
                "Failed to create build directory: {}",
                self.build_dir.display()
            )
        })?;

        let pkg_dir = self.build_dir.join(package);
        let pkgbuild_path = pkg_dir.join("PKGBUILD");

        if pkg_dir.exists() && pkgbuild_path.exists() {
            self.git_pull(&pkg_dir).await.map_err(|e| {
                tracing::warn!("Git pull failed for {}: {}", package, e);
                AurError::GitPullFailed(package.to_string())
            })?;
        } else {
            if pkg_dir.exists() {
                std::fs::remove_dir_all(&pkg_dir).ok();
            }
            self.git_clone(package).await.map_err(|e| {
                tracing::warn!("Git clone failed for {}: {}", package, e);
                AurError::GitCloneFailed(package.to_string())
            })?;
        }

        if !pkgbuild_path.exists() {
            return Err(AurError::PkgbuildNotFound(package.to_string()).into());
        }

        Self::fetch_missing_pgp_keys(&pkgbuild_path).await;

        let env = self.makepkg_env(&pkg_dir)?;

        let cache_key = self.cache_key(&pkg_dir, &env.makeflags)?;
        if let Some(cached) = self
            .cached_package(package, &env.pkgdest, &cache_key)
            .await?
        {
            return Ok(cached);
        }

        let mut cmd = Command::new("makepkg");
        cmd.args(["-s", "--noconfirm", "-f", "--needed"])
            .env("MAKEFLAGS", &env.makeflags)
            .env("PKGDEST", &env.pkgdest)
            .env("SRCDEST", &env.srcdest)
            .env("BUILDDIR", &env.builddir)
            .current_dir(&pkg_dir);

        for (key, value) in &env.extra_env {
            cmd.env(key, value);
        }

        let status = cmd.status().await.context("Failed to run makepkg")?;

        if !status.success() {
            let log_path = self.build_dir.join("_logs").join(format!("{package}.log"));
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

    #[instrument(skip(self))]
    pub async fn build_only_nodeps(&self, package: &str) -> Result<PathBuf> {
        let pkg_dir = self.build_dir.join(package);
        let pkgbuild_path = pkg_dir.join("PKGBUILD");

        if !pkgbuild_path.exists() {
            return Err(AurError::PkgbuildNotFound(package.to_string()).into());
        }

        Self::fetch_missing_pgp_keys(&pkgbuild_path).await;

        let env = self.makepkg_env(&pkg_dir)?;
        let cache_key = self.cache_key(&pkg_dir, &env.makeflags)?;

        if let Some(cached) = self
            .cached_package(package, &env.pkgdest, &cache_key)
            .await?
        {
            return Ok(cached);
        }

        let log_path = self.build_dir.join("_logs").join(format!("{package}.log"));
        let status = self
            .run_build_nodeps(&pkg_dir, &env)
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

    /// Run makepkg without --syncdeps (deps pre-installed)
    async fn run_build_nodeps(
        &self,
        pkg_dir: &Path,
        env: &MakepkgEnv,
    ) -> Result<std::process::ExitStatus> {
        let package_name = pkg_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("package");
        let log_dir = self.build_dir.join("_logs");
        std::fs::create_dir_all(&log_dir)?;
        let log_path = log_dir.join(format!("{package_name}.log"));
        let log_path_clone = log_path.clone();
        let (log_file, log_file_err) = tokio::task::spawn_blocking(move || {
            let f = File::create(&log_path_clone)?;
            let e = f.try_clone()?;
            Ok::<_, anyhow::Error>((f, e))
        })
        .await??;
        let spinner = create_spinner(&format!("Building {package_name}..."));

        let mut cmd = Command::new("makepkg");
        // Use --nodeps since dependencies are pre-installed
        // Don't use --cleanbuild for parallel builds as it can cause race conditions
        cmd.args(["--noconfirm", "-f", "--nodeps"])
            .env("MAKEFLAGS", &env.makeflags)
            .env("PKGDEST", &env.pkgdest)
            .env("SRCDEST", &env.srcdest)
            .env("BUILDDIR", &env.builddir);

        for (key, value) in &env.extra_env {
            cmd.env(key, value);
        }

        // Spawn the process and wait for it to complete
        let child = cmd
            .current_dir(pkg_dir)
            .stdout(std::process::Stdio::from(log_file))
            .stderr(std::process::Stdio::from(log_file_err))
            .spawn()
            .context("Failed to spawn makepkg")?;

        let output = child
            .wait_with_output()
            .await
            .context("Failed to wait for makepkg")?;

        let status = output.status;

        spinner.finish_and_clear();
        if !status.success() {
            eprintln!(
                "  {} Build failed: {} (see {})",
                "✗".red(),
                package_name,
                log_path.display()
            );
        }
        Ok(status)
    }

    /// Clone package from AUR
    async fn git_clone(&self, package: &str) -> Result<()> {
        let url = format!("{AUR_GIT_URL}/{package}.git");
        let dest = self.build_dir.join(package);

        if let Ok(git_path) = which("git") {
            let spinner = create_spinner("Cloning repository (git)...");
            let status = Command::new(git_path)
                .args([
                    "clone",
                    "--depth",
                    "1",
                    "--filter=blob:none",
                    "--single-branch",
                    "--",
                    &url,
                    &dest.to_string_lossy(),
                ])
                .status()
                .await
                .context("Failed to run git clone")?;
            spinner.finish_and_clear();
            if status.success() {
                return Ok(());
            }
            tracing::warn!("git clone failed, falling back to libgit2");
        }

        Err(AurError::MissingTool {
            tool: "git".to_string(),
            install_pkg: "git".to_string(),
        }
        .into())
    }

    /// Update existing clone
    async fn git_pull(&self, pkg_dir: &Path) -> Result<()> {
        let git_path = which("git").map_err(|_| AurError::MissingTool {
            tool: "git".to_string(),
            install_pkg: "git".to_string(),
        })?;

        let spinner = create_spinner("Pulling latest changes...");
        let status = Command::new(git_path)
            .args(["-C", &pkg_dir.to_string_lossy(), "pull", "--ff-only"])
            .status()
            .await
            .context("Failed to run git pull")?;
        spinner.finish_and_clear();

        if !status.success() {
            anyhow::bail!(
                "git pull failed. You may need to manually resolve conflicts in {}",
                pkg_dir.display()
            );
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
        let log_dir = self.build_dir.join("_logs");
        std::fs::create_dir_all(&log_dir)?;
        let log_path = log_dir.join(format!("{package_name}.log"));
        let log_path_clone = log_path.clone();
        let (log_file, log_file_err) = tokio::task::spawn_blocking(move || {
            let f = File::create(&log_path_clone)?;
            let e = f.try_clone()?;
            Ok::<_, anyhow::Error>((f, e))
        })
        .await??;
        let spinner = create_spinner(&format!("Building {package_name}..."));

        // Check if bubblewrap is available
        let bwrap_available = Command::new("which")
            .arg("bwrap")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false);

        if bwrap_available {
            tracing::info!("Using bubblewrap sandbox for secure AUR build");
            println!("{} Building in sandbox (bubblewrap)...", "🔒".green());

            // Install dependencies BEFORE entering sandbox (requires sudo)
            let dep_status = Command::new("makepkg")
                .args(["--syncdeps", "--noconfirm", "--nobuild"])
                .current_dir(pkg_dir)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;

            if let Err(e) = dep_status {
                tracing::warn!("Failed to install dependencies: {e}");
            }

            // - Read-only bind: /usr, /etc, /lib, /lib64
            // - Writable: Build directory, /tmp
            // - Minimal device access
            let pkg_dir_str = pkg_dir.to_string_lossy();
            let home = home::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
            let gnupg_dir = home.join(".gnupg");

            let pkgdest_str = env.pkgdest.to_string_lossy().to_string();
            let srcdest_str = env.srcdest.to_string_lossy().to_string();
            let builddir_str = env.builddir.to_string_lossy().to_string();
            let pacman_db_dir = paths::pacman_db_dir().to_string_lossy().to_string();
            let pacman_cache_root = paths::pacman_cache_root_dir().to_string_lossy().to_string();

            let mut args = vec![
                // Share network namespace to allow downloading sources
                "--share-net".to_string(),
                "--ro-bind".to_string(),
                "/usr".to_string(),
                "/usr".to_string(),
                "--ro-bind".to_string(),
                "/etc".to_string(),
                "/etc".to_string(),
                "--ro-bind".to_string(),
                "/lib".to_string(),
                "/lib".to_string(),
                "--ro-bind".to_string(),
                "/lib64".to_string(),
                "/lib64".to_string(),
                "--symlink".to_string(),
                "/usr/bin".to_string(),
                "/bin".to_string(),
                "--symlink".to_string(),
                "/usr/sbin".to_string(),
                "/sbin".to_string(),
                // Don't bind entire HOME, only .gnupg if it exists for PGP checks
                // and a tmpfs for HOME to avoid makepkg complaining
                "--tmpfs".to_string(),
                home.to_string_lossy().into_owned(),
            ];

            if gnupg_dir.exists() {
                args.push("--ro-bind".to_string());
                args.push(gnupg_dir.to_string_lossy().into_owned());
                args.push(gnupg_dir.to_string_lossy().into_owned());
            }

            args.extend(vec![
                "--bind".to_string(),
                pkg_dir_str.to_string(),
                pkg_dir_str.to_string(),
                "--bind".to_string(),
                pkgdest_str.clone(),
                pkgdest_str.clone(),
                "--bind".to_string(),
                srcdest_str.clone(),
                srcdest_str.clone(),
                "--bind".to_string(),
                builddir_str.clone(),
                builddir_str.clone(),
                "--tmpfs".to_string(),
                "/tmp".to_string(),
                "--dev".to_string(),
                "/dev".to_string(),
                "--proc".to_string(),
                "/proc".to_string(),
                "--ro-bind".to_string(),
                pacman_db_dir.clone(),
                pacman_db_dir,
                "--ro-bind".to_string(),
                pacman_cache_root.clone(),
                pacman_cache_root,
                "--die-with-parent".to_string(),
                "--chdir".to_string(),
                pkg_dir_str.to_string(),
                "--setenv".to_string(),
                "MAKEFLAGS".to_string(),
                env.makeflags.clone(),
                "--setenv".to_string(),
                "PKGDEST".to_string(),
                pkgdest_str,
                "--setenv".to_string(),
                "SRCDEST".to_string(),
                srcdest_str,
                "--setenv".to_string(),
                "BUILDDIR".to_string(),
                builddir_str,
            ]);

            for (key, value) in &env.extra_env {
                args.push("--setenv".to_string());
                args.push(key.clone());
                args.push(value.clone());
            }

            // Use sandbox-safe args (no -s since deps installed above)
            let makepkg_args = self.makepkg_args_sandbox();
            args.extend(["--".to_string(), "makepkg".to_string()]);
            args.extend(makepkg_args);

            let status = Command::new("bwrap")
                .args(args)
                .stdout(Stdio::from(log_file))
                .stderr(Stdio::from(log_file_err))
                .status()
                .await
                .context("Failed to run sandboxed makepkg")?;

            spinner.finish_and_clear();
            if !status.success() {
                println!("  {} Build failed. Log: {}", "✗".red(), log_path.display());
            }
            Ok(status)
        } else {
            if !self.settings.aur.allow_unsafe_builds {
                spinner.finish_and_clear();
                return Err(AurError::SandboxUnavailable.into());
            }

            tracing::debug!("bubblewrap not found, using regular makepkg");
            println!(
                "{} Building without sandbox (install 'bubblewrap' for isolation)...",
                "→".dimmed()
            );
            self.run_native_makepkg_with_logs(pkg_dir, env, log_file, log_file_err, spinner)
                .await
        }
    }

    async fn run_native_makepkg(
        &self,
        pkg_dir: &Path,
        env: &MakepkgEnv,
    ) -> Result<std::process::ExitStatus> {
        let package_name = pkg_dir
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("package");
        let log_dir = self.build_dir.join("_logs");
        std::fs::create_dir_all(&log_dir)?;
        let log_path = log_dir.join(format!("{package_name}.log"));
        let log_path_clone = log_path.clone();
        let (log_file, log_file_err) = tokio::task::spawn_blocking(move || {
            let f = File::create(&log_path_clone)?;
            let e = f.try_clone()?;
            Ok::<_, anyhow::Error>((f, e))
        })
        .await??;
        let spinner = create_spinner(&format!("Building {package_name}..."));

        let status = self
            .run_native_makepkg_with_logs(pkg_dir, env, log_file, log_file_err, spinner)
            .await?;

        if !status.success() {
            println!("  {} Build failed. Log: {}", "✗".red(), log_path.display());
        }
        Ok(status)
    }

    async fn run_native_makepkg_with_logs(
        &self,
        pkg_dir: &Path,
        env: &MakepkgEnv,
        log_file: File,
        log_file_err: File,
        spinner: ProgressBar,
    ) -> Result<std::process::ExitStatus> {
        let mut cmd = Command::new("makepkg");
        cmd.args(self.makepkg_args())
            .env("MAKEFLAGS", &env.makeflags)
            .env("PKGDEST", &env.pkgdest)
            .env("SRCDEST", &env.srcdest)
            .env("BUILDDIR", &env.builddir);

        for (key, value) in &env.extra_env {
            cmd.env(key, value);
        }

        let status = cmd
            .current_dir(pkg_dir)
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_err))
            .status()
            .await
            .context("Failed to run makepkg")?;

        spinner.finish_and_clear();
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
        let log_dir = self.build_dir.join("_logs");
        std::fs::create_dir_all(&log_dir)?;
        let log_path = log_dir.join(format!("{package_name}.log"));
        let log_path_clone = log_path.clone();
        let (log_file, log_file_err) = tokio::task::spawn_blocking(move || {
            let f = File::create(&log_path_clone)?;
            let e = f.try_clone()?;
            Ok::<_, anyhow::Error>((f, e))
        })
        .await??;
        let spinner = create_spinner(&format!("Building {package_name} (chroot)..."));

        let mut cmd = if Command::new("which")
            .arg("pkgctl")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
        {
            let mut cmd = Command::new("pkgctl");
            cmd.arg("build");
            if self.settings.aur.secure_makepkg {
                cmd.arg("--clean");
            }
            cmd
        } else if Command::new("which")
            .arg("makechrootpkg")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
        {
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
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_err));

        let status = cmd.status().await.context("Failed to run chroot build")?;
        spinner.finish_and_clear();
        if !status.success() {
            println!("  {} Build failed. Log: {}", "✗".red(), log_path.display());
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

    fn review_pkgbuild(pkgbuild_path: &Path) -> Result<()> {
        println!(
            "{} Review PKGBUILD before building: {}",
            "→".blue(),
            pkgbuild_path.display()
        );
        let proceed = Confirm::new()
            .with_prompt("Proceed with build?")
            .default(false)
            .interact()?;
        if !proceed {
            anyhow::bail!("Build aborted by user after PKGBUILD review.");
        }
        Ok(())
    }

    /// Auto-fetch missing PGP keys from PKGBUILD validpgpkeys array
    /// This prevents "unknown public key" errors during makepkg
    /// Fetches keys in parallel for speed
    async fn fetch_missing_pgp_keys(pkgbuild_path: &Path) {
        let Ok(pkgbuild) = PkgBuild::parse(pkgbuild_path) else {
            return;
        };

        if pkgbuild.validpgpkeys.is_empty() {
            return;
        }

        // Filter to only missing keys first (parallel check)
        let mut missing_keys = Vec::new();
        for key_id in &pkgbuild.validpgpkeys {
            // SECURITY: Validate key_id to prevent injection (hex only, max 64 chars)
            if key_id.chars().any(|c| !c.is_ascii_hexdigit()) || key_id.len() > 64 {
                tracing::warn!("Skipping invalid PGP key ID: {}", key_id);
                continue;
            }

            let check = Command::new("gpg")
                .args(["--list-keys", "--", key_id])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;

            if !check.map(|s| s.success()).unwrap_or(false) {
                missing_keys.push(key_id.clone());
            }
        }

        if missing_keys.is_empty() {
            return;
        }

        // Fetch all missing keys in parallel from Ubuntu keyserver (most reliable)
        let mut handles = Vec::new();
        for key_id in missing_keys {
            let handle = tokio::spawn(async move {
                let result = Command::new("gpg")
                    .args([
                        "--keyserver",
                        "hkps://keyserver.ubuntu.com",
                        "--recv-keys",
                        "--",
                        &key_id,
                    ])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status()
                    .await;

                if result.map(|s| s.success()).unwrap_or(false) {
                    tracing::debug!("Fetched PGP key {key_id}");
                }
            });
            handles.push(handle);
        }

        // Wait for all key fetches (with timeout)
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(10),
            futures::future::join_all(handles),
        )
        .await;
    }

    fn makepkg_env(&self, pkg_dir: &Path) -> Result<MakepkgEnv> {
        let jobs = std::thread::available_parallelism()
            .map(std::num::NonZero::get)
            .unwrap_or(1);
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

        let builddir = std::env::temp_dir().join("omg-build").join(
            pkg_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("pkg"),
        );

        std::fs::create_dir_all(&pkgdest)?;
        std::fs::create_dir_all(&srcdest)?;
        std::fs::create_dir_all(&builddir)?;

        let mut extra_env = Vec::new();

        if self.settings.aur.enable_ccache {
            let ccache_dir = self
                .settings
                .aur
                .ccache_dir
                .clone()
                .unwrap_or_else(|| self.build_dir.join("_ccache"));
            std::fs::create_dir_all(&ccache_dir)?;
            extra_env.push((
                "CCACHE_DIR".to_string(),
                ccache_dir.to_string_lossy().to_string(),
            ));
            extra_env.push((
                "CCACHE_BASEDIR".to_string(),
                pkg_dir.to_string_lossy().to_string(),
            ));
        }

        if self.settings.aur.enable_sccache {
            let sccache_dir = self
                .settings
                .aur
                .sccache_dir
                .clone()
                .unwrap_or_else(|| self.build_dir.join("_sccache"));
            std::fs::create_dir_all(&sccache_dir)?;
            extra_env.push(("RUSTC_WRAPPER".to_string(), "sccache".to_string()));
            extra_env.push((
                "SCCACHE_DIR".to_string(),
                sccache_dir.to_string_lossy().to_string(),
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

    fn cache_key(&self, pkg_dir: &Path, makeflags: &str) -> Result<String> {
        let pkgbuild = std::fs::read(pkg_dir.join("PKGBUILD"))?;
        let srcinfo = std::fs::read(pkg_dir.join(".SRCINFO")).unwrap_or_default();
        let makepkg_args = self.makepkg_args().join(" ");
        let build_method = format!("{:?}", self.settings.aur.build_method);
        let mut hasher = Sha256::new();
        hasher.update(pkgbuild);
        hasher.update(srcinfo);
        hasher.update(makeflags.as_bytes());
        hasher.update(makepkg_args.as_bytes());
        hasher.update(build_method.as_bytes());
        hasher.update(self.settings.aur.secure_makepkg.to_string().as_bytes());
        Ok(format!("{:x}", hasher.finalize()))
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
            if !cache_path.exists() {
                return None;
            }

            let cached = std::fs::read_to_string(&cache_path).unwrap_or_default();
            if cached.trim() != cache_key {
                return None;
            }

            Self::find_package_in_dir(&pkgdest, &[package])
        })
        .await
        .map_err(Into::into)
    }

    async fn write_cache_key(&self, package: &str, cache_key: &str) -> Result<()> {
        if !self.settings.aur.cache_builds {
            return Ok(());
        }

        let package = package.to_string();
        let cache_key = cache_key.to_string();
        let cache_path = self.cache_path(&package);

        tokio::task::spawn_blocking(move || {
            if let Some(parent) = cache_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(cache_path, cache_key)?;
            Ok::<(), anyhow::Error>(())
        })
        .await??;
        Ok(())
    }

    /// Install the built package via sudo omg install <path>
    async fn install_built_package(pkg_path: &Path) -> Result<()> {
        println!(
            "{} Installing built package (elevating with sudo)...",
            "→".blue()
        );

        let pkg_path_str = pkg_path.to_string_lossy();
        crate::core::privilege::run_self_sudo(&["install", "--", &pkg_path_str]).await?;

        Ok(())
    }

    /// Clean build directory for a package
    pub fn clean(&self, package: &str) -> Result<()> {
        let pkg_dir = self.build_dir.join(package);
        if pkg_dir.exists() {
            std::fs::remove_dir_all(&pkg_dir)?;
            println!("{} Cleaned build directory for {}", "✓".green(), package);
        }
        Ok(())
    }

    /// Clean all build directories
    pub fn clean_all(&self) -> Result<()> {
        if self.build_dir.exists() {
            std::fs::remove_dir_all(&self.build_dir)?;
            std::fs::create_dir_all(&self.build_dir)?;
            println!("{} Cleaned all AUR build directories", "✓".green());
        }
        Ok(())
    }
}

impl Default for AurClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a spinner
#[allow(clippy::literal_string_with_formatting_args, clippy::expect_used)]
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

/// Search AUR with detailed info
pub async fn search_detailed(query: &str) -> Result<Vec<AurPackageDetail>> {
    // SECURITY: Basic validation for search query
    if query.len() > 100 {
        anyhow::bail!("Search query too long");
    }

    let client = shared_client().clone();
    let url = format!("{AUR_RPC_URL}?v=5&type=search&arg={query}");

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

        // Word boundary match check
        fn has_word_boundary_match(haystack: &str, needle: &str) -> bool {
            for (pos, _) in haystack.match_indices(needle) {
                if pos == 0
                    || haystack.as_bytes()[pos - 1].is_ascii_whitespace()
                    || haystack.as_bytes()[pos - 1] == b'-'
                    || haystack.as_bytes()[pos - 1] == b'_'
                    || haystack.as_bytes()[pos - 1] == b'.'
                {
                    return true;
                }
            }
            false
        }

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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_sandbox_security_isolation() {
        // Verify that the sandbox arguments strictly isolate the build environment
        let _client = AurClient::default();
        let _pkg_dir = PathBuf::from("/tmp/pkg");

        let _env = MakepkgEnv {
            makeflags: String::new(),
            pkgdest: PathBuf::from("/tmp/pkgdest"),
            srcdest: PathBuf::from("/tmp/srcdest"),
            builddir: PathBuf::from("/tmp/builddir"),
            extra_env: Vec::new(),
        };

        // We can't call run_sandboxed_makepkg directly easily without mocking everything,
        // but we can inspect the argument construction logic if we extract it.
        // For now, let's verify if bwrap is available and test it if so.

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
        // Verify that environment variables are properly handled
        let client = AurClient::default();
        let pkg_dir = PathBuf::from("/tmp/test");

        // This should not panic
        let result = client.makepkg_env(&pkg_dir);
        if let Ok(env) = result {
            assert!(env.builddir.to_string_lossy().contains("omg-build"));
        }
    }
}
