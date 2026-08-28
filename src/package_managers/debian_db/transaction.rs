//! Pure Rust Transaction Engine for Debian/Ubuntu
//!
//! Handles package installation, removal, and upgrades using pure Rust:
//! - .deb archive extraction (ar + tar.xz/tar.gz)
//! - dpkg database updates (atomic rewrite; pre-transaction backup)
//! - Pre/post-install script execution
//! - Rollback of newly installed files, directories, and dpkg status
//!
//! Known limitation: files that a package *overwrites* are removed on
//! rollback but not restored from backup; the dpkg status database is
//! backed up and restored, but prior file contents are not.

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
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
use crate::runtimes::common::{BudgetedReader, BudgetedSink};

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

/// Upper bound for one ar member inside a `.deb` (control.tar/data.tar as
/// stored, before decompression). The decompressed payload has its own
/// streaming budget ([`crate::runtimes::common::MAX_DECOMPRESSED_BYTES`]);
/// this bounds raw buffering of untrusted archive members.
const MAX_DEB_MEMBER_BYTES: u64 = 2 * 1024 * 1024 * 1024;

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
    pub fn from_resolution(result: ResolutionResult) -> Result<Self> {
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
        content_store.init()?;

        Ok(Self {
            state: TransactionState::Pending,
            to_install,
            to_remove,
            to_upgrade,
            temp_dir: None,
            backups: HashMap::new(),
            installed_files: Vec::new(),
            content_store,
        })
    }

    /// Create an empty transaction
    pub fn new() -> Result<Self> {
        let content_store = ContentStore::new();
        content_store.init()?;

        Ok(Self {
            state: TransactionState::Pending,
            to_install: Vec::new(),
            to_remove: Vec::new(),
            to_upgrade: Vec::new(),
            temp_dir: None,
            backups: HashMap::new(),
            installed_files: Vec::new(),
            content_store,
        })
    }

    /// Return the private transaction workspace after `execute` initializes it.
    ///
    /// Transaction steps must never fall back to a shared directory: they may
    /// write predictable package and rollback filenames while running as root.
    fn transaction_temp_dir(&self) -> Result<PathBuf> {
        self.temp_dir
            .as_ref()
            .map(|temp_dir| temp_dir.path().to_path_buf())
            .context("Transaction temporary directory is not initialized")
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
        use tokio::sync::mpsc;

        // OPTIMIZATION: Memory-conscious HTTP client configuration
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(10))
            .pool_max_idle_per_host(MAX_CONCURRENT_PACKAGE_DOWNLOADS * 2)
            .pool_idle_timeout(Duration::from_secs(60)) // Shorter timeout to release faster
            .tcp_keepalive(Duration::from_secs(60))
            .tcp_nodelay(true)
            .build()?;

        let temp_dir = self.transaction_temp_dir()?;

        // Collect all packages that need downloading
        let packages_to_download: Vec<(usize, String, String, String, Option<String>, u64)> = self
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
                        action.size,
                    )
                })
            })
            .collect();

        if packages_to_download.is_empty() {
            return Ok(());
        }

        // OPTIMIZATION: Pre-warm connections to the mirror (fire and forget)
        // Extract unique hosts and make HEAD requests to establish connections
        if let Some((_, _, _, first_url, _, _)) = packages_to_download.first()
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

        // Pre-create the download task futures with their own sender clones
        let download_futures: Vec<_> = packages_to_download
            .into_iter()
            .map(|(idx, name, version, url, sha256, expected_size)| {
                let client = client.clone();
                let temp_dir = temp_dir.clone();
                let content_store = content_store.clone();
                let tx = tx.clone();
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
                        expected_size,
                    )
                    .await;

                    match result {
                        Ok(path) => {
                            pb.set_message("✓ dl".green().to_string());
                            pb.finish();

                            // Fail loudly if the unpack worker is gone; a silent
                            // send failure would count the package as downloaded
                            // while it is never unpacked or configured.
                            tx.send((path.clone(), name.clone()))
                                .await
                                .map_err(|_| {
                                    anyhow::anyhow!(
                                        "unpack worker exited before receiving '{name}'; transaction aborted"
                                    )
                                })?;
                            Ok((idx, path))
                        }
                        Err(e) => {
                            pb.set_message(format!("{e}").red().to_string());
                            pb.finish();
                            Err(e)
                        }
                    }
                }
            })
            .collect();

        // Drop our sender so the channel closes when all downloads complete
        drop(tx);

        // Spawn unpack worker thread
        let temp_dir_unpack = temp_dir.clone();
        let unpack_handle = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            let mut installed_files = Vec::new();
            let mut unpack_errors = Vec::new();

            // OPTIMIZATION: Process packages immediately instead of batching
            // This reduces memory pressure from holding multiple .deb files in memory
            while let Some((deb_path, name)) = rt.block_on(rx.recv()) {
                tracing::debug!("Unpacking {} immediately", name);
                match unpack_deb_standalone(&deb_path, &name, &temp_dir_unpack) {
                    Ok(files) => {
                        installed_files.extend(files);
                        tracing::debug!("Unpacked {} successfully", name);
                    }
                    Err(e) => {
                        // Recover the partial manifest so already-written
                        // files stay tracked for rollback (audit A2).
                        if let Some(partial) = e
                            .downcast_ref::<PartialExtractionError>()
                            .map(|pe| pe.installed_files.clone())
                        {
                            installed_files.extend(partial);
                        }
                        tracing::error!("Failed to unpack {}: {}", name, e);
                        unpack_errors.push((name.clone(), e));
                    }
                }

                // OPTIMIZATION: Delete .deb file immediately after unpacking
                // This reduces disk I/O and frees up temp space quickly
                if let Err(e) = remove_file_if_present(&deb_path) {
                    tracing::error!("Failed to delete unpacked {}: {}", deb_path.display(), e);
                    unpack_errors.push((name.clone(), e));
                }
            }

            (installed_files, unpack_errors)
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
    /// dpkg status updates are merged and written atomically: existing
    /// paragraphs for the same package are *replaced* (an upgrade must not
    /// leave two entries), and a pre-transaction copy of the status file is
    /// recorded for rollback before the first write.
    fn configure_packages(&mut self) -> Result<()> {
        let temp_dir = self.transaction_temp_dir()?;

        // Collect all status entries for batched write
        let mut status_entries: Vec<(String, String)> = Vec::new();
        let mut conffiles_to_copy = Vec::new();

        for action in self.to_install.iter().chain(self.to_upgrade.iter()) {
            let extract_dir = temp_dir.join(&action.name);
            let control_dir = extract_dir.join("DEBIAN");

            // Run postinst script if exists (must be sequential for dependencies)
            let postinst = control_dir.join("postinst");
            if postinst.exists() {
                run_maintainer_script(&postinst, &action.name, "configure")?;
            }

            // Prepare dpkg status entry for batched write. A failure here is
            // fatal: an unpacked-but-unregistered package would be invisible
            // to dpkg tooling.
            let control_file = control_dir.join("control");
            if control_file.exists() {
                let entry = prepare_status_entry(&control_file)
                    .with_context(|| format!("preparing dpkg status entry for {}", action.name))?;
                status_entries.push((action.name.clone(), entry));
            }

            // Collect conffiles for batched copy
            let conffiles = control_dir.join("conffiles");
            if conffiles.exists() {
                conffiles_to_copy.push((conffiles, action.name.clone()));
            }
        }

        if !status_entries.is_empty() {
            self.record_dpkg_status_entries(&status_entries, &temp_dir)?;
        }

        // Copy conffiles (can be done in parallel with rayon)
        if !conffiles_to_copy.is_empty() {
            use rayon::prelude::*;

            conffiles_to_copy.par_iter().try_for_each(|(src, name)| {
                let dest = Path::new("/var/lib/dpkg/info").join(format!("{name}.conffiles"));
                copy_conffile(src, &dest)
            })?;
        }

        Ok(())
    }

    /// Merge `entries` into `/var/lib/dpkg/status` atomically.
    ///
    /// Takes a rollback backup first (registered in `self.backups`), replaces
    /// any existing paragraph for the same package, and persists via
    /// temp-file + rename so a crash cannot truncate the database.
    fn record_dpkg_status_entries(
        &mut self,
        entries: &[(String, String)],
        temp_dir: &Path,
    ) -> Result<()> {
        let status_path = Path::new("/var/lib/dpkg/status");
        let current = match fs::read_to_string(status_path) {
            Ok(content) => content,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(error) => {
                return Err(error).context("Failed to read dpkg status file");
            }
        };

        // Integrity-bound persisted data: snapshot for rollback before the
        // first mutation of this transaction.
        let backup_path = temp_dir.join("dpkg-status.pre-transaction");
        fs::write(&backup_path, &current).with_context(|| {
            format!("Failed to back up dpkg status to {}", backup_path.display())
        })?;
        self.backups.insert(status_path.to_path_buf(), backup_path);

        let updated = merge_status_entries(&current, entries);
        write_atomic(status_path, updated.as_bytes())
            .context("Failed to persist updated dpkg status")?;

        tracing::debug!("Merged {} dpkg status entries atomically", entries.len());
        Ok(())
    }

    /// Rollback the transaction
    ///
    /// Removes files/directories installed by this transaction and restores
    /// the pre-transaction dpkg status database. Removal problems are
    /// collected: rollback always attempts the integrity-critical status
    /// restore before reporting a combined failure, instead of aborting on
    /// the first stuck path and leaving the package database permanently
    /// inconsistent. Files that a package *overwrote* are removed but their
    /// previous contents are not restored; see the module docs for the full
    /// contract.
    pub fn rollback(&mut self) -> Result<()> {
        tracing::warn!(
            "Rolling back transaction ({} files installed)",
            self.installed_files.len()
        );

        // Reverse order: children were recorded before their parent
        // directories, so unwinding backwards removes files first and then
        // the (now empty) directories created for them.
        let mut removal_failures: Vec<(PathBuf, anyhow::Error)> = Vec::new();
        for file in self.installed_files.iter().rev() {
            if let Err(error) = remove_file_if_present(file) {
                tracing::error!("Rollback could not remove {}: {error:#}", file.display());
                removal_failures.push((file.clone(), error));
            }
        }

        // The dpkg status snapshot is integrity-bound persisted data: its
        // restore must be attempted even when path cleanup got stuck.
        let mut restore_failures: Vec<(PathBuf, anyhow::Error)> = Vec::new();
        for (original, backup) in &self.backups {
            if let Err(error) = restore_backup(backup, original) {
                tracing::error!(
                    "Rollback could not restore {} from {}: {error:#}",
                    original.display(),
                    backup.display()
                );
                restore_failures.push((original.clone(), error));
            }
        }

        self.state = TransactionState::RolledBack;

        if removal_failures.is_empty() && restore_failures.is_empty() {
            tracing::info!("Transaction rolled back successfully");
            return Ok(());
        }

        anyhow::bail!(
            "Rollback incomplete: {} installed path(s) could not be removed, \
             {} backup(s) could not be restored; system may need manual repair. \
             First removal failure: {}; first restore failure: {}",
            removal_failures.len(),
            restore_failures.len(),
            removal_failures
                .first()
                .map_or_else(String::new, |(_, error)| error.to_string()),
            restore_failures
                .first()
                .map_or_else(String::new, |(_, error)| error.to_string()),
        )
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

    fn require_package_installed(name: &str, installed: bool) -> Result<()> {
        if installed {
            Ok(())
        } else {
            anyhow::bail!("package {name} is not installed")
        }
    }

    /// Execute package removal
    ///
    /// The whole removal loop (maintainer scripts via `Command`, dpkg status
    /// rewrites with fsync, filesystem walks) is synchronous and unbounded in
    /// wall time, so it runs on the blocking pool instead of the executor.
    /// `indicatif` progress bars are thread-safe and keep drawing from the
    /// blocking thread.
    pub async fn execute_removal(&mut self) -> Result<()> {
        if self.to_remove.is_empty() {
            return Ok(());
        }

        let package_names: Vec<String> = self.to_remove.iter().map(|a| a.name.clone()).collect();
        tokio::task::spawn_blocking(move || execute_removal_blocking(&package_names))
            .await
            .context("Package removal task failed")?
    }
}
/// Synchronous body of [`Transaction::execute_removal`], executed on the
/// blocking pool so maintainer scripts and fsync-heavy status rewrites never
/// stall the async executor.
fn execute_removal_blocking(packages_to_remove: &[String]) -> Result<()> {
    tracing::info!("Starting removal of {} packages", packages_to_remove.len());

    // Setup progress display
    let multi = MultiProgress::new();
    let overall = multi.add(ProgressBar::new(packages_to_remove.len() as u64));
    overall.set_style(
        ProgressStyle::default_bar()
            .template("{prefix:.bold} [{bar:40.red/blue}] {pos}/{len} {msg}")
            .expect("valid template")
            .progress_chars("=>-"),
    );
    overall.set_prefix("Removing");

    remove_packages_sequentially(packages_to_remove, &multi, &overall)?;

    overall.finish_and_clear();
    tracing::info!("Successfully removed {} packages", packages_to_remove.len());
    Ok(())
}

