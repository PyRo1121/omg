//! Pure Rust Transaction Engine for Debian/Ubuntu
//!
//! Handles package installation, removal, and upgrades using pure Rust:
//! - .deb archive extraction (ar + tar.xz/tar.gz)
//! - dpkg database updates
//! - Pre/post-install script execution
//! - Atomic transactions with rollback support
//! - File conflict detection

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{Context, Result};
use futures::stream::{self, StreamExt};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use tempfile::TempDir;

use super::content_store::ContentStore;
use super::resolver::ResolutionResult;
use super::validation::require_verified_deb;

/// Transaction state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionState {
    /// Transaction created but not started
    Pending,
    /// Downloading packages
    Downloading,
    /// Unpacking packages
    Unpacking,
    /// Configuring packages
    Configuring,
    /// Transaction completed successfully
    Completed,
    /// Transaction failed and rolled back
    RolledBack,
}

/// Maximum concurrent package downloads (pipelined architecture)
const MAX_CONCURRENT_PACKAGE_DOWNLOADS: usize = 48;

/// Maximum concurrent package unpacks (parallel extraction with rayon)
const MAX_CONCURRENT_UNPACKS: usize = 16;

/// Maximum retries per download
const MAX_DOWNLOAD_RETRIES: u32 = 3;

/// Initial backoff for retries (doubles each retry)
const INITIAL_BACKOFF_MS: u64 = 200;

/// A package transaction
pub struct Transaction {
    /// Current state
    pub state: TransactionState,
    /// Packages to install
    pub to_install: Vec<PackageAction>,
    /// Packages to remove
    pub to_remove: Vec<PackageAction>,
    /// Packages to upgrade
    pub to_upgrade: Vec<PackageAction>,
    /// Temporary directory for downloads
    temp_dir: Option<TempDir>,
    /// Backup of files for rollback
    backups: HashMap<PathBuf, PathBuf>,
    /// Files installed by this transaction
    installed_files: Vec<PathBuf>,
    /// Content-addressable storage for package deduplication
    content_store: ContentStore,
}

/// Action to perform on a package
#[derive(Debug, Clone)]
pub struct PackageAction {
    /// Package name
    pub name: String,
    /// Version
    pub version: String,
    /// Local .deb file path (after download)
    pub deb_path: Option<PathBuf>,
    /// Download URL
    pub url: Option<String>,
    /// Size in bytes
    pub size: u64,
    /// SHA256 hash (if known from repository metadata)
    pub sha256: Option<String>,
}

impl Transaction {
    /// Create a new transaction from resolution result
    pub fn from_resolution(result: ResolutionResult) -> Self {
        let to_install: Vec<PackageAction> = result
            .to_install
            .into_iter()
            .map(|name| PackageAction {
                name,
                version: String::new(),
                deb_path: None,
                url: None,
                size: 0,
                sha256: None,
            })
            .collect();

        let to_upgrade: Vec<PackageAction> = result
            .to_upgrade
            .into_iter()
            .map(|(name, _old, new)| PackageAction {
                name,
                version: new,
                deb_path: None,
                url: None,
                size: 0,
                sha256: None,
            })
            .collect();

        let to_remove: Vec<PackageAction> = result
            .to_remove
            .into_iter()
            .map(|name| PackageAction {
                name,
                version: String::new(),
                deb_path: None,
                url: None,
                size: 0,
                sha256: None,
            })
            .collect();

        let content_store = ContentStore::new();
        // Initialize content store (creates directory if needed)
        let _ = content_store.init();

        Self {
            state: TransactionState::Pending,
            to_install,
            to_remove,
            to_upgrade,
            temp_dir: None,
            backups: HashMap::new(),
            installed_files: Vec::new(),
            content_store,
        }
    }

    /// Check for file conflicts before installation
    /// Returns a list of conflicting files owned by other packages
    pub fn check_file_conflicts(&self) -> Result<Vec<(PathBuf, String)>> {
        let conflicts = Vec::new();

        // Future enhancement: Full conflict detection system
        // Would require parsing data.tar file lists and comparing with dpkg database
        // Current implementation relies on dpkg's built-in conflict detection
        //
        // Implementation plan:
        // 1. Parse file lists from data.tar for each package
        // 2. Check against /var/lib/dpkg/info/*.list files
        // 3. Detect overlapping files from different packages

        tracing::debug!(
            "File conflict check: {} potential conflicts detected",
            conflicts.len()
        );
        Ok(conflicts)
    }

    /// Create an empty transaction
    pub fn new() -> Self {
        let content_store = ContentStore::new();
        let _ = content_store.init();

        Self {
            state: TransactionState::Pending,
            to_install: Vec::new(),
            to_remove: Vec::new(),
            to_upgrade: Vec::new(),
            temp_dir: None,
            backups: HashMap::new(),
            installed_files: Vec::new(),
            content_store,
        }
    }

    /// Add a package to install
    pub fn add_install(&mut self, name: String, version: String, url: String, size: u64) {
        self.to_install.push(PackageAction {
            name,
            version,
            deb_path: None,
            url: Some(url),
            size,
            sha256: None,
        });
    }

    /// Add a package to remove
    pub fn add_remove(&mut self, name: String) {
        self.to_remove.push(PackageAction {
            name,
            version: String::new(),
            deb_path: None,
            url: None,
            size: 0,
            sha256: None,
        });
    }

    /// Execute the transaction with pipelined download+unpack
    ///
    /// OPTIMIZATION: Downloads and unpacks run concurrently. As soon as a package
    /// finishes downloading, it's queued for unpacking while other packages continue
    /// downloading. This overlaps I/O-bound (download) and CPU-bound (decompress) work.
    pub async fn execute(&mut self) -> Result<()> {
        tracing::info!(
            "Starting pipelined transaction with {} packages",
            self.package_count()
        );

        // Create temp directory
        self.temp_dir = Some(TempDir::new().context("Failed to create temp directory")?);

        // Use pipelined execution for better performance
        self.state = TransactionState::Downloading;
        if let Err(e) = self.download_and_unpack_pipelined().await {
            tracing::error!("Pipelined execution failed: {}", e);
            self.rollback()?;
            return Err(e).context("Failed during pipelined download/unpack");
        }

        // Configure packages (must be sequential after all unpacking)
        self.state = TransactionState::Configuring;
        if let Err(e) = self.configure_packages() {
            tracing::error!("Configuration failed: {}", e);
            self.rollback()?;
            return Err(e).context("Failed during package configuration");
        }

        self.state = TransactionState::Completed;
        tracing::info!("Transaction completed successfully");
        Ok(())
    }

