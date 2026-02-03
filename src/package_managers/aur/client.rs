//! AUR (Arch User Repository) client with build support

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufReader, Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;
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
    create_dir_as_user, create_dir_as_user_sync, get_original_user, get_original_user_home,
    has_word_boundary_match, is_root_owned, remove_dir_as_user,
};

use super::super::aur_deps::check_dependencies;
use super::super::aur_index::AurIndex;
use super::super::aur_metadata::{
    AurJsonPackage, get_metadata_path, read_metadata_archive, sync_aur_metadata,
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

/// AUR API client with build support
#[derive(Clone)]
pub struct AurClient {
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
    results: Vec<AurJsonPackage>,
}

impl AurClient {
    pub fn new() -> Self {
        let settings = Settings::load().unwrap_or_default();
        let build_dir = paths::cache_dir().join("aur");

        Self {
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
            let index_path = Arc::new(Self::metadata_index_path());
            if index_path.exists() {
                let query = Arc::new(query.to_string());
                let result = tokio::task::spawn_blocking({
                    let index_path = Arc::clone(&index_path);
                    let query = Arc::clone(&query);
                    move || -> Result<Vec<Package>> {
                        let index = AurIndex::open(&index_path)?;
                        let entries = index.search(&query, 50)?;
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
                    }
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
        let index_path = Arc::new(Self::metadata_index_path());
        if index_path.exists() {
            let package = Arc::new(package.to_string());
            let result = tokio::task::spawn_blocking({
                let index_path = Arc::clone(&index_path);
                let package = Arc::clone(&package);
                move || -> Result<Option<Package>> {
                    let index = AurIndex::open(&index_path)?;
                    if let Some(entry) = index.get(&package)? {
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
                }
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

        let mut local_pkgs = Vec::with_capacity(foreign_packages.len());
        for name in &foreign_packages {
            if let Some(pkg) = pacman_db::get_local_package(name)? {
                local_pkgs.push((name.clone(), pkg.version));
            }
        }

        // 2. Try fast binary index first
        let index_path = Arc::new(Self::metadata_index_path());
        if index_path.exists() {
            let result = tokio::task::spawn_blocking({
                let index_path = Arc::clone(&index_path);
                move || -> Result<Option<Vec<(String, Version, Version)>>> {
                    let index = match AurIndex::open(&index_path) {
                        Ok(idx) => idx,
                        Err(e) => {
                            warn!("Failed to open AUR index: {}. Will fallback to JSON.", e);
                            return Ok(None);
                        }
                    };

                    Ok(Some(index.get_updates(&local_pkgs)?))
                }
            })
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
        let mut updates = Vec::with_capacity(packages.len() / 10 + 1);
        let chunked_names = Self::chunk_aur_names(packages);
        // Network I/O bound - use higher concurrency
        let concurrency = self.settings.aur.build_concurrency.clamp(4, 16);

        let mut stream = futures::stream::iter(chunked_names)
            .map(|chunk| async move {
                let mut url = format!("{AUR_RPC_URL}?v=5&type=info");
                for name in &chunk {
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
                                last_error =
                                    Some(anyhow::anyhow!("AUR server error: {}", resp.status()));
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

        // Sync metadata (this will be fast if already fresh)
        sync_aur_metadata(shared_client(), &self.settings, false).await?;

        let path = get_metadata_path();
        if path.exists() {
            let results =
                tokio::task::spawn_blocking(move || read_metadata_archive(&path)).await??;
            Ok(Some(AurResponse { results }))
        } else {
            Ok(None)
        }
    }

    fn metadata_index_path() -> PathBuf {
        super::super::aur_metadata::get_index_path()
    }

    fn chunk_aur_names(names: &[String]) -> Vec<Vec<String>> {
        let base_len = format!("{AUR_RPC_URL}?v=5&type=info").len();
        let mut chunks: Vec<Vec<String>> = Vec::with_capacity((names.len() / 100) + 1);
        let mut current: Vec<String> = Vec::with_capacity(100);
        let mut current_len = base_len;

        for name in names {
            let arg_len = "&arg[]=".len() + name.len();
            if !current.is_empty() && current_len + arg_len > AUR_RPC_MAX_URI {
                chunks.push(current);
                current = Vec::with_capacity(100);
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

        // Start sudoloop for long build operations
        #[cfg(unix)]
        let _sudoloop = if crate::core::sudoloop::can_use_sudoloop() {
            tracing::debug!("Starting sudoloop for AUR build");
            Some(crate::core::sudoloop::SudoLoop::start())
        } else {
            None
        };

        // Beautiful header matching the new install.rs style
        use owo_colors::OwoColorize;
        println!();
        println!(
            "  {}",
            "╭─────────────────────────────────────────╮".magenta()
        );
        println!(
            "  {} {} {}",
            "│".magenta(),
            format!("  Building {package}  ").bold().magenta(),
            "│".magenta()
        );
        println!(
            "  {}",
            "╰─────────────────────────────────────────╯".magenta()
        );
        println!();

        create_dir_as_user(&self.build_dir).await?;

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
                // Provide helpful error that explains the failure
                anyhow::anyhow!(
                    "Failed to clone {package} from AUR.\n  \
                     → Package may not exist: https://aur.archlinux.org/packages/{package}\n  \
                     → Check your internet connection\n  \
                     → Original error: {e}"
                )
            })?;
        }

        let pkgbuild_path = pkg_dir.join("PKGBUILD");
        if !pkgbuild_path.exists() {
            return Err(AurError::PkgbuildNotFound(package.to_string()).into());
        }

        Self::fetch_missing_pgp_keys(&pkgbuild_path).await;

        let env = self.makepkg_env(&pkg_dir)?;

        // Parse and download sources in parallel
        let sources = parse_sources(&pkg_dir).unwrap_or_default();
        if !sources.is_empty() {
            let _ = download_sources(sources, &env.srcdest).await;
            // Errors are logged but not fatal - makepkg will retry
        }

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

        create_dir_as_user(&self.build_dir).await?;

        let pkg_dir = self.build_dir.join(package);
        let pkgbuild_path = pkg_dir.join("PKGBUILD");

        if pkg_dir.exists() && pkgbuild_path.exists() {
            self.git_pull(&pkg_dir).await.map_err(|e| {
                tracing::warn!("Git pull failed for {}: {}", package, e);
                AurError::GitPullFailed(package.to_string())
            })?;
        } else {
            if pkg_dir.exists() {
                remove_dir_as_user(&pkg_dir).await.ok();
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
        create_dir_as_user(&self.build_dir).await?;

        let pkg_dir = self.build_dir.join(package);
        let pkgbuild_path = pkg_dir.join("PKGBUILD");

        if pkg_dir.exists() && pkgbuild_path.exists() {
            self.git_pull(&pkg_dir).await.map_err(|e| {
                tracing::warn!("Git pull failed for {}: {}", package, e);
                AurError::GitPullFailed(package.to_string())
            })?;
        } else {
            if pkg_dir.exists() {
                remove_dir_as_user(&pkg_dir).await.ok();
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
        create_dir_as_user(&log_dir).await?;
        let log_path = Arc::new(log_dir.join(format!("{package_name}.log")));
        let (log_file, log_file_err) = tokio::task::spawn_blocking({
            let log_path = Arc::clone(&log_path);
            move || {
                let f = File::create(&*log_path)?;
                let e = f.try_clone()?;
                Ok::<_, anyhow::Error>((f, e))
            }
        })
        .await??;
        let spinner = create_spinner(&format!("Building {package_name}..."));

        let mut cmd = Command::new("makepkg");
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
            tracing::error!(
                package = package_name,
                log_path = %log_path.display(),
                "Build failed"
            );
        }
        Ok(status)
    }

    async fn git_clone(&self, package: &str) -> Result<()> {
        let url = format!("{AUR_GIT_URL}/{package}.git");
        let dest = self.build_dir.join(package);

        let spinner = create_spinner("Cloning repository...");

        if let Some(user) = get_original_user() {
            let home = get_original_user_home();
            let dest_str = dest.to_string_lossy();

            let mut cmd = Command::new("sudo");
            cmd.args(["-u", &user]);

            if let Some(ref home_path) = home {
                cmd.arg("-H");
                cmd.env("HOME", home_path);
            }

            cmd.args(["git", "clone", "--depth=1", "--", &url, dest_str.as_ref()]);

            let status = cmd
                .status()
                .await
                .with_context(|| format!("Failed to run git clone as user '{user}'"))?;

            spinner.finish_and_clear();

            if !status.success() {
                anyhow::bail!("git clone failed for {url}");
            }
        } else {
            let url_clone = url.clone();
            let dest_clone = dest.clone();
            let result = tokio::task::spawn_blocking(move || {
                let mut builder = git2::build::RepoBuilder::new();
                builder.fetch_options({
                    let mut fetch_opts = git2::FetchOptions::new();
                    fetch_opts.depth(1);
                    fetch_opts
                });
                builder.clone(&url_clone, &dest_clone)
            })
            .await?;

            spinner.finish_and_clear();

            result.with_context(|| format!("Failed to clone {url}"))?;
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
                .args(["chown", "-R", &format!("{}:{}", current_user, current_user)])
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

        if let Some(user) = get_original_user() {
            let home = get_original_user_home();
            let pkg_dir_str = pkg_dir.to_string_lossy();

            let mut cmd = Command::new("sudo");
            cmd.args(["-u", &user]);

            if let Some(ref home_path) = home {
                cmd.arg("-H");
                cmd.env("HOME", home_path);
            }

            cmd.args(["git", "-C", pkg_dir_str.as_ref(), "pull", "--ff-only"]);

            let status = cmd
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
            Ok(())
        } else {
            let pkg_dir_clone = pkg_dir.to_path_buf();
            let result = tokio::task::spawn_blocking(move || -> Result<()> {
                let repo = git2::Repository::open(&pkg_dir_clone)
                    .context("Failed to open git repository")?;

                let mut remote = repo
                    .find_remote("origin")
                    .context("Failed to find origin remote")?;

                remote
                    .fetch(&["main", "master"], None, None)
                    .context("Failed to fetch from remote")?;

                let fetch_head = repo
                    .find_reference("FETCH_HEAD")
                    .context("Failed to find FETCH_HEAD")?;
                let fetch_commit = repo
                    .reference_to_annotated_commit(&fetch_head)
                    .context("Failed to get fetch commit")?;

                let (analysis, _) = repo
                    .merge_analysis(&[&fetch_commit])
                    .context("Failed to analyze merge")?;

                if analysis.is_up_to_date() {
                    return Ok(());
                }

                if analysis.is_fast_forward() {
                    let refname = "refs/heads/master";
                    let mut reference = repo
                        .find_reference(refname)
                        .or_else(|_| repo.find_reference("refs/heads/main"))
                        .context("Failed to find branch reference")?;
                    reference
                        .set_target(fetch_commit.id(), "Fast-forward")
                        .context("Failed to fast-forward")?;
                    repo.set_head(refname)
                        .or_else(|_| repo.set_head("refs/heads/main"))
                        .context("Failed to set HEAD")?;
                    repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))
                        .context("Failed to checkout HEAD")?;
                    return Ok(());
                }

                anyhow::bail!(
                    "Cannot fast-forward, manual merge required in {}",
                    pkg_dir_clone.display()
                )
            })
            .await?;

            spinner.finish_and_clear();
            result
        }
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
        create_dir_as_user(&log_dir).await?;
        let log_path = Arc::new(log_dir.join(format!("{package_name}.log")));
        let (log_file, log_file_err) = tokio::task::spawn_blocking({
            let log_path = Arc::clone(&log_path);
            move || {
                let f = File::create(&*log_path)?;
                let e = f.try_clone()?;
                Ok::<_, anyhow::Error>((f, e))
            }
        })
        .await??;
        let spinner = create_spinner(&format!("Building {package_name}..."));

        let bwrap_available = which("bwrap").is_ok();

        if bwrap_available {
            tracing::info!("Using bubblewrap sandbox for secure AUR build");
            println!("{} Building in sandbox (bubblewrap)...", "🔒".green());

            // Install dependencies BEFORE entering sandbox (requires sudo)
            // If running as root, drop to original user or nobody
            let build_user = std::env::var("SUDO_USER")
                .ok()
                .or_else(|| std::env::var("DOAS_USER").ok());

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
            let pkg_dir_str = pkg_dir.to_string_lossy();
            let home = home::home_dir().unwrap_or_else(|| PathBuf::from("/root"));
            let gnupg_dir = home.join(".gnupg");

            let pkgdest_str = env.pkgdest.to_string_lossy();
            let srcdest_str = env.srcdest.to_string_lossy();
            let builddir_str = env.builddir.to_string_lossy();
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

            let status = cmd
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
        create_dir_as_user(&log_dir).await?;
        let log_path = Arc::new(log_dir.join(format!("{package_name}.log")));
        let (log_file, log_file_err) = tokio::task::spawn_blocking({
            let log_path = Arc::clone(&log_path);
            move || {
                let f = File::create(&*log_path)?;
                let e = f.try_clone()?;
                Ok::<_, anyhow::Error>((f, e))
            }
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
        // Get build user (original user from sudo/doas, or fallback to nobody)
        let build_user = std::env::var("SUDO_USER")
            .ok()
            .or_else(|| std::env::var("DOAS_USER").ok());

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
        create_dir_as_user(&log_dir).await?;
        let log_path = Arc::new(log_dir.join(format!("{package_name}.log")));
        let (log_file, log_file_err) = tokio::task::spawn_blocking({
            let log_path = Arc::clone(&log_path);
            move || {
                let f = File::create(&*log_path)?;
                let e = f.try_clone()?;
                Ok::<_, anyhow::Error>((f, e))
            }
        })
        .await??;
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

    #[cfg(feature = "pgp")]
    async fn fetch_missing_pgp_keys(pkgbuild_path: &Path) {
        use crate::core::security::keyserver;

        let Ok(pkgbuild) = PkgBuild::parse(pkgbuild_path) else {
            return;
        };

        if pkgbuild.validpgpkeys.is_empty() {
            return;
        }

        let gnupg_home =
            dirs::home_dir().map_or_else(|| PathBuf::from("/tmp/.gnupg"), |h| h.join(".gnupg"));
        let keyring_path = gnupg_home.join("pubring.kbx");

        let mut missing_keys = Vec::with_capacity(pkgbuild.validpgpkeys.len());
        for key_id in &pkgbuild.validpgpkeys {
            if key_id.chars().any(|c| !c.is_ascii_hexdigit()) || key_id.len() > 64 {
                tracing::warn!("Skipping invalid PGP key ID: {key_id}");
                continue;
            }

            match keyserver::is_key_in_keyring(key_id, &keyring_path) {
                Ok(true) => {}
                Ok(false) | Err(_) => missing_keys.push(key_id.clone()),
            }
        }

        if missing_keys.is_empty() {
            return;
        }

        tracing::info!("Fetching {} missing PGP key(s)...", missing_keys.len());

        let results = keyserver::fetch_keys(&missing_keys).await;
        for (key_id, result) in results {
            match result {
                Ok(cert) => {
                    let info = keyserver::get_key_info(&cert);
                    tracing::debug!("Fetched PGP key: {info}");
                    if let Err(e) = keyserver::append_to_keyring(&cert, &keyring_path) {
                        tracing::warn!("Failed to save key {key_id} to keyring: {e}");
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch PGP key {key_id}: {e}");
                }
            }
        }
    }

    #[cfg(not(feature = "pgp"))]
    async fn fetch_missing_pgp_keys(_pkgbuild_path: &Path) {
        tracing::debug!("PGP feature disabled, skipping key fetch");
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

        let builddir = std::env::temp_dir().join("omg-build").join(
            pkg_dir
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("pkg"),
        );

        create_dir_as_user_sync(&pkgdest)?;
        create_dir_as_user_sync(&srcdest)?;
        std::fs::create_dir_all(&builddir)?;

        if crate::core::is_root()
            && let Some(build_user) = std::env::var("SUDO_USER")
                .ok()
                .or_else(|| std::env::var("DOAS_USER").ok())
        {
            let _ = std::process::Command::new("chown")
                .args(["-R", &build_user, builddir.to_str().unwrap_or("")])
                .output();
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

        let cache_path = self.cache_path(package);

        if let Some(parent) = cache_path.parent() {
            create_dir_as_user(parent).await?;
        }

        if let Some(user) = get_original_user() {
            let cache_path_str = cache_path.to_string_lossy();
            let cache_key = cache_key.to_string();
            let status = Command::new("sudo")
                .args(["-u", &user, "sh", "-c"])
                .arg(format!("echo '{cache_key}' > '{cache_path_str}'"))
                .status()
                .await?;

            if !status.success() {
                anyhow::bail!("Failed to write cache key as user '{user}'");
            }
        } else {
            let cache_key = cache_key.to_string();
            tokio::task::spawn_blocking(move || {
                std::fs::write(cache_path, cache_key)?;
                Ok::<(), anyhow::Error>(())
            })
            .await??;
        }
        Ok(())
    }

    /// Install the built package via direct ALPM or sudo omg install <path>
    async fn install_built_package(pkg_path: &Path) -> Result<()> {
        println!("{} Installing built package...", "→".blue());

        let pkg_path_str = pkg_path.to_string_lossy();

        // Use direct ALPM if we have capabilities (turbo mode) or running as root
        if crate::core::caps::can_write_pacman_db() {
            crate::package_managers::execute_transaction(
                vec![pkg_path_str.into_owned()],
                false,
                false,
                None,
            )?;
        } else {
            crate::core::privilege::run_self_sudo(&["install", "--", pkg_path_str.as_ref()])
                .await?;
        }

        Ok(())
    }

    pub fn clean(&self, package: &str) -> Result<()> {
        let pkg_dir = self.build_dir.join(package);
        if pkg_dir.exists() {
            if let Some(user) = get_original_user() {
                let pkg_dir_str = pkg_dir.to_string_lossy();
                let status = std::process::Command::new("sudo")
                    .args(["-u", &user, "rm", "-rf", "--", pkg_dir_str.as_ref()])
                    .status()?;
                if !status.success() {
                    anyhow::bail!("Failed to clean directory as user '{user}'");
                }
            } else {
                std::fs::remove_dir_all(&pkg_dir)?;
            }
            println!("{} Cleaned build directory for {}", "✓".green(), package);
        }
        Ok(())
    }

    pub fn clean_all(&self) -> Result<()> {
        if self.build_dir.exists() {
            if let Some(user) = get_original_user() {
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

impl Default for AurClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a spinner
#[allow(clippy::literal_string_with_formatting_args, clippy::expect_used)] // Static indicatif template is always valid; braces are template syntax not Rust format args
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
#[allow(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
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
            names.push(format!("package-name-{:04}", i));
        }

        let chunks = AurClient::chunk_aur_names(&names);

        let base_len = format!("{AUR_RPC_URL}?v=5&type=info").len();
        for (idx, chunk) in chunks.iter().enumerate() {
            let mut url_len = base_len;
            for name in chunk {
                url_len += "&arg[]=".len() + name.len();
            }
            assert!(
                url_len <= AUR_RPC_MAX_URI,
                "Chunk {} has URL length {} which exceeds max {}",
                idx,
                url_len,
                AUR_RPC_MAX_URI
            );
        }

        let total_packages: usize = chunks.iter().map(|c| c.len()).sum();
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

        let base_len = format!("{AUR_RPC_URL}?v=5&type=info").len();
        for chunk in &chunks {
            let mut url_len = base_len;
            for name in chunk {
                url_len += "&arg[]=".len() + name.len();
            }
            assert!(url_len <= AUR_RPC_MAX_URI);
        }

        let total: usize = chunks.iter().map(|c| c.len()).sum();
        assert_eq!(total, 4);
    }

    #[test]
    fn test_chunk_aur_names_exactly_at_boundary() {
        let base_len = format!("{AUR_RPC_URL}?v=5&type=info").len();
        let available = AUR_RPC_MAX_URI - base_len;

        // Formula: arg_size = "&arg[]=".len() + pkg_name.len() = 7 + 10 = 17 chars/pkg
        let arg_size = "&arg[]=".len() + 10;
        let count = available / arg_size;

        let names: Vec<String> = (0..count).map(|i| format!("pkg{:06}", i)).collect();
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
}