/// Remove `packages_to_remove` one at a time, driving the per-package and
/// overall progress bars. Split from [`execute_removal_blocking`] so tests can
/// exercise the removal step sequence without progress-bar setup.
fn remove_packages_sequentially(
    packages_to_remove: &[String],
    multi: &MultiProgress,
    overall: &ProgressBar,
) -> Result<()> {
    // Process packages in dependency order (leaves first).
    for package_name in packages_to_remove {
        let pb = multi.add(ProgressBar::new(5));
        pb.set_style(
            ProgressStyle::default_bar()
                .template("  {prefix:.cyan} [{bar:25.red/blue}] {msg}")
                .expect("valid template")
                .progress_chars("=>-"),
        );
        pb.set_prefix(package_name.clone());

        pb.set_message("validating");
        pb.inc(1);
        Transaction::require_package_installed(
            package_name,
            super::is_installed_fast(package_name)?,
        )?;

        let version = Transaction::require_installed_version(
            package_name,
            super::get_package_version(package_name)?,
        )?;

        pb.set_message("prerm");
        pb.inc(1);
        run_removal_maintainer_script(package_name, "prerm").map_err(|error| {
            removal_step_failed(&pb, overall, package_name, "prerm script", error)
        })?;

        pb.set_message("removing files");
        pb.inc(1);
        remove_package_files(package_name).map_err(|error| {
            removal_step_failed(&pb, overall, package_name, "file removal", error)
        })?;

        pb.set_message("postrm");
        pb.inc(1);
        run_removal_maintainer_script(package_name, "postrm").map_err(|error| {
            removal_step_failed(&pb, overall, package_name, "postrm script", error)
        })?;

        pb.set_message("updating status");
        pb.inc(1);
        update_dpkg_status_for_removal(package_name).map_err(|error| {
            removal_step_failed(&pb, overall, package_name, "status update", error)
        })?;

        pb.set_message("cleanup");
        pb.inc(1);
        cleanup_dpkg_info_files(package_name)
            .map_err(|error| removal_step_failed(&pb, overall, package_name, "cleanup", error))?;

        pb.set_message("\u{2713}".green().to_string());
        pb.finish();
        overall.inc(1);
        tracing::info!("Removed {} v{}", package_name, version);
    }

    Ok(())
}
/// Merge new dpkg status paragraphs into `current`.
///
/// Existing paragraphs whose `Package:` name appears in `entries` are
/// replaced by the new entry; all other paragraphs are preserved verbatim.
/// New entries are appended in the given order. The result keeps dpkg's
/// one-blank-line paragraph separation and a trailing newline.
fn merge_status_entries(current: &str, entries: &[(String, String)]) -> String {
    let mut pending: std::collections::HashMap<&str, &str> = entries
        .iter()
        .map(|(name, entry)| (name.as_str(), entry.as_str()))
        .collect();

    let mut out = String::with_capacity(current.len() + 512);
    for paragraph in current.split("\n\n") {
        if paragraph.trim().is_empty() {
            continue;
        }
        let name = paragraph
            .lines()
            .find_map(|line| line.strip_prefix("Package: "))
            .map(str::trim);
        let replaced = name.and_then(|name| pending.remove(name));
        if let Some(entry) = replaced {
            out.push_str(entry);
        } else {
            out.push_str(paragraph);
            out.push('\n');
        }
        out.push('\n');
    }

    for (name, entry) in entries {
        if pending.remove(name.as_str()).is_some() {
            out.push_str(entry);
            out.push('\n');
        }
    }

    out
}