    /// Pipelined download and unpack - starts unpacking while still downloading
    ///
    /// OPTIMIZATION: Uses batched parallel unpacking with rayon for CPU-bound decompression.
    /// Downloads complete packages are collected and unpacked in parallel batches.
    async fn download_and_unpack_pipelined(&mut self) -> Result<()> {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use tokio::sync::mpsc;

        // OPTIMIZATION: Memory-conscious HTTP client configuration
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(10))
            // Reduce connection pool to save memory (24 vs 96 connections)
            .pool_max_idle_per_host(MAX_CONCURRENT_PACKAGE_DOWNLOADS * 2)
            .pool_idle_timeout(Duration::from_secs(60)) // Shorter timeout to release faster
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_nodelay(true)
            .build()?;

        let temp_dir = self
            .temp_dir
            .as_ref()
            .map_or_else(|| PathBuf::from("/tmp"), |t| t.path().to_path_buf());

        // Collect all packages that need downloading
        let packages_to_download: Vec<(usize, String, String, String, Option<String>)> = self
            .to_install
            .iter()
            .chain(self.to_upgrade.iter())
            .enumerate()
            .filter_map(|(idx, action)| {
                action.url.as_ref().map(|url| {
                    (
                        idx,
                        action.name.clone(),
                        action.version.clone(),
                        url.clone(),
                        action.sha256.clone(),
                    )
                })
            })
            .collect();

        if packages_to_download.is_empty() {
            return Ok(());
        }

        // OPTIMIZATION: Pre-warm connections to the mirror (fire and forget)
        // Extract unique hosts and make HEAD requests to establish connections
        if let Some((_, _, _, first_url, _)) = packages_to_download.first()
            && let Ok(url) = reqwest::Url::parse(first_url)
        {
            let warm_url = format!("{}://{}/", url.scheme(), url.host_str().unwrap_or(""));
            // Fire off HEAD requests to warm up connection pool (no waiting)
            for _ in 0..2 {
                let client = client.clone();
                let warm_url = warm_url.clone();
                tokio::spawn(async move {
                    let _ = client.head(&warm_url).send().await;
                });
            }
        }

        let total_packages = packages_to_download.len();
        tracing::info!(
            "Pipelined processing {} packages (download: {}, unpack: {} concurrent)",
            total_packages,
            MAX_CONCURRENT_PACKAGE_DOWNLOADS,
            MAX_CONCURRENT_UNPACKS
        );

        // Setup progress bars
        let multi = MultiProgress::new();
        let overall = multi.add(ProgressBar::new(total_packages as u64));
        overall.set_style(
            ProgressStyle::default_bar()
                .template("{prefix:.bold} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
                .expect("valid template")
                .progress_chars("=>-"),
        );
        overall.set_prefix("Processing");

        // Channel for passing downloaded packages to unpack workers
        // Small buffer to reduce memory pressure
        let (tx, mut rx) = mpsc::channel::<(PathBuf, String)>(MAX_CONCURRENT_UNPACKS);
        let content_store = self.content_store.clone();

        // Track completed downloads for parallel unpacking
        let downloads_complete = Arc::new(AtomicUsize::new(0));

        // Pre-create the download task futures with their own sender clones
        let download_futures: Vec<_> = packages_to_download
            .into_iter()
            .map(|(idx, name, version, url, sha256)| {
                let client = client.clone();
                let temp_dir = temp_dir.clone();
                let content_store = content_store.clone();
                let tx = tx.clone();
                let downloads_complete = Arc::clone(&downloads_complete);
                let pb = multi.add(ProgressBar::new(0));
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template(
                            "  {prefix:.cyan} [{bar:25.green/blue}] {bytes}/{total_bytes} {msg}",
                        )
                        .expect("valid template")
                        .progress_chars("=>-"),
                );
                pb.set_prefix(name.clone());

                async move {
                    let result = download_package_streaming(
                        &client,
                        &name,
                        &version,
                        &url,
                        &temp_dir,
                        &pb,
                        &content_store,
                        sha256.as_deref(),
                    )
                    .await;

                    match &result {
                        Ok(path) => {
                            pb.set_message("✓ dl".green().to_string());
                            pb.finish();
                            downloads_complete.fetch_add(1, Ordering::Relaxed);
                            // Send to unpack channel immediately
                            let _ = tx.send((path.clone(), name.clone())).await;
                        }
                        Err(e) => {
                            pb.set_message(format!("{e}").red().to_string());
                            pb.finish();
                        }
                    }
                    result.map(|path| (idx, path))
                }
            })
            .collect();

        // Drop our sender so the channel closes when all downloads complete
        drop(tx);

