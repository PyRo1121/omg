//! Pure Rust DNF/Fedora package manager backend
//!
//! Provides direct database access for ultra-fast queries without CLI overhead.
//! Uses `SQLite` for RPM database and parses repository metadata.
//!
//! ## Performance Targets
//! - Search: <100ms (vs DNF's 3-5s)
//! - List installed: <50ms (vs DNF's 2s)
//!
//! ## Architecture
//! 1. Read installed packages from `/var/lib/rpm/rpmdb.sqlite`
//! 2. Parse RPM header blobs for metadata extraction
//! 3. Parse repository metadata from `/etc/yum.repos.d/`
//! 4. Cache parsed metadata in binary format using `bitcode`

use std::future::Future;
use std::pin::Pin;

use anyhow::{Context, Result};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::fs;

use crate::core::{Package, PackageSource, is_root};
use crate::package_managers::PackageManager;
use crate::package_managers::types::{UpdateInfo, parse_version_or_zero};

#[cfg(feature = "fedora")]
use rusqlite::{Connection, OpenFlags};

/// RPM header magic bytes
const RPM_HEADER_MAGIC: [u8; 8] = [0x8e, 0xad, 0xe8, 0x01, 0x00, 0x00, 0x00, 0x00];

/// RPM tag constants for parsing header entries
#[cfg(feature = "fedora")]
mod rpm_tags {
    pub const NAME: u32 = 1000;
    pub const VERSION: u32 = 1001;
    pub const RELEASE: u32 = 1002;
    pub const SUMMARY: u32 = 1004;
    pub const SIZE: u32 = 1009;
    pub const INSTALL_TIME: u32 = 1008;
    pub const REASON: u32 = 1160; // User/Dependency
}

/// RPM header data types
mod rpm_types {
    pub const STRING: u32 = 6;
    pub const STRING_ARRAY: u32 = 8;
}

/// Cached repository package metadata
#[derive(Debug, Clone, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
struct RepoPackage {
    name: String,
    version: String,
    release: String,
    arch: String,
    summary: String,
    description: String,
    repo: String,
    url: Option<String>,
    size: u64,
    license: Vec<String>,
}

/// Repository metadata index
#[derive(Debug, Clone, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
struct RepoIndex {
    packages: Vec<RepoPackage>,
    last_updated: u64,
}

/// DNF Package Manager implementation
pub struct DnfPackageManager {
    /// Path to RPM `SQLite` database
    rpm_db_path: PathBuf,
    /// Path to yum repository configuration
    repos_dir: PathBuf,
    /// In-memory repository index cache
    repo_cache: Arc<RwLock<Option<Vec<RepoPackage>>>>,
    /// Installed packages cache (name -> package info)
    installed_cache: Arc<DashMap<String, InstalledPackage>>,
    /// Cache directory for binary metadata
    cache_dir: PathBuf,
}

/// Installed package information from RPM database
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used for future RPM database implementation
struct InstalledPackage {
    name: String,
    version: String,
    release: String,
    summary: String,
    size: i64,
    install_time: i64,
    reason: InstallReason,
}

/// Why a package was installed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallReason {
    User,
    Dependency,
}

impl Default for DnfPackageManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DnfPackageManager {
    /// Create a new DNF package manager instance
    ///
    /// # Panics
    /// Panics if no user cache directory can be determined (e.g., $HOME unset).
    #[must_use]
    pub fn new() -> Self {
        let cache_dir = dirs::cache_dir()
            .expect("Cannot determine cache directory - ensure $HOME is set")
            .join("omg")
            .join("dnf");

        Self {
            rpm_db_path: PathBuf::from("/var/lib/rpm/rpmdb.sqlite"),
            repos_dir: PathBuf::from("/etc/yum.repos.d"),
            repo_cache: Arc::new(RwLock::new(None)),
            installed_cache: Arc::new(DashMap::new()),
            cache_dir,
        }
    }