/// Atomically replace `dest` with `data` via temp file + fsync + rename.
fn write_atomic(dest: &Path, data: &[u8]) -> Result<()> {
    use tempfile::NamedTempFile;

    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    let mut temp = NamedTempFile::new_in(parent)
        .with_context(|| format!("Failed to create temporary file in {}", parent.display()))?;
    temp.write_all(data)?;
    temp.as_file_mut()
        .sync_all()
        .context("Failed to sync temporary file")?;
    temp.persist(dest)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to persist {}", dest.display()))?;
    Ok(())
}

fn remove_file_if_present(path: &Path) -> Result<()> {
    // Rollback walks `installed_files` in reverse so children are removed
    // before the directories created for them. A directory that still has
    // content pre-dates this transaction and must not be deleted; surfacing
    // that as an error keeps rollback honest instead of destroying foreign
    // files.
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir(path)
            .with_context(|| format!("Failed to remove directory {}", path.display())),
        Ok(_) => match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("Failed to remove {}", path.display()))
            }
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("Failed to inspect {} for removal", path.display()))
        }
    }
}

fn restore_backup(backup: &Path, original: &Path) -> Result<()> {
    fs::copy(backup, original).with_context(|| {
        format!(
            "Failed to restore backup {} -> {}",
            backup.display(),
            original.display()
        )
    })?;
    Ok(())
}

fn copy_conffile(src: &Path, dest: &Path) -> Result<()> {
    fs::copy(src, dest).with_context(|| {
        format!(
            "Failed to copy conffiles {} -> {}",
            src.display(),
            dest.display()
        )
    })?;
    Ok(())
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

        // Bound the raw buffering of each untrusted archive member; the
        // decompression budget applies later, to the payload itself.
        let mut contents = Vec::new();
        (&mut entry)
            .take(MAX_DEB_MEMBER_BYTES.saturating_add(1))
            .read_to_end(&mut contents)
            .with_context(|| format!("Failed to read {name} from .deb archive"))?;
        if u64::try_from(contents.len()).unwrap_or(u64::MAX) > MAX_DEB_MEMBER_BYTES {
            anyhow::bail!("Archive member {name} exceeds the {MAX_DEB_MEMBER_BYTES} byte limit");
        }

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
            run_maintainer_script(&preinst, package_name, "install")?;
        }
    }

    // Extract data.tar to filesystem. On mid-extraction failure the partial
    // manifest of already-written files is recovered and returned alongside
    // the error so rollback tracking never loses residue (audit A2).
    let installed_files = if let Some(data) = data_tar {
        match extract_tar_to_root(&data) {
            Ok(files) => files,
            Err(error) => {
                let partial = error
                    .downcast_ref::<PartialExtractionError>()
                    .map(|e| e.installed_files.clone())
                    .unwrap_or_default();
                if !partial.is_empty() {
                    tracing::warn!(
                        "Partial extraction wrote {} file(s); recovered for rollback tracking",
                        partial.len()
                    );
                }
                return Err(error);
            }
        }
    } else {
        Vec::new()
    };

    Ok(installed_files)
}

/// Map a data.tar entry path onto an absolute path under `root`, rejecting
/// anything that is not a plain relative path.
///
/// Debian data.tar entries are conventionally `./usr/bin/tool`; this accepts
/// that form (and plain `usr/bin/tool`) while explicitly rejecting parent
/// components and other non-normal components instead of letting the kernel
/// resolve them at extraction time.
fn data_tar_entry_path(root: &Path, entry_path: &Path) -> Result<PathBuf> {
    let text = entry_path.to_string_lossy();
    let rel = text.strip_prefix("./").unwrap_or(&text);

    let mut resolved = root.to_path_buf();
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(part) => resolved.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                anyhow::bail!(
                    "Unsafe data.tar entry (parent component): {}",
                    entry_path.display()
                )
            }
            Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!(
                    "Unsafe data.tar entry (absolute or prefixed): {}",
                    entry_path.display()
                )
            }
        }
    }

    if resolved == root {
        anyhow::bail!("data.tar entry resolves to the filesystem root");
    }
    Ok(resolved)
}

/// Validate that a relative symlink target stays inside the extraction root.
///
/// The link's parent directory is expressed relative to `root`, then the
/// target's components are applied with a depth counter: a `..` at depth zero
/// (or any absolute target) is rejected. This treats the extraction root as
/// the containment boundary regardless of where it lives on disk — for the
/// production root of `/` the two coincide, but tests and future non-root
/// extraction must not be able to pop above their own root.
fn validate_root_relative_link_target(root: &Path, link_path: &Path, target: &Path) -> Result<()> {
    let rel_parent = link_path.strip_prefix(root).unwrap_or(link_path);
    let mut depth = rel_parent
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count();

    for component in target.components() {
        match component {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir => {
                anyhow::ensure!(
                    depth > 0,
                    "Archive symlink escapes the extraction directory: {} -> {}",
                    link_path.display(),
                    target.display()
                );
                depth -= 1;
            }
            Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!(
                    "Archive symlink target must be relative: {} -> {}",
                    link_path.display(),
                    target.display()
                )
            }
        }
    }
    Ok(())
}