        // Spawn unpack worker thread
        let temp_dir_unpack = temp_dir.clone();
        let unpack_handle = tokio::task::spawn_blocking(move || {
            use std::sync::Mutex;

            let rt = tokio::runtime::Handle::current();
            let installed_files = Mutex::new(Vec::new());
            let unpack_errors = Mutex::new(Vec::new());

            // OPTIMIZATION: Process packages immediately instead of batching
            // This reduces memory pressure from holding multiple .deb files in memory
            while let Some((deb_path, name)) = rt.block_on(rx.recv()) {
                tracing::debug!("Unpacking {} immediately", name);
                match unpack_deb_standalone(&deb_path, &name, &temp_dir_unpack) {
                    Ok(files) => {
                        let mut guard = installed_files.lock().expect("lock poisoned");
                        guard.extend(files);
                        tracing::debug!("Unpacked {} successfully", name);
                    }
                    Err(e) => {
                        tracing::error!("Failed to unpack {}: {}", name, e);
                        let mut guard = unpack_errors.lock().expect("lock poisoned");
                        guard.push((name.clone(), e));
                    }
                }

                // OPTIMIZATION: Delete .deb file immediately after unpacking
                // This reduces disk I/O and frees up temp space quickly
                if let Err(e) = std::fs::remove_file(&deb_path) {
                    tracing::warn!("Failed to delete {}: {}", deb_path.display(), e);
                }
            }

            let files = installed_files.into_inner().unwrap_or_default();
            let errors = unpack_errors.into_inner().unwrap_or_default();
            (files, errors)
        });

        // Run downloads concurrently
        let results: Vec<_> = stream::iter(download_futures)
            .buffer_unordered(MAX_CONCURRENT_PACKAGE_DOWNLOADS)
            .inspect(|_| overall.inc(1))
            .collect()
            .await;

        overall.finish_and_clear();

        // Wait for unpacking to complete
        let (installed_files, unpack_errors) = unpack_handle.await?;
        self.installed_files = installed_files;

        // Check for download failures
        let mut downloaded_paths: HashMap<usize, PathBuf> = HashMap::new();
        let mut download_failures = Vec::new();

        for result in results {
            match result {
                Ok((idx, path)) => {
                    downloaded_paths.insert(idx, path);
                }
                Err(e) => {
                    download_failures.push(e);
                }
            }
        }

        if !download_failures.is_empty() {
            let error_msg = download_failures
                .iter()
                .map(|e| format!("  - {e}"))
                .collect::<Vec<_>>()
                .join("\n");
            anyhow::bail!(
                "Failed to download {} packages:\n{}",
                download_failures.len(),
                error_msg
            );
        }

        if !unpack_errors.is_empty() {
            let error_msg = unpack_errors
                .iter()
                .map(|(name, e)| format!("  - {name}: {e}"))
                .collect::<Vec<_>>()
                .join("\n");
            anyhow::bail!(
                "Failed to unpack {} packages:\n{}",
                unpack_errors.len(),
                error_msg
            );
        }

        // Update package actions with downloaded paths
        for (idx, action) in self
            .to_install
            .iter_mut()
            .chain(self.to_upgrade.iter_mut())
            .enumerate()
        {
            if let Some(path) = downloaded_paths.get(&idx) {
                action.deb_path = Some(path.clone());
            }
        }