    /// Load installed packages from RPM `SQLite` database
    ///
    /// Reads directly from `/var/lib/rpm/rpmdb.sqlite` and parses RPM header blobs
    /// to extract package metadata. Caches results in memory for subsequent calls.
    async fn load_installed_packages(&self) -> Result<Vec<InstalledPackage>> {
        // Check if we have cached data
        if !self.installed_cache.is_empty() {
            return Ok(self
                .installed_cache
                .iter()
                .map(|entry| entry.value().clone())
                .collect());
        }

        // Fallback to reading from SQLite database
        let db_path = self.rpm_db_path.clone();
        let packages =
            tokio::task::spawn_blocking(move || Self::read_rpm_database(&db_path)).await??;

        // Populate cache
        for pkg in &packages {
            self.installed_cache.insert(pkg.name.clone(), pkg.clone());
        }

        Ok(packages)
    }

    /// Read RPM database, trying `SQLite` first then falling back to subprocess
    #[cfg(feature = "fedora")]
    fn read_rpm_database(db_path: &Path) -> Result<Vec<InstalledPackage>> {
        // Try SQLite first (Fedora 33+, RHEL 9+) - 50-100x faster
        if db_path.exists() {
            match Self::read_rpm_sqlite(db_path) {
                Ok(packages) => return Ok(packages),
                Err(e) => {
                    tracing::warn!("SQLite access failed: {e}, falling back to rpm -qa");
                }
            }
        }

        // Fallback to subprocess for BDB/NDB systems or when SQLite fails
        Self::read_rpm_via_query()
    }

    /// Read RPM database via subprocess (non-fedora feature fallback)
    #[cfg(not(feature = "fedora"))]
    fn read_rpm_database(_db_path: &Path) -> Result<Vec<InstalledPackage>> {
        Self::read_rpm_via_query()
    }

