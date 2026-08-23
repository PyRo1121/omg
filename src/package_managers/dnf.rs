//! Pure Rust DNF/Fedora package manager backend
//!
//! Reads installed packages directly from the RPM `SQLite` database for
//! fast queries without CLI overhead.
//!
//! ## Architecture
//! 1. Read installed packages from `/var/lib/rpm/rpmdb.sqlite`
//!    (`rpm -qa` subprocess fallback for BDB/NDB systems)
//! 2. Parse RPM header blobs for metadata extraction
//!
//! ## Known limitation
//!
//! Repository metadata access (repomd/primary.xml) is not integrated:
//! `search` and `info` cover installed packages only, and `list_updates`
//! fails explicitly instead of reporting a fake empty update set.
//! Transactions delegate to the `dnf` CLI.

use std::future::Future;
use std::pin::Pin;

use anyhow::{Context, Result};
use dashmap::DashMap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use crate::core::{Package, PackageSource, is_root};
use crate::package_managers::PackageManager;
use crate::package_managers::types::{UpdateInfo, parse_version_or_zero};

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
    pub const REASON: u32 = 1160; // User/Dependency
}

/// RPM header data types
mod rpm_types {
    pub const STRING: u32 = 6;
    pub const STRING_ARRAY: u32 = 8;
}

/// DNF Package Manager implementation
pub struct DnfPackageManager {
    /// Path to RPM `SQLite` database
    rpm_db_path: PathBuf,
    /// Path to yum repository configuration (used by the `dnf` CLI)
    repos_dir: PathBuf,
    /// Installed packages cache (name -> package info)
    installed_cache: Arc<DashMap<String, InstalledPackage>>,
}

/// Installed package information from RPM database
#[derive(Debug, Clone)]
struct InstalledPackage {
    name: String,
    version: String,
    release: String,
    summary: String,
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            rpm_db_path: PathBuf::from("/var/lib/rpm/rpmdb.sqlite"),
            repos_dir: PathBuf::from("/etc/yum.repos.d"),
            installed_cache: Arc::new(DashMap::new()),
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