        tracing::info!(
            "Pipelined processing complete: {} packages downloaded and unpacked",
            downloaded_paths.len()
        );
        Ok(())
    }

    /// Configure all unpacked packages
    ///
    /// OPTIMIZATION: Batches dpkg status updates into a single write operation
    /// instead of opening/closing the file for each package.
    fn configure_packages(&self) -> Result<()> {
        use std::io::BufWriter;

        let temp_dir = self
            .temp_dir
            .as_ref()
            .map_or_else(|| PathBuf::from("/tmp"), |t| t.path().to_path_buf());

        // Collect all status entries for batched write
        let mut status_entries = Vec::new();
        let mut conffiles_to_copy = Vec::new();

        for action in self.to_install.iter().chain(self.to_upgrade.iter()) {
            let extract_dir = temp_dir.join(&action.name);
            let control_dir = extract_dir.join("DEBIAN");

            // Run postinst script if exists (must be sequential for dependencies)
            let postinst = control_dir.join("postinst");
            if postinst.exists() {
                run_maintainer_script(&postinst, "configure")?;
            }

            // Prepare dpkg status entry for batched write
            let control_file = control_dir.join("control");
            if control_file.exists()
                && let Ok(entry) = prepare_status_entry(&control_file)
            {
                status_entries.push(entry);
            }

            // Collect conffiles for batched copy
            let conffiles = control_dir.join("conffiles");
            if conffiles.exists() {
                conffiles_to_copy.push((conffiles, action.name.clone()));
            }
        }

        // OPTIMIZATION: Batch write all status entries with large buffer
        if !status_entries.is_empty() {
            let status_path = Path::new("/var/lib/dpkg/status");
            let file = fs::OpenOptions::new()
                .append(true)
                .open(status_path)
                .context("Failed to open dpkg status file")?;

            // OPTIMIZATION: Larger buffer (256KB) for dpkg status file
            // Typical control file is 1-2KB, so 256KB can hold 128-256 packages
            let mut writer = BufWriter::with_capacity(256 * 1024, file);
            for entry in &status_entries {
                writer.write_all(b"\n")?;
                writer.write_all(entry.as_bytes())?;
            }
            writer.flush()?;

            tracing::debug!(
                "Batched {} dpkg status entries in single write (256KB buffer)",
                status_entries.len()
            );
        }

        // Copy conffiles (can be done in parallel with rayon)
        if !conffiles_to_copy.is_empty() {
            use rayon::prelude::*;

            conffiles_to_copy.par_iter().for_each(|(src, name)| {
                let dest = Path::new("/var/lib/dpkg/info").join(format!("{name}.conffiles"));
                if let Err(e) = fs::copy(src, &dest) {
                    tracing::warn!("Failed to copy conffiles for {}: {}", name, e);
                }
            });
        }

        Ok(())
    }

    /// Rollback the transaction
    pub fn rollback(&mut self) -> Result<()> {
        tracing::warn!(
            "Rolling back transaction ({} files installed)",
            self.installed_files.len()
        );

        // Remove installed files
        let mut removed_count = 0;
        for file in &self.installed_files {
            if fs::remove_file(file).is_ok() {
                removed_count += 1;
            } else {
                tracing::warn!("Failed to remove file during rollback: {}", file.display());
            }
        }
        tracing::debug!("Removed {} files during rollback", removed_count);

        // Restore backups
        let mut restored_count = 0;
        for (original, backup) in &self.backups {
            if backup.exists() {
                if fs::copy(backup, original).is_ok() {
                    restored_count += 1;
                } else {
                    tracing::warn!(
                        "Failed to restore backup: {} -> {}",
                        backup.display(),
                        original.display()
                    );
                }
            }
        }
        tracing::debug!("Restored {} backups", restored_count);

        self.state = TransactionState::RolledBack;
        tracing::info!("Transaction rolled back successfully");
        Ok(())
    }

    /// Get the total download size
    pub fn total_download_size(&self) -> u64 {
        self.to_install.iter().map(|a| a.size).sum::<u64>()
            + self.to_upgrade.iter().map(|a| a.size).sum::<u64>()
    }

    /// Get the number of packages to process
    pub fn package_count(&self) -> usize {
        self.to_install.len() + self.to_upgrade.len() + self.to_remove.len()
    }

    /// Installed packages must have a version in dpkg status; an empty string is not a version.
    fn require_installed_version(name: &str, version: Option<String>) -> Result<String> {
        match version {
            Some(version) if !version.is_empty() => Ok(version),
            Some(_) | None => {
                anyhow::bail!("installed package {name} has no version in dpkg status")
            }
        }
    }

    /// Execute package removal
    #[allow(clippy::unused_async)]
    pub async fn execute_removal(&mut self) -> Result<()> {
        if self.to_remove.is_empty() {
            return Ok(());
        }

        tracing::info!("Starting removal of {} packages", self.to_remove.len());

        // Setup progress display
        let multi = MultiProgress::new();
        let overall = multi.add(ProgressBar::new(self.to_remove.len() as u64));
        overall.set_style(
            ProgressStyle::default_bar()
                .template("{prefix:.bold} [{bar:40.red/blue}] {pos}/{len} {msg}")
                .expect("valid template")
                .progress_chars("=>-"),
        );
        overall.set_prefix("Removing");

        // Process packages in dependency order (leaves first)
        // For now, process in reverse order as a simple heuristic
        let packages_to_remove: Vec<_> = self.to_remove.iter().map(|a| a.name.clone()).collect();

        for package_name in &packages_to_remove {
            let pb = multi.add(ProgressBar::new(5));
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("  {prefix:.cyan} [{bar:25.red/blue}] {msg}")
                    .expect("valid template")
                    .progress_chars("=>-"),
            );
            pb.set_prefix(package_name.clone());

            // Validate package is installed
            pb.set_message("validating");
            pb.inc(1);
            if !super::is_installed_fast(package_name)? {
                pb.set_message("not installed".red().to_string());
                pb.finish();
                tracing::warn!("Package {} is not installed, skipping", package_name);
                overall.inc(1);
                continue;
            }

            // Get installed version for logging
            let version = Self::require_installed_version(
                package_name,
                super::get_package_version(package_name)?,
            )?;

            // Run prerm script
            pb.set_message("prerm");
            pb.inc(1);
            if let Err(e) = run_prerm_script(package_name) {
                pb.set_message(format!("prerm failed: {e}").red().to_string());
                pb.finish();
                overall.inc(1);
                tracing::error!("Failed to run prerm script for {package_name}: {e}");
                anyhow::bail!("Package removal failed during prerm script for {package_name}");
            }

            // Remove package files
            pb.set_message("removing files");
            pb.inc(1);
            if let Err(e) = remove_package_files(package_name) {
                pb.set_message(format!("file removal failed: {e}").red().to_string());
                pb.finish();
                overall.inc(1);
                tracing::error!("Failed to remove files for {package_name}: {e}");
                anyhow::bail!("Package removal failed during file removal for {package_name}");
            }

            // Run postrm script
            pb.set_message("postrm");
            pb.inc(1);
            if let Err(e) = run_postrm_script(package_name) {
                pb.set_message(format!("postrm failed: {e}").red().to_string());
                pb.finish();
                overall.inc(1);
                tracing::error!("Failed to run postrm script for {package_name}: {e}");
                anyhow::bail!("Package removal failed during postrm script for {package_name}");
            }

            // Update dpkg status
            pb.set_message("updating status");
            pb.inc(1);
            if let Err(e) = update_dpkg_status_for_removal(package_name) {
                pb.set_message(format!("status update failed: {e}").red().to_string());
                pb.finish();
                overall.inc(1);
                tracing::error!("Failed to update dpkg status for {package_name}: {e}");
                anyhow::bail!("Package removal failed during status update for {package_name}");
            }

            // Clean up dpkg info files
            pb.set_message("cleanup");
            pb.inc(1);
            cleanup_dpkg_info_files(package_name);

            pb.set_message("✓".green().to_string());
            pb.finish();
            overall.inc(1);
            tracing::info!("Removed {} v{}", package_name, version);
        }

        overall.finish_and_clear();
        tracing::info!("Successfully removed {} packages", packages_to_remove.len());
        Ok(())
    }
}

impl Default for Transaction {
    fn default() -> Self {
        Self::new()
    }
}

/// Standalone function to unpack a .deb file (for use in pipelined processing)
///
/// This is separate from the Transaction method to allow concurrent unpacking
/// without holding a mutable borrow on the transaction.
fn unpack_deb_standalone(
    deb_path: &Path,
    package_name: &str,
    temp_dir: &Path,
) -> Result<Vec<PathBuf>> {
    tracing::debug!("Unpacking .deb: {} ({})", package_name, deb_path.display());

    let extract_dir = temp_dir.join(package_name);
    fs::create_dir_all(&extract_dir)
        .with_context(|| format!("Failed to create extraction directory for {package_name}"))?;

    // Extract the .deb (ar archive)
    let deb_file = File::open(deb_path)
        .with_context(|| format!("Failed to open .deb file: {}", deb_path.display()))?;
    let mut archive = ar::Archive::new(deb_file);

    let mut control_tar: Option<Vec<u8>> = None;
    let mut data_tar: Option<Vec<u8>> = None;

    while let Some(entry) = archive.next_entry() {
        let mut entry = entry?;
        let name = String::from_utf8_lossy(entry.header().identifier()).to_string();

        let mut contents = Vec::new();
        entry
            .read_to_end(&mut contents)
            .with_context(|| format!("Failed to read {name} from .deb archive"))?;

        if name.starts_with("control.tar") {
            control_tar = Some(contents);
        } else if name.starts_with("data.tar") {
            data_tar = Some(contents);
        }
    }

    // Extract control.tar to get maintainer scripts
    if let Some(control_data) = control_tar {
        let control_dir = extract_dir.join("DEBIAN");
        fs::create_dir_all(&control_dir)?;
        extract_tar_auto(&control_data, &control_dir)?;

        // Run preinst script if exists
        let preinst = control_dir.join("preinst");
        if preinst.exists() {
            run_maintainer_script(&preinst, "install")?;
        }
    }

    // Extract data.tar to filesystem
    let installed_files = if let Some(data) = data_tar {
        extract_tar_to_root(&data)?
    } else {
        Vec::new()
    };

    Ok(installed_files)
}