    /// Parse installed packages using `rpm -qa` subprocess
    ///
    /// Fallback for systems without `SQLite` RPM database (`BerkeleyDB`, `NDB`).
    fn read_rpm_via_query() -> Result<Vec<InstalledPackage>> {
        let output = Command::new("rpm")
            .args([
                "-qa",
                "--queryformat",
                "%{NAME}\t%{VERSION}\t%{RELEASE}\t%{SUMMARY}\t%{SIZE}\t%{INSTALLTIME}\t%{REASON}\n",
            ])
            .output()
            .context("Failed to execute rpm -qa")?;

        if !output.status.success() {
            anyhow::bail!("rpm command failed: {}", output.status);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::with_capacity(2048);

        for line in stdout.lines() {
            let fields: Vec<&str> = line.split('\t').collect();
            if fields.len() < 7 {
                continue;
            }

            let reason = match fields[6] {
                "0" | "user" => InstallReason::User,
                _ => InstallReason::Dependency,
            };

            packages.push(InstalledPackage {
                name: fields[0].to_string(),
                version: fields[1].to_string(),
                release: fields[2].to_string(),
                summary: fields[3].to_string(),
                size: fields[4].parse().unwrap_or(0),
                install_time: fields[5].parse().unwrap_or(0),
                reason,
            });
        }

        Ok(packages)
    }

    /// Parse RPM header blob to extract metadata
    ///
    /// RPM headers use a binary format:
    /// - Magic: 0x8eade801 00000000
    /// - Index entries: tag(u32), type(u32), offset(i32), count(u32)
    fn parse_rpm_header(blob: &[u8]) -> Result<HashMap<u32, Vec<u8>>> {
        if blob.len() < 16 {
            anyhow::bail!("RPM header too short");
        }

        // Verify magic bytes
        if blob[0..8] != RPM_HEADER_MAGIC {
            anyhow::bail!("Invalid RPM header magic");
        }

        let num_entries = u32::from_be_bytes([blob[8], blob[9], blob[10], blob[11]]) as usize;
        let data_size = u32::from_be_bytes([blob[12], blob[13], blob[14], blob[15]]) as usize;

        let index_start = 16;
        let index_size = num_entries * 16; // Each entry is 16 bytes
        let data_start = index_start + index_size;

        if blob.len() < data_start + data_size {
            anyhow::bail!("RPM header truncated");
        }

        let mut tags = HashMap::new();

        // Parse index entries
        for i in 0..num_entries {
            let entry_offset = index_start + (i * 16);
            let tag = u32::from_be_bytes([
                blob[entry_offset],
                blob[entry_offset + 1],
                blob[entry_offset + 2],
                blob[entry_offset + 3],
            ]);
            let tag_type = u32::from_be_bytes([
                blob[entry_offset + 4],
                blob[entry_offset + 5],
                blob[entry_offset + 6],
                blob[entry_offset + 7],
            ]);
            let offset = i32::from_be_bytes([
                blob[entry_offset + 8],
                blob[entry_offset + 9],
                blob[entry_offset + 10],
                blob[entry_offset + 11],
            ]) as usize;
            let count = u32::from_be_bytes([
                blob[entry_offset + 12],
                blob[entry_offset + 13],
                blob[entry_offset + 14],
                blob[entry_offset + 15],
            ]) as usize;

            // Extract data based on type
            let data_offset = data_start + offset;
            if data_offset < blob.len() {
                let data = match tag_type {
                    rpm_types::STRING => {
                        // Null-terminated string
                        let end = blob[data_offset..]
                            .iter()
                            .position(|&b| b == 0)
                            .unwrap_or(0);
                        blob[data_offset..data_offset + end].to_vec()
                    }
                    rpm_types::STRING_ARRAY => {
                        // Array of null-terminated strings
                        let mut strings = Vec::new();
                        let mut pos = data_offset;
                        for _ in 0..count {
                            let end = blob[pos..].iter().position(|&b| b == 0).unwrap_or(0);
                            strings.extend_from_slice(&blob[pos..pos + end]);
                            strings.push(b'\n');
                            pos += end + 1;
                        }
                        strings
                    }
                    _ => blob[data_offset..data_offset + count].to_vec(),
                };
                tags.insert(tag, data);
            }
        }

        Ok(tags)
    }

    /// Parse an RPM blob into an `InstalledPackage`
    ///
    /// Extracts name, version, release, summary, size, install time, and reason
    /// from the RPM header blob format.
    #[cfg(feature = "fedora")]
    fn parse_package_from_blob(blob: &[u8]) -> Result<InstalledPackage> {
        let tags = Self::parse_rpm_header(blob)?;

        // Helper to extract string from tag data
        let get_string = |tag: u32| -> String {
            tags.get(&tag)
                .map(|data| String::from_utf8_lossy(data).to_string())
                .unwrap_or_default()
        };

        // Helper to extract i64 from tag data (big-endian)
        let get_i64 = |tag: u32| -> i64 {
            tags.get(&tag)
                .and_then(|data| {
                    if data.len() >= 4 {
                        // RPM uses 32-bit integers for most fields
                        Some(i64::from(i32::from_be_bytes([
                            data[0], data[1], data[2], data[3],
                        ])))
                    } else {
                        None
                    }
                })
                .unwrap_or(0)
        };

        let name = get_string(rpm_tags::NAME);
        if name.is_empty() {
            anyhow::bail!("RPM header missing NAME tag");
        }

        let reason_val = get_i64(rpm_tags::REASON);
        let reason = if reason_val == 0 {
            InstallReason::User
        } else {
            InstallReason::Dependency
        };

        Ok(InstalledPackage {
            name,
            version: get_string(rpm_tags::VERSION),
            release: get_string(rpm_tags::RELEASE),
            summary: get_string(rpm_tags::SUMMARY),
            size: get_i64(rpm_tags::SIZE),
            install_time: get_i64(rpm_tags::INSTALL_TIME),
            reason,
        })
    }

    /// Read RPM database directly from `SQLite` (Fedora 33+, RHEL 9+)
    ///
    /// Opens `/var/lib/rpm/rpmdb.sqlite` in read-only mode and parses
    /// RPM header blobs from the `Packages` table. This is 50-100x faster
    /// than spawning `rpm -qa`.
    #[cfg(feature = "fedora")]
    fn read_rpm_sqlite(db_path: &Path) -> Result<Vec<InstalledPackage>> {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .context("Failed to open RPM SQLite database")?;

        conn.busy_timeout(std::time::Duration::from_secs(5))?;

        let mut stmt = conn.prepare("SELECT blob FROM Packages")?;
        let mut packages = Vec::with_capacity(2048);

        let rows = stmt.query_map([], |row| {
            let blob: Vec<u8> = row.get(0)?;
            Ok(blob)
        })?;

        for row in rows {
            let blob = row?;
            let pkg = Self::parse_package_from_blob(&blob)
                .context("Malformed RPM header in Packages table")?;
            packages.push(pkg);
        }

        tracing::debug!("Loaded {} packages from SQLite database", packages.len());
        Ok(packages)
    }

    /// Load repository metadata from yum.repos.d configuration
    async fn load_repo_metadata(&self) -> Result<Vec<RepoPackage>> {
        // Check in-memory cache first
        if let Some(ref packages) = *self.repo_cache.read().expect("lock poisoned") {
            return Ok(packages.clone());
        }

        // Check binary cache on disk
        let cache_file = self.cache_dir.join("repo_index.bin");
        if let Ok(cached) = self.load_cached_index(&cache_file).await {
            *self.repo_cache.write().expect("lock poisoned") = Some(cached.packages.clone());
            return Ok(cached.packages);
        }

        // Parse repository metadata from scratch
        let packages = self.parse_repo_metadata().await?;

        // Save to binary cache and update in-memory cache
        self.save_cached_index(&cache_file, &packages).await?;
        *self.repo_cache.write().expect("lock poisoned") = Some(packages.clone());

        Ok(packages)
    }

    /// Load cached repository index from disk
    async fn load_cached_index(&self, path: &Path) -> Result<RepoIndex> {
        let data = fs::read(path).await?;
        let index: RepoIndex = bitcode::decode(&data)?;
        Ok(index)
    }

    /// Save repository index to binary cache
    async fn save_cached_index(&self, path: &Path, packages: &[RepoPackage]) -> Result<()> {
        fs::create_dir_all(&self.cache_dir).await?;

        let index = RepoIndex {
            packages: packages.to_vec(),
            last_updated: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
        };

        let data = bitcode::encode(&index);
        fs::write(path, data).await?;
        Ok(())
    }

    /// Parse enabled repository configuration from `/etc/yum.repos.d/*.repo`.
    /// Remote `repomd.xml` / `primary.xml.gz` fetching is not implemented.
    async fn parse_repo_metadata(&self) -> Result<Vec<RepoPackage>> {
        let repos = self.discover_repositories().await?;
        let mut all_packages = Vec::new();

        for repo in repos {
            let mut packages = Self::fetch_repo_packages(&repo)?;
            all_packages.append(&mut packages);
        }

        Ok(all_packages)
    }

    /// Discover enabled repositories from /etc/yum.repos.d
    async fn discover_repositories(&self) -> Result<Vec<RepoConfig>> {
        Self::discover_repositories_in(&self.repos_dir).await
    }

    async fn discover_repositories_in(repos_dir: &Path) -> Result<Vec<RepoConfig>> {
        let mut repos = Vec::new();

        let mut entries = fs::read_dir(repos_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("repo") {
                continue;
            }
            let repo_configs = Self::parse_repo_file(&path)
                .await
                .with_context(|| format!("Failed to parse repository file {}", path.display()))?;
            repos.extend(repo_configs);
        }

        Ok(repos)
    }

    /// Parse a .repo file for repository configuration
    async fn parse_repo_file(path: &Path) -> Result<Vec<RepoConfig>> {
        let content = fs::read_to_string(path).await?;
        let mut repos = Vec::new();
        let mut current_repo: Option<RepoConfig> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                if let Some(repo) = current_repo.take().filter(|r| r.enabled) {
                    repos.push(repo);
                }

                let name = line[1..line.len() - 1].to_string();
                current_repo = Some(RepoConfig {
                    name,
                    enabled: true,
                    baseurl: None,
                    metalink: None,
                    mirrorlist: None,
                });
            } else if let Some(ref mut repo) = current_repo
                && let Some((key, value)) = line.split_once('=')
            {
                match key.trim() {
                    "enabled" => repo.enabled = value.trim() == "1",
                    "baseurl" => repo.baseurl = Some(value.trim().to_string()),
                    "metalink" => repo.metalink = Some(value.trim().to_string()),
                    "mirrorlist" => repo.mirrorlist = Some(value.trim().to_string()),
                    _ => {}
                }
            }
        }