/// A link whose creation is deferred until every regular file has been
/// written, so no regular-file write can traverse a link from the archive.
enum PendingRootLink {
    Symbolic { path: PathBuf, target: PathBuf },
    Hard { path: PathBuf, target: PathBuf },
}

fn create_root_links(links: Vec<PendingRootLink>) -> Result<()> {
    for link in links {
        match link {
            PendingRootLink::Symbolic { path, target } => {
                std::os::unix::fs::symlink(&target, &path).with_context(|| {
                    format!(
                        "Failed to create archive symlink {} -> {}",
                        path.display(),
                        target.display()
                    )
                })?;
            }
            PendingRootLink::Hard { path, target } => {
                fs::hard_link(&target, &path).with_context(|| {
                    format!(
                        "Failed to create archive hard link {} -> {}",
                        path.display(),
                        target.display()
                    )
                })?;
            }
        }
    }
    Ok(())
}

/// Extract a tar stream to a directory, delegating traversal sanitization to
/// the `tar` crate's hardened `unpack`.
///
/// Trust note: unlike [`extract_tar_to_root_at`] (which validates every entry
/// explicitly), this path relies on `tar::Entry::unpack` refusing paths that
/// escape `dest`. Acceptable because control.tar carries only the small
/// maintainer-script set, but revisit before exposing non-root extraction.
fn extract_tar_stream(reader: &mut dyn Read, dest: &Path) -> Result<()> {
    let mut archive = tar::Archive::new(reader);

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

/// Detect the compression format of a `.tar.<ext>` payload and return a
/// reader over the decompressed tar bytes. Every decoder is wrapped so the
/// decompressed-size budget is enforced while bytes are produced; a bomb
/// aborts mid-stream instead of exhausting memory.
fn tar_payload_reader(data: &[u8]) -> Result<Box<dyn Read + '_>> {
    tar_payload_reader_with_budget(data, BudgetedSink::max_budget())
}

/// [`tar_payload_reader`] with an explicit budget; tests use a small budget
/// so the abort path is exercisable without gigabyte allocations.
fn tar_payload_reader_with_budget(data: &[u8], budget: u64) -> Result<Box<dyn Read + '_>> {
    if data.len() > 4 && data.starts_with(b"\x28\xb5\x2f\xfd") {
        // Zstd: Fast decompression, good compression
        let decoder = ruzstd::decoding::StreamingDecoder::new(std::io::Cursor::new(data))
            .map_err(|e| anyhow::anyhow!("Failed to create zstd decoder: {e}"))?;
        return Ok(Box::new(BudgetedReader::new(decoder, budget)));
    }

    if data.len() > 6 && data.starts_with(b"\xfd7zXZ\x00") {
        // XZ: lzma-rs only exposes a Read->Write API, so bound the output
        // sink instead; it stops accepting bytes at the budget, which stops
        // buffer growth during decompression rather than after it.
        let mut sink = BudgetedSink::with_budget(budget);
        lzma_rs::xz_decompress(&mut std::io::Cursor::new(data), &mut sink)
            .map_err(|e| anyhow::anyhow!("Failed to decompress XZ payload: {e}"))?;
        return Ok(Box::new(std::io::Cursor::new(sink.into_inner())));
    }

    if data.len() > 2 && data[0] == 0x1f && data[1] == 0x8b {
        // Gzip: legacy packages
        let decoder = flate2::read::GzDecoder::new(data);
        return Ok(Box::new(BudgetedReader::new(decoder, budget)));
    }

    // Uncompressed tar
    Ok(Box::new(std::io::Cursor::new(data)))
}

/// Create any missing ancestor directories between `root` and `path`,
/// recording each newly created directory in `installed_files` so rollback can
/// remove the full chain instead of leaving empty residue behind. Ancestors
/// already handled for this archive are skipped.
fn ensure_parent_dirs_recorded(
    path: &Path,
    root: &Path,
    seen: &mut std::collections::HashSet<PathBuf>,
    installed_files: &mut Vec<PathBuf>,
) -> Result<()> {
    let mut missing = Vec::new();
    for ancestor in path.ancestors().skip(1) {
        if ancestor == root || seen.contains(ancestor) {
            break;
        }
        seen.insert(ancestor.to_path_buf());
        match fs::symlink_metadata(ancestor) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing.push(ancestor.to_path_buf());
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to inspect {} for extraction", ancestor.display())
                });
            }
        }
    }

    // Ancestors arrive nearest-first; create outermost first so each single
    // level `create_dir` sees an existing parent.
    for dir in missing.into_iter().rev() {
        fs::create_dir(&dir)
            .with_context(|| format!("Failed to create directory {}", dir.display()))?;
        installed_files.push(dir);
    }
    Ok(())
}

/// Extract a tar archive with auto-detection of compression
fn extract_tar_auto(data: &[u8], dest: &Path) -> Result<()> {
    let mut reader = tar_payload_reader(data)?;
    extract_tar_stream(reader.as_mut(), dest)
}

/// Extract a data.tar payload into the filesystem root with dpkg-like
/// semantics and hardened handling of untrusted entries:
///
/// - regular files are written directly (streaming); their paths are
///   normalized through [`data_tar_entry_path`], which rejects parent and
///   absolute components;
/// - directories are created and recorded so rollback can remove them;
/// - symbolic links are validated (relative targets only, staying inside the
///   extraction root) and created only after every regular file has been
///   written, so a file entry can never traverse a link defined by the same
///   archive;
/// - hard links are re-created against the already-extracted tree after all
///   files exist;
/// - any other entry type (devices, FIFOs) fails the install explicitly.
fn extract_tar_to_root(data: &[u8]) -> Result<Vec<PathBuf>> {
    extract_tar_to_root_at(Path::new("/"), data)
}

/// [`extract_tar_to_root`] against an explicit root; tests use a temporary
/// directory, production uses `/`.
/// Error wrapper carrying the files already written before a mid-extraction
/// failure, so the caller can merge them into rollback tracking instead of
/// leaving untracked residue under `/` (audit A2).
#[derive(Debug)]
struct PartialExtractionError {
    source: anyhow::Error,
    installed_files: Vec<PathBuf>,
}

impl std::fmt::Display for PartialExtractionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.source)
    }
}

impl std::error::Error for PartialExtractionError {}