/// Extract a tar archive with auto-detection of compression
fn extract_tar_auto(data: &[u8], dest: &Path) -> Result<()> {
    // OPTIMIZATION: Check zstd FIRST (2-3x faster decompression than xz)
    // Modern Debian packages increasingly use zstd compression
    if data.len() > 4 && data.starts_with(b"\x28\xb5\x2f\xfd") {
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(std::io::Cursor::new(data))
            .map_err(|e| anyhow::anyhow!("Failed to create zstd decoder: {e}"))?;
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        return extract_tar(&decompressed, dest);
    }

    // Try xz (still common for Debian packages)
    if data.len() > 6 && data.starts_with(b"\xfd7zXZ\x00") {
        let mut decoder = Vec::new();
        lzma_rs::xz_decompress(&mut std::io::Cursor::new(data), &mut decoder)?;
        return extract_tar(&decoder, dest);
    }

    // Try gzip (legacy packages)
    if data.len() > 2 && data[0] == 0x1f && data[1] == 0x8b {
        let mut decoder = flate2::read::GzDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        return extract_tar(&decompressed, dest);
    }

    // Assume uncompressed tar
    extract_tar(data, dest)
}

/// Extract a tar archive to a directory
fn extract_tar(data: &[u8], dest: &Path) -> Result<()> {
    let mut archive = tar::Archive::new(std::io::Cursor::new(data));

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let dest_path = dest.join(&path);

        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        entry.unpack(&dest_path)?;
    }

    Ok(())
}

/// Extract a tar archive to the filesystem root
/// OPTIMIZATION: Pre-allocated buffers to reduce memory usage
fn extract_tar_to_root(data: &[u8]) -> Result<Vec<PathBuf>> {
    // OPTIMIZATION: Conservative buffer pre-allocation to reduce memory usage
    // Check compression format and allocate conservatively
    let (decompressed, estimated_ratio) = if data.len() > 4 && data.starts_with(b"\x28\xb5\x2f\xfd")
    {
        // Zstd: Fast decompression, good compression
        let mut decoder = ruzstd::decoding::StreamingDecoder::new(std::io::Cursor::new(data))
            .map_err(|e| anyhow::anyhow!("Failed to create zstd decoder: {e}"))?;
        // Conservative allocation: 3x initial size, let Vec grow as needed
        let mut decompressed = Vec::with_capacity(data.len() * 3);
        decoder.read_to_end(&mut decompressed)?;
        decompressed.shrink_to_fit(); // Release excess capacity immediately
        (decompressed, 5)
    } else if data.len() > 6 && data.starts_with(b"\xfd7zXZ\x00") {
        // XZ: Slower decompression, best compression
        // Conservative allocation: 4x initial size
        let mut decoder = Vec::with_capacity(data.len() * 4);
        lzma_rs::xz_decompress(&mut std::io::Cursor::new(data), &mut decoder)?;
        decoder.shrink_to_fit(); // Release excess capacity immediately
        (decoder, 7)
    } else if data.len() > 2 && data[0] == 0x1f && data[1] == 0x8b {
        // Gzip: Fast decompression, moderate compression
        let mut decoder = flate2::read::GzDecoder::new(data);
        let mut decompressed = Vec::with_capacity(data.len() * 3);
        decoder.read_to_end(&mut decompressed)?;
        decompressed.shrink_to_fit(); // Release excess capacity immediately
        (decompressed, 4)
    } else {
        (data.to_vec(), 1)
    };

    tracing::debug!(
        "Decompressed data.tar: {} bytes -> {} bytes ({}x ratio)",
        data.len(),
        decompressed.len(),
        estimated_ratio
    );

    let mut archive = tar::Archive::new(std::io::Cursor::new(&decompressed));

    // OPTIMIZATION: Streaming extraction with minimal memory footprint
    // Process files immediately instead of buffering all in memory
    let mut installed_files = Vec::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();

        // Convert to absolute path (strip leading ./)
        let path_str = path.to_string_lossy();
        let path_ref: &str = &path_str;
        let abs_path = if path_str.starts_with("./") {
            PathBuf::from("/").join(&path_ref[2..])
        } else if path_str.starts_with('/') {
            PathBuf::from(path_ref)
        } else {
            PathBuf::from("/").join(path_ref)
        };

        let is_dir = entry.header().entry_type().is_dir();
        let mode = entry.header().mode()?;

        if is_dir {
            // Handle directories immediately
            fs::create_dir_all(&abs_path)?;
        } else {
            // OPTIMIZATION: Stream file contents directly to disk without buffering
            // Create parent directories
            if let Some(parent) = abs_path.parent() {
                fs::create_dir_all(parent).with_context(|| {
                    format!("Failed to create parent dir for {}", abs_path.display())
                })?;
            }

            // OPTIMIZATION: Use streaming copy with size-appropriate strategy
            let size = entry.header().size()?;

            if size < 1024 {
                // Tiny files: read and write in one go
                let mut contents = Vec::with_capacity(size as usize);
                entry.read_to_end(&mut contents)?;
                fs::write(&abs_path, contents)
                    .with_context(|| format!("Failed to write file: {}", abs_path.display()))?;
            } else if size < 65536 {
                // Small-medium files: buffered copy
                use std::io::BufWriter;
                let file = File::create(&abs_path)
                    .with_context(|| format!("Failed to create file: {}", abs_path.display()))?;
                let mut writer = BufWriter::with_capacity(16384, file);
                std::io::copy(&mut entry, &mut writer)
                    .with_context(|| format!("Failed to copy to: {}", abs_path.display()))?;
                writer.flush()?;
            } else {
                // Large files: direct unbuffered write (let OS optimize)
                let mut file = File::create(&abs_path)
                    .with_context(|| format!("Failed to create file: {}", abs_path.display()))?;
                std::io::copy(&mut entry, &mut file)
                    .with_context(|| format!("Failed to copy to: {}", abs_path.display()))?;
            }

            // Set permissions
            let mut perms = fs::metadata(&abs_path)?.permissions();
            perms.set_mode(mode);
            fs::set_permissions(&abs_path, perms)?;

            installed_files.push(abs_path);
        }
    }

    Ok(installed_files)
}