    /// Parse installed packages using `rpm -qa` subprocess
    ///
    /// Fallback for systems without `SQLite` RPM database (`BerkeleyDB`, `NDB`).
    fn read_rpm_via_query() -> Result<Vec<InstalledPackage>> {
        let output = Command::new("rpm")
            .args([
                "-qa",
                "--queryformat",
                "%{NAME}\t%{VERSION}\t%{RELEASE}\t%{SUMMARY}\t%{REASON}\n",
            ])
            .output()
            .context("Failed to execute rpm -qa")?;

        if !output.status.success() {
            anyhow::bail!("rpm command failed: {}", output.status);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut packages = Vec::with_capacity(2048);

        for line in stdout.lines() {
            if line.is_empty() {
                continue;
            }
            packages.push(Self::parse_rpm_qa_line(line)?);
        }

        Ok(packages)
    }

    fn parse_rpm_qa_line(line: &str) -> Result<InstalledPackage> {
        let fields: Vec<&str> = line.split('\t').collect();
        if fields.len() < 5 {
            anyhow::bail!(
                "malformed rpm -qa output: expected 5 fields, got {}",
                fields.len()
            );
        }

        let reason = match fields[4] {
            "0" | "user" => InstallReason::User,
            _ => InstallReason::Dependency,
        };

        Ok(InstalledPackage {
            name: fields[0].to_string(),
            version: fields[1].to_string(),
            release: fields[2].to_string(),
            summary: fields[3].to_string(),
            reason,
        })
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

        let index_start: usize = 16;
        // Checked arithmetic: num_entries/data_size come from the archive and
        // must not overflow the offsets on any target.
        let index_size = num_entries
            .checked_mul(16)
            .ok_or_else(|| anyhow::anyhow!("RPM header index size overflow"))?;
        let data_start = index_start
            .checked_add(index_size)
            .ok_or_else(|| anyhow::anyhow!("RPM header index size overflow"))?;
        let expected_len = data_start
            .checked_add(data_size)
            .ok_or_else(|| anyhow::anyhow!("RPM header data size overflow"))?;

        if blob.len() < expected_len {
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
                        // Array of null-terminated strings; `count` comes from
                        // the archive, so every read is bounds-checked — a
                        // hostile count must error, never panic.
                        let mut strings = Vec::new();
                        let mut pos = data_offset;
                        for _ in 0..count {
                            if pos >= blob.len() {
                                anyhow::bail!("RPM STRING_ARRAY exceeds payload bounds");
                            }
                            let end = blob[pos..]
                                .iter()
                                .position(|&b| b == 0)
                                .unwrap_or(blob.len() - pos);
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
    /// Extracts name, version, release, summary, and installation reason from
    /// the RPM header blob format.
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
            reason,
        })
    }

    /// Read RPM database directly from `SQLite` (Fedora 33+, RHEL 9+)
    ///
    /// Opens `/var/lib/rpm/rpmdb.sqlite` in read-only mode and parses
    /// RPM header blobs from the `Packages` table. This is 50-100x faster
    /// than spawning `rpm -qa`.
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

    /// A handle sharing this manager's caches, so blocking workers mutate
    /// the same state as the caller instead of a throwaway copy.
    #[must_use]
    fn cache_handle(&self) -> Self {
        Self {
            rpm_db_path: self.rpm_db_path.clone(),
            repos_dir: self.repos_dir.clone(),
            installed_cache: Arc::clone(&self.installed_cache),
        }
    }

    /// Execute the `dnf` CLI as root (callers escalate via
    /// `run_self_sudo` first) and invalidate caches on success.
    fn run_dnf(&self, args: &[&str]) -> Result<()> {
        let mut cmd = Command::new("dnf");

        let status = cmd.args(args).arg("-y").status()?;

        if status.success() {
            self.installed_cache.clear();
            Ok(())
        } else {
            anyhow::bail!("dnf command failed with status {status}")
        }
    }
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

            // Repository search is unavailable until DNF repo metadata is
            // integrated; only installed packages match here.
            tracing::debug!("DNF repository search unavailable; returning installed matches only");

            // Deduplicate by name; sort installed rows first within a name
            // so dedup keeps the entry carrying `installed: true`.
            results.sort_by(|a, b| {
                a.name
                    .cmp(&b.name)
                    .then_with(|| b.installed.cmp(&a.installed))
            });
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

            if !is_root() {
                crate::core::privilege::run_self_sudo(&["install", "--"]).await?;
                return Ok(());
            }

            tokio::task::spawn_blocking({
                // Share caches so post-install invalidation reaches the caller.
                let manager = self.cache_handle();
                move || {
                    let mut args = vec!["install", "--"];
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

            if !is_root() {
                crate::core::privilege::run_self_sudo(&["remove", "--"]).await?;
                return Ok(());
            }

            tokio::task::spawn_blocking({
                let manager = self.cache_handle();
                move || {
                    let mut args = vec!["remove", "--"];
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
            if !is_root() {
                crate::core::privilege::run_self_sudo(&["upgrade"]).await?;
                return Ok(());
            }

            tokio::task::spawn_blocking({
                let manager = self.cache_handle();
                move || manager.run_dnf(&["upgrade"])
            })
            .await?
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // Clear caches and let the dnf CLI refresh its metadata
            self.installed_cache.clear();

            if !is_root() {
                crate::core::privilege::run_self_sudo(&["sync"]).await?;
                return Ok(());
            }

            tokio::task::spawn_blocking({
                let manager = self.cache_handle();
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

            // Repository lookups are unavailable until DNF repo metadata is
            // integrated; report an honest miss instead of pretending.
            tracing::debug!("DNF repository info unavailable for {package}");

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

            // Orphan detection needs an installed-package reverse-dependency
            // graph that this backend does not build; report zero rather than
            // guessing. Documented as unsupported in the backend docs.
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
            // Update detection compares installed versions against repository
            // metadata. Remote repomd/primary.xml access is not implemented,
            // so update checks fail explicitly rather than reporting a fake
            // empty update set.
            let _installed = self.load_installed_packages().await?;
            anyhow::bail!(
                "DNF repository metadata access is not implemented; \
                 update checks require dnf repoquery integration"
            );
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
    fn test_parse_rpm_qa_line_reads_installed_package() {
        let pkg = DnfPackageManager::parse_rpm_qa_line(
            "bash\t5.2.15\t1.fc39\tThe GNU Bourne Again shell\t0",
        )
        .expect("valid rpm -qa line");
        assert_eq!(pkg.name, "bash");
        assert_eq!(pkg.version, "5.2.15");
        assert_eq!(pkg.reason, InstallReason::User);
    }

    #[test]
    fn test_parse_rpm_qa_line_rejects_truncated_row() {
        let error = DnfPackageManager::parse_rpm_qa_line("bash\t5.2.15")
            .expect_err("truncated rpm -qa line must not skip the package");
        assert!(
            error.to_string().contains("malformed rpm -qa output"),
            "got: {error}"
        );
    }

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