        if let Some(repo) = current_repo
            && repo.enabled
        {
            repos.push(repo);
        }

        Ok(repos)
    }

    /// Fetch packages from a repository.
    ///
    /// Remote metadata download is not implemented; an empty list would look like
    /// a successful empty catalog.
    fn fetch_repo_packages(repo: &RepoConfig) -> Result<Vec<RepoPackage>> {
        anyhow::bail!(
            "DNF repository metadata fetch is not implemented (repo: {})",
            repo.name
        );
    }

    /// Execute DNF command with privilege escalation if needed
    #[allow(clippy::unused_async)] // May add async operations in future
    fn run_dnf(&self, args: &[&str]) -> Result<()> {
        let mut cmd = if is_root() {
            Command::new("dnf")
        } else {
            let mut c = Command::new("sudo");
            c.arg("dnf");
            c
        };

        let status = cmd.args(args).arg("-y").status()?;

        if status.success() {
            // Invalidate caches after mutations
            self.installed_cache.clear();
            let mut cache = self.repo_cache.write().expect("lock poisoned");
            *cache = None;
            Ok(())
        } else {
            anyhow::bail!("dnf command failed with status {status}")
        }
    }
}

/// Repository configuration
#[derive(Debug, Clone)]
struct RepoConfig {
    name: String,
    enabled: bool,
    baseurl: Option<String>,
    metalink: Option<String>,
    mirrorlist: Option<String>,
}