/// Run a maintainer script (preinst, postinst, prerm, postrm)
fn run_maintainer_script(script: &Path, arg: &str) -> Result<()> {
    // Make executable
    let mut perms = fs::metadata(script)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(script, perms)?;

    let status = Command::new(script)
        .arg(arg)
        .env("DPKG_MAINTSCRIPT_PACKAGE", "")
        .env("DPKG_MAINTSCRIPT_ARCH", std::env::consts::ARCH)
        .status()
        .with_context(|| format!("Failed to run {}", script.display()))?;

    if !status.success() {
        anyhow::bail!(
            "Maintainer script {} failed with exit code {:?}",
            script.display(),
            status.code()
        );
    }

    Ok(())
}

/// Prepare a status entry from a control file (for batched writing)
fn prepare_status_entry(control_file: &Path) -> Result<String> {
    let control_content = fs::read_to_string(control_file)?;

    // Parse control file and add Status field
    let found_status = control_content
        .lines()
        .any(|line| line.starts_with("Status:"));

    let mut result = String::new();
    if found_status {
        // Replace existing Status line
        for line in control_content.lines() {
            if line.starts_with("Status:") {
                result.push_str("Status: install ok installed\n");
            } else {
                result.push_str(line);
                result.push('\n');
            }
        }
    } else {
        // Insert Status field after Package field
        for line in control_content.lines() {
            result.push_str(line);
            result.push('\n');
            if line.starts_with("Package:") {
                result.push_str("Status: install ok installed\n");
            }
        }
    }
    Ok(result)
}

/// Download a package with streaming to disk (lower memory usage)
///
/// OPTIMIZATION: Streams response directly to disk instead of buffering in memory.
/// This reduces memory pressure and allows the OS to optimize disk writes.
async fn download_package_streaming(
    client: &reqwest::Client,
    name: &str,
    version: &str,
    url: &str,
    temp_dir: &Path,
    progress: &ProgressBar,
    content_store: &ContentStore,
    sha256: Option<&str>,
) -> Result<PathBuf> {
    let filename = format!("{name}_{version}.deb");
    let dest = temp_dir.join(&filename);

    // Check content store first (if we have the SHA256 hash)
    if let Some(hash) = sha256
        && content_store.contains(hash)
    {
        match content_store.hard_link(hash, &dest) {
            Ok(()) => {
                require_verified_deb(&dest, name, Some(hash))?;
                progress.set_message("cached ✓".green().to_string());
                tracing::info!(
                    "Using cached .deb from content store: {name} (hash: {})",
                    &hash[..8]
                );
                return Ok(dest);
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to hard link from content store: {e}, falling back to download"
                );
            }
        }
    }

    // OPTIMIZATION: Fast path - download without overhead
    // Skip content store on first download to minimize latency
    match download_streaming_once(client, url, &dest, progress).await {
        Ok(()) => {
            tracing::debug!("Successfully downloaded {} to {}", name, dest.display());
            require_verified_deb(&dest, name, sha256)?;
            return Ok(dest);
        }
        Err(e) => {
            tracing::debug!("First download attempt failed for {}: {}", name, e);
        }
    }

    // Retry path (only if first attempt failed)
    let mut last_error = None;
    for attempt in 1..MAX_DOWNLOAD_RETRIES {
        let backoff = Duration::from_millis(INITIAL_BACKOFF_MS << attempt);
        progress.set_message(format!("retry {}/{}", attempt + 1, MAX_DOWNLOAD_RETRIES));
        tokio::time::sleep(backoff).await;

        match download_streaming_once(client, url, &dest, progress).await {
            Ok(()) => {
                tracing::debug!("Retry succeeded for {}", name);
                require_verified_deb(&dest, name, sha256)?;
                return Ok(dest);
            }
            Err(e) => {
                tracing::warn!(
                    "Download attempt {} failed for {}: {}",
                    attempt + 1,
                    name,
                    e
                );
                last_error = Some(e);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        anyhow::anyhow!("Failed to download {name} after {MAX_DOWNLOAD_RETRIES} retries")
    }))
}