fn extract_tar_to_root_at(root: &Path, data: &[u8]) -> Result<Vec<PathBuf>> {
    // Inner scope owns the manifest so ANY failure can carry the files
    // already written back to the caller (audit A2): without this, a
    // mid-extraction error left untracked residue under `/` that rollback
    // could never see.
    let inner = |installed_files: &mut Vec<PathBuf>| -> anyhow::Result<()> {
        let mut reader = tar_payload_reader(data)?;
        let mut archive = tar::Archive::new(reader.as_mut());

        tracing::debug!(
            "Extracting data.tar ({} compressed bytes) into {}",
            data.len(),
            root.display()
        );

        let mut pending_links = Vec::new();
        let mut seen_dirs: std::collections::HashSet<PathBuf> = std::collections::HashSet::new();

        for entry in archive.entries()? {
            let mut entry = entry?;
            let entry_path = data_tar_entry_path(root, &entry.path()?)?;
            let entry_type = entry.header().entry_type();

            if entry_type.is_dir() {
                ensure_parent_dirs_recorded(&entry_path, root, &mut seen_dirs, installed_files)?;
                if let Err(error) = fs::create_dir(&entry_path)
                    && error.kind() != std::io::ErrorKind::AlreadyExists
                {
                    return Err(error).with_context(|| {
                        format!("Failed to create directory {}", entry_path.display())
                    });
                }
                installed_files.push(entry_path);
                continue;
            }

            if entry_type.is_symlink() {
                let target = entry
                    .link_name()?
                    .context("Archive symlink is missing its target")?
                    .into_owned();
                validate_root_relative_link_target(root, &entry_path, &target)?;
                installed_files.push(entry_path.clone());
                pending_links.push(PendingRootLink::Symbolic {
                    path: entry_path,
                    target,
                });
                continue;
            }

            if entry_type.is_hard_link() {
                let target = entry
                    .link_name()?
                    .context("Archive hard link is missing its target")?
                    .into_owned();
                let target = data_tar_entry_path(root, &target)?;
                installed_files.push(entry_path.clone());
                pending_links.push(PendingRootLink::Hard {
                    path: entry_path,
                    target,
                });
                continue;
            }

            if !entry_type.is_file() {
                anyhow::bail!(
                    "Unsupported special entry in package data.tar: {} ({entry_type:?})",
                    entry_path.display()
                );
            }

            ensure_parent_dirs_recorded(&entry_path, root, &mut seen_dirs, installed_files)?;

            let mode = entry.header().mode()?;
            let size = entry.header().size()?;

            if size < 1024 {
                let mut contents = Vec::with_capacity(size as usize);
                entry.read_to_end(&mut contents)?;
                fs::write(&entry_path, contents)
                    .with_context(|| format!("Failed to write file: {}", entry_path.display()))?;
            } else {
                use std::io::BufWriter;
                let file = File::create(&entry_path)
                    .with_context(|| format!("Failed to create file: {}", entry_path.display()))?;
                let mut writer = BufWriter::with_capacity(16384, file);
                std::io::copy(&mut entry, &mut writer)
                    .with_context(|| format!("Failed to copy to: {}", entry_path.display()))?;
                writer.flush()?;
            }

            let mut perms = fs::metadata(&entry_path)?.permissions();
            perms.set_mode(mode);
            fs::set_permissions(&entry_path, perms)?;

            installed_files.push(entry_path);
        }

        create_root_links(pending_links)?;
        Ok(())
    };

    let mut installed_files = Vec::new();
    match inner(&mut installed_files) {
        Ok(()) => Ok(installed_files),
        Err(source) => Err(PartialExtractionError {
            source,
            installed_files,
        }
        .into()),
    }
}