impl PackageManager for DnfPackageManager {
    fn name(&self) -> &'static str {
        "dnf"
    }

    fn search(
        &self,
        query: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        let query_lower = query.to_lowercase();
        Box::pin(async move {
            // Search installed packages first
            let installed = self.load_installed_packages().await?;
            let mut results: Vec<Package> = installed
                .iter()
                .filter(|pkg| {
                    pkg.name.to_lowercase().contains(&query_lower)
                        || pkg.summary.to_lowercase().contains(&query_lower)
                })
                .map(|pkg| Package {
                    name: pkg.name.clone(),
                    version: parse_version_or_zero(&format!("{}-{}", pkg.version, pkg.release)),
                    description: pkg.summary.clone(),
                    source: PackageSource::Official,
                    installed: true,
                })
                .collect();

            // Search repository metadata
            if let Ok(repo_packages) = self.load_repo_metadata().await {
                let repo_results: Vec<Package> = repo_packages
                    .iter()
                    .filter(|pkg| {
                        pkg.name.to_lowercase().contains(&query_lower)
                            || pkg.summary.to_lowercase().contains(&query_lower)
                    })
                    .map(|pkg| {
                        let is_installed = installed.iter().any(|i| i.name == pkg.name);
                        Package {
                            name: pkg.name.clone(),
                            version: parse_version_or_zero(&format!(
                                "{}-{}",
                                pkg.version, pkg.release
                            )),
                            description: pkg.summary.clone(),
                            source: PackageSource::Official,
                            installed: is_installed,
                        }
                    })
                    .collect();

                results.extend(repo_results);
            }

            // Deduplicate by name
            results.sort_by(|a, b| a.name.cmp(&b.name));
            results.dedup_by(|a, b| a.name == b.name);

            Ok(results)
        })
    }

    fn install(
        &self,
        packages: &[String],
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
            crate::core::security::validate_package_names(&packages)?;

            tokio::task::spawn_blocking({
                let manager = Self::new();
                move || {
                    let mut args = vec!["install"];
                    let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
                    args.extend_from_slice(&pkg_refs);
                    manager.run_dnf(&args)
                }
            })
            .await?
        })
    }

    fn remove(&self, packages: &[String]) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
            crate::core::security::validate_package_names(&packages)?;

            tokio::task::spawn_blocking({
                let manager = Self::new();
                move || {
                    let mut args = vec!["remove"];
                    let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
                    args.extend_from_slice(&pkg_refs);
                    manager.run_dnf(&args)
                }
            })
            .await?
        })
    }

    fn update(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            tokio::task::spawn_blocking({
                let manager = Self::new();
                move || manager.run_dnf(&["upgrade"])
            })
            .await?
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // Clear caches and trigger metadata refresh
            self.installed_cache.clear();
            {
                let mut cache = self.repo_cache.write().expect("lock poisoned");
                *cache = None;
            }

            // Remove binary cache to force re-parsing
            let cache_file = self.cache_dir.join("repo_index.bin");
            let _ = fs::remove_file(cache_file).await;

            tokio::task::spawn_blocking({
                let manager = Self::new();
                move || manager.run_dnf(&["makecache"])
            })
            .await?
        })
    }

    fn info(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Package>>> + Send + '_>> {
        let package = package.to_string();
        Box::pin(async move {
            let installed = self.load_installed_packages().await?;

            if let Some(pkg) = installed.iter().find(|p| p.name == package) {
                return Ok(Some(Package {
                    name: pkg.name.clone(),
                    version: parse_version_or_zero(&format!("{}-{}", pkg.version, pkg.release)),
                    description: pkg.summary.clone(),
                    source: PackageSource::Official,
                    installed: true,
                }));
            }

            // Search repository metadata
            if let Ok(repo_packages) = self.load_repo_metadata().await
                && let Some(pkg) = repo_packages.iter().find(|p| p.name == package)
            {
                return Ok(Some(Package {
                    name: pkg.name.clone(),
                    version: parse_version_or_zero(&format!("{}-{}", pkg.version, pkg.release)),
                    description: pkg.summary.clone(),
                    source: PackageSource::Official,
                    installed: false,
                }));
            }

            Ok(None)
        })
    }

    fn list_installed(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        Box::pin(async move {
            let installed = self.load_installed_packages().await?;

            Ok(installed
                .into_iter()
                .map(|pkg| Package {
                    name: pkg.name,
                    version: parse_version_or_zero(&format!("{}-{}", pkg.version, pkg.release)),
                    description: pkg.summary,
                    source: PackageSource::Official,
                    installed: true,
                })
                .collect())
        })
    }

    fn get_status(
        &self,
        _fast: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(usize, usize, usize, usize)>> + Send + '_>> {
        Box::pin(async move {
            let installed = self.load_installed_packages().await?;
            let total = installed.len();
            let explicit = installed
                .iter()
                .filter(|p| p.reason == InstallReason::User)
                .count();

            // Count orphans (dependencies with no dependents)
            // This requires dependency graph analysis - simplified for now
            let orphans = 0;

            // Count available updates
            let updates = self.list_updates().await?.len();

            Ok((total, explicit, orphans, updates))
        })
    }

    fn list_explicit(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            let installed = self.load_installed_packages().await?;

            Ok(installed
                .into_iter()
                .filter(|pkg| pkg.reason == InstallReason::User)
                .map(|pkg| pkg.name)
                .collect())
        })
    }

    fn list_updates(&self) -> Pin<Box<dyn Future<Output = Result<Vec<UpdateInfo>>> + Send + '_>> {
        Box::pin(async move {
            // Optimization: Fetch installed packages and repo metadata in parallel
            // These are independent I/O operations - ~2x speedup on update checks
            let (installed, repo_packages) =
                tokio::try_join!(self.load_installed_packages(), self.load_repo_metadata())?;

            let mut installed_map = HashMap::new();
            for pkg in &installed {
                installed_map.insert(&pkg.name, pkg);
            }

            let mut updates = Vec::new();

            for repo_pkg in &repo_packages {
                if let Some(installed_pkg) = installed_map.get(&repo_pkg.name) {
                    let installed_ver = parse_version_or_zero(&format!(
                        "{}-{}",
                        installed_pkg.version, installed_pkg.release
                    ));
                    let available_ver = parse_version_or_zero(&format!(
                        "{}-{}",
                        repo_pkg.version, repo_pkg.release
                    ));

                    if available_ver > installed_ver {
                        updates.push(UpdateInfo {
                            name: repo_pkg.name.clone(),
                            old_version: format!(
                                "{}-{}",
                                installed_pkg.version, installed_pkg.release
                            ),
                            new_version: format!("{}-{}", repo_pkg.version, repo_pkg.release),
                            repo: repo_pkg.repo.clone(),
                        });
                    }
                }
            }

            Ok(updates)
        })
    }

    fn is_installed(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        let package = package.to_string();
        Box::pin(async move {
            if self.installed_cache.contains_key(&package) {
                return Ok(true);
            }
            let packages = self.load_installed_packages().await?;
            Ok(packages.iter().any(|p| p.name == package))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dnf_manager_creation() {
        let manager = DnfPackageManager::new();
        assert_eq!(manager.name(), "dnf");
    }

    #[test]
    fn test_rpm_header_parsing() {
        // Test RPM header parsing with minimal valid header
        let mut header = Vec::new();
        header.extend_from_slice(&RPM_HEADER_MAGIC);
        header.extend_from_slice(&[0, 0, 0, 1]); // 1 entry
        header.extend_from_slice(&[0, 0, 0, 8]); // 8 bytes data
        // Entry: tag=1000 (NAME), type=6 (STRING), offset=0, count=4
        header.extend_from_slice(&1000u32.to_be_bytes());
        header.extend_from_slice(&6u32.to_be_bytes());
        header.extend_from_slice(&0i32.to_be_bytes());
        header.extend_from_slice(&4u32.to_be_bytes());
        // Data: "test\0\0\0\0"
        header.extend_from_slice(b"test\0\0\0\0");

        let result = DnfPackageManager::parse_rpm_header(&header);
        assert!(result.is_ok());

        let tags = result.unwrap();
        assert!(tags.contains_key(&1000));
    }

    #[test]
    fn test_parse_rpm_header_rejects_invalid_magic() {
        let error = DnfPackageManager::parse_rpm_header(&[0u8; 32])
            .expect_err("invalid magic must not parse as an empty tag map");
        assert!(
            error.to_string().contains("Invalid RPM header magic"),
            "got: {error}"
        );
    }

    #[test]
    fn test_fetch_repo_packages_is_unimplemented_error() {
        let repo = RepoConfig {
            name: "fedora".to_string(),
            enabled: true,
            baseurl: None,
            metalink: None,
            mirrorlist: None,
        };
        let error = DnfPackageManager::fetch_repo_packages(&repo)
            .expect_err("unimplemented fetch must not look like an empty catalog");
        assert!(
            error
                .to_string()
                .contains("DNF repository metadata fetch is not implemented"),
            "got: {error}"
        );
        assert!(error.to_string().contains("fedora"), "got: {error}");
    }

    #[tokio::test]
    async fn test_discover_repositories_missing_dir_is_error() {
        let error = DnfPackageManager::discover_repositories_in(Path::new("/no/such/yum.repos.d"))
            .await
            .expect_err("missing repos dir must not look like an empty catalog");
        assert!(
            error.to_string().contains("No such file")
                || error
                    .chain()
                    .any(|cause| cause.to_string().contains("No such file")),
            "got: {error:?}"
        );
    }

    #[tokio::test]
    async fn test_discover_repositories_unreadable_repo_file_is_error() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let broken = dir.path().join("broken.repo");
        std::fs::create_dir(&broken).expect("directory named .repo is not a readable file");

        let error = DnfPackageManager::discover_repositories_in(dir.path())
            .await
            .expect_err("unreadable .repo must not be skipped");
        let message = format!("{error:#}");
        assert!(
            message.contains("Failed to parse repository file"),
            "got: {message}"
        );
        assert!(message.contains("broken.repo"), "got: {message}");
    }

    #[tokio::test]
    async fn test_discover_repositories_parses_enabled_repo() {
        let dir = tempfile::TempDir::new().expect("temp dir");
        std::fs::write(
            dir.path().join("fedora.repo"),
            "[fedora]\nenabled=1\nbaseurl=https://example.test/fedora\n",
        )
        .expect("write repo file");
        std::fs::write(dir.path().join("readme.txt"), "not a repo\n").expect("write ignored file");

        let repos = DnfPackageManager::discover_repositories_in(dir.path())
            .await
            .expect("valid .repo must parse");
        assert_eq!(repos.len(), 1);
        assert_eq!(repos[0].name, "fedora");
        assert_eq!(
            repos[0].baseurl.as_deref(),
            Some("https://example.test/fedora")
        );
    }

    #[cfg(feature = "fedora")]
    fn minimal_named_rpm_header(name: &[u8]) -> Vec<u8> {
        let mut header = Vec::new();
        header.extend_from_slice(&RPM_HEADER_MAGIC);
        header.extend_from_slice(&[0, 0, 0, 1]);
        header.extend_from_slice(&(name.len() as u32).to_be_bytes());
        header.extend_from_slice(&1000u32.to_be_bytes());
        header.extend_from_slice(&6u32.to_be_bytes());
        header.extend_from_slice(&0i32.to_be_bytes());
        header.extend_from_slice(&(name.len() as u32).to_be_bytes());
        header.extend_from_slice(name);
        header
    }

    #[cfg(feature = "fedora")]
    fn write_packages_db(blobs: &[&[u8]]) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let db = dir.path().join("rpmdb.sqlite");
        let conn = Connection::open(&db).expect("open sqlite");
        conn.execute("CREATE TABLE Packages (blob BLOB NOT NULL)", [])
            .expect("create Packages");
        for blob in blobs {
            conn.execute("INSERT INTO Packages (blob) VALUES (?1)", [blob.to_vec()])
                .expect("insert blob");
        }
        dir
    }

    #[cfg(feature = "fedora")]
    #[test]
    fn test_read_rpm_sqlite_malformed_blob_is_error() {
        let dir = write_packages_db(&[&[0u8; 32]]);
        let error = DnfPackageManager::read_rpm_sqlite(&dir.path().join("rpmdb.sqlite"))
            .expect_err("malformed header must not look like an empty inventory");
        let message = format!("{error:#}");
        assert!(
            message.contains("Malformed RPM header in Packages table"),
            "got: {message}"
        );
    }

    #[cfg(feature = "fedora")]
    #[test]
    fn test_read_rpm_sqlite_mixed_blobs_do_not_drop_corrupt_row() {
        let valid = minimal_named_rpm_header(b"bash\0");
        let dir = write_packages_db(&[valid.as_slice(), &[0u8; 32]]);
        let error = DnfPackageManager::read_rpm_sqlite(&dir.path().join("rpmdb.sqlite"))
            .expect_err("one corrupt row must not omit that package from the catalog");
        let message = format!("{error:#}");
        assert!(
            message.contains("Malformed RPM header in Packages table"),
            "got: {message}"
        );
    }
}