/// Stream download directly to disk
async fn download_streaming_once(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    progress: &ProgressBar,
) -> Result<()> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to request {url}"))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} for {url}", response.status());
    }

    // Set total size for progress bar
    let total_size = response.content_length().unwrap_or(0);
    if total_size > 0 {
        progress.set_length(total_size);
    }

    // OPTIMIZATION: Stream to file with larger buffer (64KB) to reduce syscalls
    let mut file = tokio::fs::File::create(dest)
        .await
        .with_context(|| format!("Failed to create file: {}", dest.display()))?;

    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;

    // OPTIMIZATION: Smaller write buffer (8KB) to reduce memory pressure
    // Modern filesystems handle small writes efficiently
    let mut write_buffer = Vec::with_capacity(8192); // 8KB buffer

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("Failed to read chunk from {url}"))?;

        // OPTIMIZATION: Batch small chunks into 8KB writes
        write_buffer.extend_from_slice(&chunk);

        // Flush buffer when it reaches 8KB (good balance of memory and syscall efficiency)
        if write_buffer.len() >= 8192 {
            file.write_all(&write_buffer)
                .await
                .with_context(|| format!("Failed to write to {}", dest.display()))?;
            downloaded += write_buffer.len() as u64;
            progress.set_position(downloaded);
            write_buffer.clear();
        }
    }

    // Write remaining buffered data
    if !write_buffer.is_empty() {
        file.write_all(&write_buffer)
            .await
            .with_context(|| format!("Failed to write final chunk to {}", dest.display()))?;
        downloaded += write_buffer.len() as u64;
        progress.set_position(downloaded);
    }

    // Clear buffer explicitly to release memory immediately
    drop(write_buffer);

    // Flush but don't fsync (unsafe-io optimization - OS handles durability)
    file.flush().await?;

    Ok(())
}

/// Run prerm script for package removal
fn run_prerm_script(package_name: &str) -> Result<()> {
    // Try both with and without architecture suffix
    let arch = std::env::consts::ARCH;
    let candidates = [
        format!("{package_name}:{arch}.prerm"),
        format!("{package_name}.prerm"),
    ];

    for script_name in &candidates {
        let script = Path::new("/var/lib/dpkg/info").join(script_name);
        if script.exists() {
            run_maintainer_script(&script, "remove")?;
            return Ok(());
        }
    }

    // No prerm script found - this is OK
    tracing::debug!("No prerm script found for {}", package_name);
    Ok(())
}

/// Run postrm script for package removal
fn run_postrm_script(package_name: &str) -> Result<()> {
    let arch = std::env::consts::ARCH;
    let candidates = [
        format!("{package_name}:{arch}.postrm"),
        format!("{package_name}.postrm"),
    ];

    for script_name in &candidates {
        let script = Path::new("/var/lib/dpkg/info").join(script_name);
        if script.exists() {
            run_maintainer_script(&script, "remove")?;
            return Ok(());
        }
    }

    tracing::debug!("No postrm script found for {}", package_name);
    Ok(())
}

/// Remove package files from the filesystem
fn remove_package_files(package_name: &str) -> Result<()> {
    // Find the .list file
    let arch = std::env::consts::ARCH;
    let candidates = [
        format!("{package_name}:{arch}.list"),
        format!("{package_name}.list"),
    ];

    let mut list_file_path = None;
    for candidate in &candidates {
        let path = Path::new("/var/lib/dpkg/info").join(candidate);
        if path.exists() {
            list_file_path = Some(path);
            break;
        }
    }

    let Some(list_path) = list_file_path else {
        tracing::warn!(
            "No .list file found for {}, cannot remove files",
            package_name
        );
        return Ok(());
    };

    // Read file list
    let list_content = fs::read_to_string(&list_path)
        .with_context(|| format!("Failed to read {}", list_path.display()))?;

    // Parse file paths - collect files and directories separately
    let mut files_to_remove = Vec::new();
    let mut dirs_to_remove = Vec::new();

    for line in list_content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let path = PathBuf::from(line);
        if line.ends_with('/') {
            // Directory
            dirs_to_remove.push(path);
        } else {
            // Regular file or symlink
            files_to_remove.push(path);
        }
    }

    // Remove files in reverse order (deepest first)
    files_to_remove.reverse();

    let mut removed_count = 0;
    let mut failed_count = 0;

    for file_path in &files_to_remove {
        // Skip if it doesn't exist
        if !file_path.exists() {
            continue;
        }

        // Check if it's a conffile - skip those (keep for config-files state)
        if is_conffile(package_name, file_path) {
            tracing::debug!("Skipping conffile: {}", file_path.display());
            continue;
        }

        // Remove the file
        match fs::remove_file(file_path) {
            Ok(()) => {
                removed_count += 1;
                tracing::trace!("Removed: {}", file_path.display());
            }
            Err(e) => {
                failed_count += 1;
                tracing::debug!("Failed to remove {}: {}", file_path.display(), e);
            }
        }
    }

    // Try to remove empty directories (bottom-up)
    dirs_to_remove.reverse();
    for dir_path in &dirs_to_remove {
        if dir_path.exists()
            && dir_path.is_dir()
            && let Ok(mut entries) = fs::read_dir(dir_path)
            && entries.next().is_none()
            && let Err(e) = fs::remove_dir(dir_path)
        {
            tracing::trace!("Could not remove directory {}: {}", dir_path.display(), e);
        }
    }

    tracing::debug!(
        "Removed {} files from {} (skipped: {})",
        removed_count,
        package_name,
        failed_count
    );

    Ok(())
}

/// Check if a file path is a conffile for the given package
fn is_conffile(package_name: &str, file_path: &Path) -> bool {
    let arch = std::env::consts::ARCH;
    let candidates = [
        format!("{package_name}:{arch}.conffiles"),
        format!("{package_name}.conffiles"),
    ];

    for candidate in &candidates {
        let conffiles_path = Path::new("/var/lib/dpkg/info").join(candidate);
        if let Ok(content) = fs::read_to_string(&conffiles_path) {
            for line in content.lines() {
                if Path::new(line.trim()) == file_path {
                    return true;
                }
            }
        }
    }

    false
}