/// Run a maintainer script (preinst, postinst, prerm, postrm)
fn run_maintainer_script(script: &Path, package_name: &str, arg: &str) -> Result<()> {
    // Make executable
    let mut perms = fs::metadata(script)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(script, perms)?;

    let status = Command::new(script)
        .arg(arg)
        .env("DPKG_MAINTSCRIPT_PACKAGE", package_name)
        .env("DPKG_MAINTSCRIPT_ARCH", super::debian_arch())
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
    let has_status = control_content
        .lines()
        .any(|line| line.starts_with("Status:"));
    let mut result = String::new();

    for line in control_content.lines() {
        if line.starts_with("Status:") {
            result.push_str("Status: install ok installed\n");
        } else {
            result.push_str(line);
            result.push('\n');
            if !has_status && line.starts_with("Package:") {
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
    expected_size: u64,
) -> Result<PathBuf> {
    // A compromised mirror must not be able to fill the disk: abort once the
    // response doubles the metadata-declared size plus a 1 MiB slack.
    let max_bytes = if expected_size > 0 {
        Some(expected_size.saturating_mul(2).saturating_add(1024 * 1024))
    } else {
        None
    };

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
    match download_streaming_once(client, url, &dest, progress, max_bytes).await {
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
        let backoff =
            Duration::from_millis(INITIAL_BACKOFF_MS.saturating_mul(1 << attempt.min(20)));
        progress.set_message(format!("retry {}/{}", attempt + 1, MAX_DOWNLOAD_RETRIES));
        tokio::time::sleep(backoff).await;

        match download_streaming_once(client, url, &dest, progress, max_bytes).await {
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

/// Abort a download once it grows past the metadata-derived ceiling.
fn enforce_download_cap(downloaded: u64, max_bytes: Option<u64>, url: &str) -> Result<()> {
    if let Some(max) = max_bytes
        && downloaded > max
    {
        anyhow::bail!(
            "Download from {} reached {downloaded} bytes, exceeding the \
             expected maximum of {max} bytes; refusing to fill the disk",
            crate::core::http::redact_url(url)
        );
    }
    Ok(())
}

/// Stream download directly to disk
async fn download_streaming_once(
    client: &reqwest::Client,
    url: &str,
    dest: &Path,
    progress: &ProgressBar,
    max_bytes: Option<u64>,
) -> Result<()> {
    use futures::StreamExt;
    use tokio::io::AsyncWriteExt;

    let safe_url = crate::core::http::redact_url(url);
    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("Failed to request {safe_url}"))?;

    if !response.status().is_success() {
        anyhow::bail!("HTTP {} for {safe_url}", response.status());
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
        let chunk = chunk.with_context(|| format!("Failed to read chunk from {safe_url}"))?;

        // OPTIMIZATION: Batch small chunks into 8KB writes
        write_buffer.extend_from_slice(&chunk);

        // Flush buffer when it reaches 8KB (good balance of memory and syscall efficiency)
        if write_buffer.len() >= 8192 {
            file.write_all(&write_buffer)
                .await
                .with_context(|| format!("Failed to write to {}", dest.display()))?;
            downloaded += write_buffer.len() as u64;
            enforce_download_cap(downloaded, max_bytes, url)?;
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
        enforce_download_cap(downloaded, max_bytes, url)?;
        progress.set_position(downloaded);
    }

    // Clear buffer explicitly to release memory immediately
    drop(write_buffer);

    // Flush but don't fsync (unsafe-io optimization - OS handles durability)
    file.flush().await?;

    Ok(())
}

fn removal_step_failed(
    pb: &ProgressBar,
    overall: &ProgressBar,
    package_name: &str,
    step: &str,
    error: anyhow::Error,
) -> anyhow::Error {
    pb.set_message(format!("{step} failed: {error}").red().to_string());
    pb.finish();
    overall.inc(1);
    // Debug-level here: the propagated chain below carries the full detail to
    // the single boundary that owns user-facing error reporting.
    tracing::debug!(
        target: "pkg_removal",
        package_name = %package_name,
        step = %step,
        "removal step failed: {error:#}"
    );
    error.context(format!(
        "package removal failed during {step} for {package_name}"
    ))
}

const DPKG_INFO_DIR: &str = "/var/lib/dpkg/info";

fn dpkg_info_candidates(package_name: &str, extension: &str) -> [PathBuf; 3] {
    // dpkg architecture names differ from Rust's target ARCH on the most
    // common platform: x86_64 -> amd64 (audit A1). Probe the translated
    // name first, then the raw Rust ARCH, then the unqualified fallback.
    let arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        other => other,
    };
    [
        Path::new(DPKG_INFO_DIR).join(format!("{package_name}:{arch}.{extension}")),
        Path::new(DPKG_INFO_DIR).join(format!(
            "{package_name}:{}.{extension}",
            std::env::consts::ARCH
        )),
        Path::new(DPKG_INFO_DIR).join(format!("{package_name}.{extension}")),
    ]
}

fn existing_dpkg_info_file(package_name: &str, extension: &str) -> Option<PathBuf> {
    dpkg_info_candidates(package_name, extension)
        .into_iter()
        .find(|path| path.exists())
}

fn run_removal_maintainer_script(package_name: &str, kind: &str) -> Result<()> {
    let Some(script) = existing_dpkg_info_file(package_name, kind) else {
        tracing::debug!("No {kind} script found for {package_name}");
        return Ok(());
    };
    run_maintainer_script(&script, package_name, "remove")
}

/// Remove package files from the filesystem
fn remove_package_files(package_name: &str) -> Result<()> {
    let list_path = existing_dpkg_info_file(package_name, "list")
        .ok_or_else(|| anyhow::anyhow!("No .list file found for {package_name}"))?;

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
            dirs_to_remove.push(path);
        } else {
            files_to_remove.push(path);
        }
    }

    files_to_remove.reverse();

    let mut removed_count = 0;

    for file_path in &files_to_remove {
        if !file_path.exists() {
            continue;
        }

        if is_conffile(package_name, file_path)? {
            tracing::debug!("Skipping conffile: {}", file_path.display());
            continue;
        }

        fs::remove_file(file_path)
            .with_context(|| format!("Failed to remove {}", file_path.display()))?;
        removed_count += 1;
        tracing::trace!("Removed: {}", file_path.display());
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

    tracing::debug!("Removed {removed_count} files from {package_name}");

    Ok(())
}

/// Check if a file path is a conffile for the given package
fn is_conffile(package_name: &str, file_path: &Path) -> Result<bool> {
    let Some(conffiles_path) = existing_dpkg_info_file(package_name, "conffiles") else {
        return Ok(false);
    };
    let content = fs::read_to_string(&conffiles_path)
        .with_context(|| format!("Failed to read {}", conffiles_path.display()))?;
    Ok(content
        .lines()
        .any(|line| Path::new(line.trim()) == file_path))
}

/// Update /var/lib/dpkg/status to mark package as removed (config-files state)
fn update_dpkg_status_for_removal(package_name: &str) -> Result<()> {
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

    write_atomic(status_path, updated_content.as_bytes())
        .context("Failed to persist updated status file")?;

    tracing::debug!(
        "Updated dpkg status for {} to config-files state",
        package_name
    );
    Ok(())
}

/// Clean up dpkg info files after removal
fn cleanup_dpkg_info_files(package_name: &str) -> Result<()> {
    let extensions_to_remove = [
        "list", "md5sums", "prerm", "postinst", "preinst", "postrm", "triggers", "shlibs",
        "symbols",
    ];

    let mut removed_count = 0;

    for ext in &extensions_to_remove {
        for file_path in dpkg_info_candidates(package_name, ext) {
            if !file_path.exists() {
                continue;
            }
            fs::remove_file(&file_path)
                .with_context(|| format!("Failed to remove {}", file_path.display()))?;
            removed_count += 1;
            tracing::trace!("Removed: {}", file_path.display());
        }
    }

    tracing::debug!("Cleaned up {removed_count} info files for {package_name} (kept conffiles)");
    Ok(())
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
        let tx = Transaction::new().expect("content store init");
        assert_eq!(tx.state, TransactionState::Pending);
        assert!(tx.to_install.is_empty());
        assert!(tx.to_remove.is_empty());
    }

    #[test]
    fn transaction_steps_reject_missing_private_workspace() {
        let mut transaction = Transaction::new().expect("content store init");

        let error = transaction
            .configure_packages()
            .expect_err("uninitialized transaction must fail closed");
        assert!(
            error
                .to_string()
                .contains("temporary directory is not initialized"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn test_transaction_add_install() {
        let mut tx = Transaction::new().expect("content store init");
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
        let mut tx = Transaction::new().expect("content store init");
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
    fn test_require_package_installed_rejects_missing() {
        Transaction::require_package_installed("vim", true).expect("installed package is valid");
        let error = Transaction::require_package_installed("vim", false)
            .expect_err("not-installed removal must not succeed");
        assert!(
            error.to_string().contains("package vim is not installed"),
            "got: {error}"
        );
    }

    #[test]
    fn test_dpkg_info_candidates_prefer_arch_qualified_name() {
        let [debian_arch, rust_arch, unqualified] = dpkg_info_candidates("curl", "list");
        let expected_arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            other => other,
        };
        assert!(debian_arch.ends_with(format!("curl:{expected_arch}.list")));
        assert!(rust_arch.ends_with(format!("curl:{}.list", std::env::consts::ARCH)));
        assert!(unqualified.ends_with("curl.list"));
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

    #[test]
    fn remove_file_if_present_allows_missing() {
        remove_file_if_present(Path::new("/no/such/rollback/file"))
            .expect("missing installed file is already gone");
    }

    #[test]
    fn remove_file_if_present_deletes_existing() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("installed");
        std::fs::write(&path, b"pkg").expect("installed file");
        remove_file_if_present(&path).expect("rollback remove");
        assert!(!path.exists());
    }

    #[test]
    fn restore_backup_copies_file() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let backup = dir.path().join("backup");
        let original = dir.path().join("original");
        std::fs::write(&backup, b"old").expect("backup");
        restore_backup(&backup, &original).expect("restore");
        assert_eq!(std::fs::read(&original).expect("restored"), b"old");
    }

    #[test]
    fn restore_backup_rejects_missing() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let error = restore_backup(
            &dir.path().join("missing-backup"),
            &dir.path().join("original"),
        )
        .expect_err("missing backup must not look restored");
        assert!(
            error.to_string().contains("Failed to restore backup"),
            "got: {error}"
        );
    }

    #[test]
    fn copy_conffile_rejects_missing_source() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let error = copy_conffile(&dir.path().join("src"), &dir.path().join("dest"))
            .expect_err("missing conffiles must not look copied");
        assert!(
            error.to_string().contains("Failed to copy conffiles"),
            "got: {error}"
        );
    }

    #[test]
    fn copy_conffile_writes_destination() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let src = dir.path().join("src");
        let dest = dir.path().join("dest");
        std::fs::write(&src, b"/etc/foo.conf\n").expect("conffiles");
        copy_conffile(&src, &dest).expect("copy");
        assert_eq!(std::fs::read(&dest).expect("copied"), b"/etc/foo.conf\n");
    }

    // ─── data.tar extraction hardening ───

    /// Build an uncompressed tar with a callback so tests can append
    /// arbitrary entries (files, links, special types).
    fn build_tar(
        append: impl FnOnce(&mut tar::Builder<Vec<u8>>) -> std::io::Result<()>,
    ) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        append(&mut builder).expect("tar fixture");
        builder.into_inner().expect("tar finish")
    }

    fn append_regular_file(builder: &mut tar::Builder<Vec<u8>>, name: &str, body: &[u8]) {
        let mut header = tar::Header::new_gnu();
        header.set_size(body.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, body)
            .expect("append file");
    }

    #[test]
    fn absolute_symlink_target_in_data_tar_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");
        let victim = temp.path().join("victim.txt");

        let data = build_tar(|builder| {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_cksum();
            builder.append_link(&mut header, "./lib", &victim)
        });

        let error = extract_tar_to_root_at(temp.path(), &data)
            .expect_err("absolute symlink target must be rejected");

        assert!(error.to_string().contains("must be relative"), "{error}");
        assert!(!victim.exists(), "attack target must not be touched");
    }

    #[test]
    fn relative_symlink_escaping_the_root_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");

        // Link sits one directory below the root, so three `..` steps escape
        // the extraction root even though every component is relative.
        let data = build_tar(|builder| {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_size(0);
            header.set_cksum();
            builder.append_link(&mut header, "./out", "../../../outside")
        });

        let error = extract_tar_to_root_at(temp.path(), &data)
            .expect_err("escaping symlink must be rejected");

        assert!(
            error.to_string().contains("escapes the extraction"),
            "{error}"
        );
        assert!(!temp.path().join("outside").exists());
        // Nothing was created outside the root either.
        let outside = temp
            .path()
            .parent()
            .expect("tempdir parent")
            .join("outside");
        assert!(!outside.exists());
    }

    #[test]
    fn file_written_after_symlink_cannot_traverse_it() {
        // The attack: ship `./lib -> <relative escape>` and `./lib/secret`.
        // The escaping link is rejected during the scan, so nothing is
        // written anywhere; a regular-file write can never traverse a link
        // defined by the same archive because links are only created after
        // every regular file exists.
        let temp = tempfile::tempdir().expect("tempdir");

        let data = build_tar(|builder| {
            let mut link_header = tar::Header::new_gnu();
            link_header.set_entry_type(tar::EntryType::Symlink);
            link_header.set_size(0);
            link_header.set_cksum();
            // Relative target that resolves outside the extraction root:
            // the link lives at <root>/lib, so two `..` steps leave the root.
            builder.append_link(&mut link_header, "./lib", "../../pwned")?;

            append_regular_file(builder, "./lib/secret", b"pwned");
            Ok(())
        });

        let error = extract_tar_to_root_at(temp.path(), &data)
            .expect_err("escaping symlink must fail the install loudly");

        assert!(
            error.to_string().contains("escapes the extraction"),
            "{error}"
        );
        // Neither the escaped location nor the in-root payload exists.
        let escaped = temp.path().parent().expect("tempdir parent").join("pwned");
        assert!(!escaped.exists(), "nothing may land outside the root");
        assert!(!temp.path().join("lib").exists());
    }

    #[test]
    fn benign_links_are_recreated_and_recorded_for_rollback() {
        let temp = tempfile::tempdir().expect("tempdir");

        let data = build_tar(|builder| {
            append_regular_file(builder, "./usr/share/doc/pkg/readme", b"hello");

            let mut sym_header = tar::Header::new_gnu();
            sym_header.set_entry_type(tar::EntryType::Symlink);
            sym_header.set_size(0);
            sym_header.set_cksum();
            builder.append_link(&mut sym_header, "./usr/share/doc/pkg/latest", "readme")?;

            let mut hard_header = tar::Header::new_gnu();
            hard_header.set_entry_type(tar::EntryType::Link);
            hard_header.set_size(0);
            hard_header.set_cksum();
            builder.append_link(
                &mut hard_header,
                "./usr/share/doc/pkg/dup",
                "./usr/share/doc/pkg/readme",
            )
        });

        let installed =
            extract_tar_to_root_at(temp.path(), &data).expect("benign archive must extract");

        let readme = temp.path().join("usr/share/doc/pkg/readme");
        let latest = temp.path().join("usr/share/doc/pkg/latest");
        let dup = temp.path().join("usr/share/doc/pkg/dup");

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let link_metadata = std::fs::symlink_metadata(&latest).expect("symlink exists");
            assert!(
                link_metadata.file_type().is_symlink(),
                "recreated as symlink"
            );
            assert_eq!(
                std::fs::read_link(&latest).expect("link target"),
                Path::new("readme")
            );

            // Hard link shares the inode of its target.
            let meta = std::fs::metadata(&readme).expect("readme");
            let dup_meta = std::fs::metadata(&dup).expect("dup");
            assert_eq!((meta.dev(), meta.ino()), (dup_meta.dev(), dup_meta.ino()));
        }

        for path in [&readme, &latest, &dup] {
            assert!(
                installed.contains(path),
                "{} must be recorded for rollback",
                path.display()
            );
        }
    }

    #[test]
    fn parent_component_in_data_tar_path_is_rejected() {
        let temp = tempfile::tempdir().expect("tempdir");

        // The tar Builder validates paths, so craft the hostile entry through
        // a raw ustar header: an entry literally named `./share/../evil`.
        let data = build_tar(|builder| {
            let mut header = tar::Header::new_gnu();
            {
                // Bypass Builder path validation: this fixture needs an entry
                // whose NAME carries a `..` component, which is exactly what
                // the extractor must reject.
                let old = header.as_old_mut();
                let mut name = [0u8; 100];
                let hostile = b"./share/../evil";
                name[..hostile.len()].copy_from_slice(hostile);
                old.name = name;
            }
            header.set_entry_type(tar::EntryType::Regular);
            header.set_size(4);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append(&header, &b"nope"[..])
        });

        let error = extract_tar_to_root_at(temp.path(), &data)
            .expect_err("parent components must be rejected");

        assert!(error.to_string().contains("parent component"), "{error}");
        assert!(!temp.path().join("evil").exists());
    }

    #[test]
    fn special_entries_fail_the_install_explicitly() {
        let temp = tempfile::tempdir().expect("tempdir");

        let data = build_tar(|builder| {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Fifo);
            header.set_size(0);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, "./run/pipe", std::io::empty())
        });

        let error = extract_tar_to_root_at(temp.path(), &data).expect_err("FIFO must be rejected");

        assert!(
            error.to_string().contains("Unsupported special entry"),
            "{error}"
        );
    }

    #[test]
    fn rollback_removal_handles_dirs_and_links() {
        let temp = tempfile::tempdir().expect("tempdir");
        let dir = temp.path().join("usr");
        let nested = dir.join("bin");
        std::fs::create_dir_all(&nested).expect("dirs");
        let file = nested.join("tool");
        std::fs::write(&file, b"x").expect("file");
        #[cfg(unix)]
        std::os::unix::fs::symlink("tool", nested.join("tool-link")).expect("link");

        // Recorded order mirrors real archive entry order (parents appear
        // before children in data.tar); removal walks in reverse so the
        // deepest entries go first.
        let mut recorded = vec![dir.clone(), nested.clone(), file.clone()];
        #[cfg(unix)]
        recorded.push(nested.join("tool-link"));

        for path in recorded.iter().rev() {
            remove_file_if_present(path).expect("reverse-order removal");
        }

        assert!(!nested.exists(), "nested dir removed after its children");
        assert!(!dir.exists(), "top dir removed last");
        // Re-running removal is a no-op (rollback idempotence).
        remove_file_if_present(&file).expect("missing path tolerated");
    }

    #[test]
    fn gzipped_payload_bomb_is_bounded_during_streaming() {
        use flate2::write::GzEncoder;

        // 64 MiB of zeros compresses to ~60 KiB.
        let mut encoder = GzEncoder::new(Vec::new(), flate2::Compression::best());
        std::io::copy(
            &mut std::io::repeat(0u8).take(64 * 1024 * 1024),
            &mut encoder,
        )
        .expect("compress");
        let bomb = encoder.finish().expect("finish");

        // A 1 MiB budget: far below the 64 MiB expansion, so the abort must
        // fire mid-stream, long before the full payload is produced.
        const TEST_BUDGET_BYTES: u64 = 1024 * 1024;
        let mut reader =
            tar_payload_reader_with_budget(&bomb, TEST_BUDGET_BYTES).expect("gzip detection");
        let mut sink = Vec::new();
        let error = reader
            .read_to_end(&mut sink)
            .expect_err("budget must abort the stream mid-decompression");

        assert!(error.to_string().contains("exceeds"), "{error}");
        assert!(
            error.to_string().contains("maximum supported size"),
            "{error}"
        );
        // The sink stopped at the budget instead of expanding to 64 MiB.
        assert!(
            sink.len() as u64 <= TEST_BUDGET_BYTES,
            "sink grew to {} bytes, past the {TEST_BUDGET_BYTES}-byte budget",
            sink.len()
        );
    }
    #[test]
    fn merge_status_entries_replaces_existing_paragraph() {
        let current = "Package: vim\nStatus: install ok installed\nVersion: 1.0\n\nPackage: git\nStatus: install ok installed\nVersion: 2.0\n";
        let entries = vec![(
            "vim".to_string(),
            "Package: vim\nStatus: install ok installed\nVersion: 9.9\n".to_string(),
        )];

        let merged = merge_status_entries(current, &entries);

        assert!(merged.contains("Version: 9.9"), "{merged}");
        assert!(
            !merged.contains("Version: 1.0\n"),
            "old paragraph must be replaced: {merged}"
        );
        // Untouched package survives exactly once.
        assert_eq!(merged.matches("Package: git").count(), 1);
        assert_eq!(merged.matches("Package: vim").count(), 1);
        assert!(merged.ends_with('\n'));
    }

    #[test]
    fn merge_status_entries_appends_new_packages() {
        let current = "Package: git\nStatus: install ok installed\nVersion: 2.0\n";
        let entries = vec![(
            "curl".to_string(),
            "Package: curl\nStatus: install ok installed\nVersion: 8.0\n".to_string(),
        )];

        let merged = merge_status_entries(current, &entries);

        assert!(merged.contains("Package: curl"), "{merged}");
        assert!(merged.contains("Package: git"), "{merged}");
        assert_eq!(merged.matches("Package: ").count(), 2);
    }

    #[test]
    fn merge_status_entries_into_empty_status_writes_only_new_entries() {
        let entries = vec![(
            "curl".to_string(),
            "Package: curl\nStatus: install ok installed\nVersion: 8.0\n".to_string(),
        )];

        let merged = merge_status_entries("", &entries);
        assert_eq!(
            merged,
            "Package: curl\nStatus: install ok installed\nVersion: 8.0\n\n"
        );
    }

    // ─── wave-3 fixes ───

    #[tokio::test]
    async fn execute_removal_completes_for_empty_transaction() {
        let mut tx = Transaction::new().expect("content store init");
        tx.execute_removal()
            .await
            .expect("removing nothing must succeed");
    }

    #[tokio::test]
    async fn execute_removal_reports_unknown_package_as_failure() {
        // The blocking-pool route must still propagate per-package validation
        // failures instead of silently completing.
        let mut tx = Transaction::new().expect("content store init");
        tx.add_remove("omg-wave3-definitely-not-installed".to_string());
        let error = tx
            .execute_removal()
            .await
            .expect_err("unknown package must fail loudly");
        assert!(
            error.to_string().contains("not installed")
                || error.to_string().contains("no version")
                || error.to_string().contains("Failed"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn rollback_restores_dpkg_status_even_when_a_path_cannot_be_removed() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let original = dir.path().join("status");
        std::fs::write(&original, b"after-transaction").expect("post-tx status");
        let backup = dir.path().join("status.pre-transaction");
        std::fs::write(&backup, b"pre-transaction").expect("backup status");

        // A directory that still holds foreign content cannot be removed by
        // rollback; that must not prevent the integrity-critical restore.
        let stuck = dir.path().join("shared");
        std::fs::create_dir_all(stuck.join("foreign")).expect("stubborn dir");

        let content_store = ContentStore::with_path(dir.path().join("content-store"));
        content_store.init().expect("content store init");

        let mut tx = Transaction {
            state: TransactionState::Configuring,
            to_install: Vec::new(),
            to_remove: Vec::new(),
            to_upgrade: Vec::new(),
            temp_dir: None,
            backups: HashMap::from([(original.clone(), backup)]),
            installed_files: vec![stuck],
            content_store,
        };

        let error = tx
            .rollback()
            .expect_err("stuck path must be reported as incomplete rollback");
        assert!(error.to_string().contains("Rollback incomplete"), "{error}");
        assert_eq!(
            std::fs::read(&original).expect("restored status"),
            b"pre-transaction",
            "dpkg status must be restored despite the removal failure"
        );
    }

    #[test]
    fn implicit_parent_directories_are_recorded_for_rollback() {
        let temp = tempfile::tempdir().expect("tempdir");

        let data = build_tar(|builder| {
            append_regular_file(builder, "./n1/n2/file", b"payload");
            Ok(())
        });

        let installed =
            extract_tar_to_root_at(temp.path(), &data).expect("extraction must succeed");

        let n1 = temp.path().join("n1");
        let n2 = n1.join("n2");
        let file = n2.join("file");
        for path in [&n1, &n2, &file] {
            assert!(
                installed.contains(path),
                "{} must be recorded",
                path.display()
            );
        }

        // Reverse-order removal (as rollback does) unwinds the whole chain.
        for path in installed.iter().rev() {
            remove_file_if_present(path).expect("reverse removal");
        }
        assert!(!n1.exists(), "implicit parent chain fully removed");
    }
}