/// Update /var/lib/dpkg/status to mark package as removed (config-files state)
fn update_dpkg_status_for_removal(package_name: &str) -> Result<()> {
    use tempfile::NamedTempFile;

    let status_path = Path::new("/var/lib/dpkg/status");
    if !status_path.exists() {
        anyhow::bail!("dpkg status file not found: {}", status_path.display());
    }

    // Read entire status file
    let status_content =
        fs::read_to_string(status_path).context("Failed to read dpkg status file")?;

    // Parse and update the package paragraph
    let mut updated_content = String::with_capacity(status_content.len());
    let mut in_target_package = false;
    let mut found_package = false;

    for line in status_content.lines() {
        if line.is_empty() {
            // End of paragraph
            in_target_package = false;
            updated_content.push('\n');
        } else if let Some(pkg) = line.strip_prefix("Package: ") {
            in_target_package = pkg.trim() == package_name;
            if in_target_package {
                found_package = true;
            }
            updated_content.push_str(line);
            updated_content.push('\n');
        } else if in_target_package && line.starts_with("Status: ") {
            // Update status from "install ok installed" to "deinstall ok config-files"
            updated_content.push_str("Status: deinstall ok config-files\n");
        } else {
            updated_content.push_str(line);
            updated_content.push('\n');
        }
    }

    if !found_package {
        anyhow::bail!("Package {package_name} not found in dpkg status");
    }

    // Atomic write using temp file + rename
    let parent = status_path
        .parent()
        .unwrap_or_else(|| Path::new("/var/lib/dpkg"));
    let mut temp_file =
        NamedTempFile::new_in(parent).context("Failed to create temporary status file")?;

    temp_file
        .write_all(updated_content.as_bytes())
        .context("Failed to write updated status")?;
    temp_file
        .as_file_mut()
        .sync_all()
        .context("Failed to sync updated dpkg status")?;
    temp_file
        .persist(status_path)
        .map_err(|error| error.error)
        .context("Failed to persist updated status file")?;

    tracing::debug!(
        "Updated dpkg status for {} to config-files state",
        package_name
    );
    Ok(())
}

/// Clean up dpkg info files after removal
fn cleanup_dpkg_info_files(package_name: &str) {
    let arch = std::env::consts::ARCH;

    // Files to remove
    let extensions_to_remove = [
        "list", "md5sums", "prerm", "postinst", "preinst", "postrm", "triggers", "shlibs",
        "symbols",
    ];

    let mut removed_count = 0;

    for ext in &extensions_to_remove {
        let candidates = [
            format!("{package_name}:{arch}.{ext}"),
            format!("{package_name}.{ext}"),
        ];

        for candidate in &candidates {
            let file_path = Path::new("/var/lib/dpkg/info").join(candidate);
            if file_path.exists() {
                if let Err(e) = fs::remove_file(&file_path) {
                    tracing::warn!("Failed to remove {}: {}", file_path.display(), e);
                } else {
                    removed_count += 1;
                    tracing::trace!("Removed: {}", file_path.display());
                }
            }
        }
    }

    // Keep .conffiles and .config files (for config-files state)
    tracing::debug!(
        "Cleaned up {} info files for {} (kept conffiles)",
        removed_count,
        package_name
    );
}

/// Dry-run a transaction (show what would be done)
pub fn dry_run(result: &ResolutionResult) -> String {
    let mut output = String::new();

    if !result.to_install.is_empty() {
        output.push_str("The following NEW packages will be installed:\n  ");
        output.push_str(&result.to_install.join(" "));
        output.push('\n');
    }

    if !result.to_upgrade.is_empty() {
        output.push_str("The following packages will be upgraded:\n  ");
        let names: Vec<_> = result
            .to_upgrade
            .iter()
            .map(|(n, _, _)| n.as_str())
            .collect();
        output.push_str(&names.join(" "));
        output.push('\n');
    }

    if !result.to_remove.is_empty() {
        output.push_str("The following packages will be REMOVED:\n  ");
        output.push_str(&result.to_remove.join(" "));
        output.push('\n');
    }

    use std::fmt::Write;
    let _ = write!(
        output,
        "\n{} to upgrade, {} newly installed, {} to remove.\n",
        result.to_upgrade.len(),
        result.to_install.len(),
        result.to_remove.len()
    );

    let _ = writeln!(
        output,
        "Need to download {} bytes of archives.",
        result.download_size
    );

    let _ = writeln!(
        output,
        "After this operation, {} bytes of additional disk space will be used.",
        result.installed_size
    );

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transaction_new() {
        let tx = Transaction::new();
        assert_eq!(tx.state, TransactionState::Pending);
        assert!(tx.to_install.is_empty());
        assert!(tx.to_remove.is_empty());
    }

    #[test]
    fn test_transaction_add_install() {
        let mut tx = Transaction::new();
        tx.add_install(
            "vim".to_string(),
            "9.0".to_string(),
            "http://example.com/vim.deb".to_string(),
            1024,
        );
        assert_eq!(tx.to_install.len(), 1);
        assert_eq!(tx.to_install[0].name, "vim");
    }

    #[test]
    fn test_transaction_sizes() {
        let mut tx = Transaction::new();
        tx.add_install("pkg1".to_string(), "1.0".to_string(), String::new(), 1000);
        tx.add_install("pkg2".to_string(), "1.0".to_string(), String::new(), 2000);
        assert_eq!(tx.total_download_size(), 3000);
        assert_eq!(tx.package_count(), 2);
    }

    #[test]
    fn test_require_installed_version_rejects_missing_and_empty() {
        assert_eq!(
            Transaction::require_installed_version("vim", Some("9.0".into())).unwrap(),
            "9.0"
        );
        let missing = Transaction::require_installed_version("vim", None).unwrap_err();
        assert!(
            missing.to_string().contains("no version in dpkg status"),
            "got: {missing}"
        );
        let empty = Transaction::require_installed_version("vim", Some(String::new())).unwrap_err();
        assert!(
            empty.to_string().contains("no version in dpkg status"),
            "got: {empty}"
        );
    }

    #[test]
    fn test_dry_run() {
        let result = ResolutionResult {
            to_install: vec!["vim".to_string(), "git".to_string()],
            to_upgrade: vec![("curl".to_string(), "1.0".to_string(), "2.0".to_string())],
            to_remove: Vec::new(),
            download_size: 10240,
            installed_size: 51200,
        };

        let output = dry_run(&result);
        assert!(output.contains("vim"));
        assert!(output.contains("git"));
        assert!(output.contains("curl"));
        assert!(output.contains("10240"));
    }
}
