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
use zerocopy::{FromBytes, Immutable, KnownLayout, big_endian::U32};

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

// RPM header data types (librpm numbering): 1=CHAR 2=INT8 3=INT16 4=INT32
// 5=INT64 6=STRING 7=BIN 8=STRING_ARRAY 9=I18NSTRING. The parser matches on
// these numerals directly; see parse_rpm_header for the invariant table.

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

    /// Parse an RPM header region into a tag -> raw-bytes map.
    ///
    /// S1 rewrite (FEDORA-ENGINE.md; citations: /tmp/omg-fleet13
    /// rnd-pm-formats + rnd-pm-10): the header intro and every index entry
    /// are read through zero-copy `zerocopy` big-endian views, and all
    /// librpm validation invariants are enforced fail-closed:
    /// - magic AND reserved words must match (`8e ad e8 01 00 00 00 00`);
    /// - entry counts are capped before any allocation;
    /// - tag types must be in librpm's range 1..=9 (stored NULL/type 0 and
    ///   out-of-range types are rejected, not skipped);
    /// - STRING and I18NSTRING carry exactly one element (count-one rule);
    /// - entry data regions must lie fully inside the declared data area —
    ///   hostile or negative offsets are hard errors, never silent skips.
    fn parse_rpm_header(blob: &[u8]) -> Result<HashMap<u32, Vec<u8>>> {
        const MAX_HEADER_ENTRIES: u32 = 100_000;

        #[derive(FromBytes, KnownLayout, Immutable)]
        #[repr(C)]
        struct HeaderIntro {
            magic: [u8; 4],
            reserved: [u8; 4],
            num_entries: U32,
            data_size: U32,
        }

        #[derive(FromBytes, KnownLayout, Immutable)]
        #[repr(C)]
        struct IndexEntry {
            tag: U32,
            tag_type: U32,
            offset: [u8; 4],
            count: U32,
        }

        if blob.len() < size_of::<HeaderIntro>() {
            anyhow::bail!("RPM header too short");
        }
        let (intro_bytes, rest) = blob.split_at(size_of::<HeaderIntro>());
        let intro = HeaderIntro::ref_from_bytes(intro_bytes).expect("intro length checked above");
        if intro.magic != RPM_HEADER_MAGIC[..4] || intro.reserved != RPM_HEADER_MAGIC[4..8] {
            anyhow::bail!("Invalid RPM header magic");
        }

        let num_entries = intro.num_entries.get();
        anyhow::ensure!(
            num_entries <= MAX_HEADER_ENTRIES,
            "RPM header declares {num_entries} entries (limit {MAX_HEADER_ENTRIES})"
        );
        let data_size = intro.data_size.get() as usize;

        let entries_len = num_entries as usize * size_of::<IndexEntry>();
        // The data area starts immediately after the index (librpm layout);
        // deriving it from the tail would let appended bytes shift the
        // payload window and satisfy string terminators outside the
        // declared region.
        let data_start = entries_len;
        anyhow::ensure!(
            data_start
                .checked_add(data_size)
                .is_some_and(|end| end <= rest.len()),
            "RPM header truncated"
        );

        let payload = &rest[data_start..data_start + data_size];
        let mut tags = HashMap::with_capacity(num_entries as usize);
        for chunk in rest[..data_start].chunks_exact(size_of::<IndexEntry>()) {
            let entry =
                IndexEntry::ref_from_bytes(chunk).expect("chunk length checked by chunks_exact");
            let tag = entry.tag.get();
            let tag_type = entry.tag_type.get();
            let count = entry.count.get() as usize;

            anyhow::ensure!(
                (1..=9).contains(&tag_type),
                "RPM tag {tag} has unsupported type {tag_type}"
            );
            anyhow::ensure!(
                !matches!(tag_type, 6 | 9) || count == 1, // 6=STRING 9=I18NSTRING
                "RPM string tag {tag} must have count 1, got {count}"
            );

            let rel = i32::from_be_bytes(entry.offset);
            anyhow::ensure!(rel >= 0, "RPM tag {tag} has negative data offset {rel}");
            let base = rel as usize;
            anyhow::ensure!(
                base < payload.len(),
                "RPM tag {tag} data offset outside payload"
            );

            // Region length this tag occupies inside the payload.
            let region: usize = match tag_type {
                1 | 2 => count.saturating_mul(1),
                3 => count.saturating_mul(2),
                4 => count.saturating_mul(4),
                5 => count.saturating_mul(8),
                7 => count,
                6 | 8 | 9 => {
                    // Strings terminate inside the declared payload only;
                    // bytes beyond it can never satisfy a NUL.
                    payload[base..]
                        .iter()
                        .position(|&b| b == 0)
                        .map(|npos| npos + 1)
                        .ok_or_else(|| anyhow::anyhow!("RPM tag {tag} string missing terminator"))?
                }
                _ => unreachable!("type range validated above"),
            };

            let abs_end = base
                .checked_add(region)
                .ok_or_else(|| anyhow::anyhow!("RPM tag {tag} data region overflows"))?;
            anyhow::ensure!(
                abs_end <= payload.len(),
                "RPM tag {tag} data region exceeds payload"
            );

            tags.insert(tag, payload[base..abs_end].to_vec());
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
    /// `run_privileged_child` first) and invalidate caches on success.
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
                // Native elevation with the exact resolved package list —
                // no omg re-exec, no second listing or confirmation prompt.
                let mut args = vec!["install", "-y", "--"];
                args.extend(packages.iter().map(String::as_str));
                crate::core::privilege::run_privileged_program("dnf", &args).await?;
                self.installed_cache.clear();
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
                let mut args = vec!["remove", "-y", "--"];
                args.extend(packages.iter().map(String::as_str));
                crate::core::privilege::run_privileged_program("dnf", &args).await?;
                self.installed_cache.clear();
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
                crate::core::privilege::run_privileged_program("dnf", &["upgrade", "-y"]).await?;
                self.installed_cache.clear();
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
                crate::core::privilege::run_privileged_program("dnf", &["makecache", "-y"]).await?;
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

            // Count available updates via the dnf CLI, mirroring how this
            // backend already executes transactions. `check-update` exits 0
            // with no updates and 100 when updates are available; any other
            // outcome means the count is unavailable, which must not fail the
            // whole status command.
            let updates = tokio::task::spawn_blocking(|| {
                Command::new("dnf")
                    .args(["-q", "--cacheonly", "check-update"])
                    .output()
            })
            .await
            .context("dnf check-update task failed")?
            .map_or_else(
                |error| {
                    tracing::debug!("dnf CLI unavailable for update count: {error}");
                    0
                },
                |output| match output.status.code() {
                    Some(0) => 0,
                    Some(100) => String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .filter(|line| !line.trim().is_empty())
                        .count(),
                    _ => {
                        tracing::debug!(
                            "dnf check-update returned {:?}; update count unavailable",
                            output.status.code()
                        );
                        0
                    }
                },
            );

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
        header.extend_from_slice(&[0, 0, 0, 5]); // 5 bytes data ("test\0")
        // Entry: tag=1000 (NAME), type=6 (STRING), offset=0.
        // librpm count-one invariant (rnd-pm-10): STRING carries exactly one
        // element; the old fixture's count=4 encoded pre-S1 lax parsing.
        header.extend_from_slice(&1000u32.to_be_bytes());
        header.extend_from_slice(&6u32.to_be_bytes());
        header.extend_from_slice(&0i32.to_be_bytes());
        header.extend_from_slice(&1u32.to_be_bytes());
        // Data: "test\0"
        header.extend_from_slice(b"test\0");

        let result = DnfPackageManager::parse_rpm_header(&header);
        assert!(result.is_ok());

        let tags = result.unwrap();
        assert!(tags.contains_key(&1000));
    }

    fn strict_header(entries: &[(u32, u32, i32, u32)], data: &[u8]) -> Vec<u8> {
        let mut header = Vec::new();
        header.extend_from_slice(&RPM_HEADER_MAGIC);
        header.extend_from_slice(&(entries.len() as u32).to_be_bytes());
        header.extend_from_slice(&(data.len() as u32).to_be_bytes());
        for (tag, typ, off, count) in entries {
            header.extend_from_slice(&tag.to_be_bytes());
            header.extend_from_slice(&typ.to_be_bytes());
            header.extend_from_slice(&off.to_be_bytes());
            header.extend_from_slice(&count.to_be_bytes());
        }
        header.extend_from_slice(data);
        header
    }

    #[test]
    fn s1_rejects_negative_entry_offset() {
        let blob = strict_header(&[(1000, 6, -1, 1)], b"test\0");
        assert!(DnfPackageManager::parse_rpm_header(&blob).is_err());
    }

    #[test]
    fn s1_rejects_unknown_tag_type() {
        for bad_type in [0u32, 10, 12] {
            let blob = strict_header(&[(1000, bad_type, 0, 1)], b"abcd");
            assert!(
                DnfPackageManager::parse_rpm_header(&blob).is_err(),
                "type {bad_type} must be rejected"
            );
        }
    }

    #[test]
    fn s1_enforces_string_count_one() {
        let blob = strict_header(&[(1000, 6, 0, 4)], b"test\0");
        assert!(DnfPackageManager::parse_rpm_header(&blob).is_err());
    }

    #[test]
    fn s1_rejects_string_missing_terminator() {
        let blob = strict_header(&[(1004, 6, 0, 1)], b"no-nul");
        assert!(DnfPackageManager::parse_rpm_header(&blob).is_err());
    }

    #[test]
    fn s1_rejects_data_region_outside_declared_payload() {
        // Offset points past the declared data size.
        let blob = strict_header(&[(1000, 6, 99, 1)], b"test\0");
        assert!(DnfPackageManager::parse_rpm_header(&blob).is_err());
    }

    #[test]
    fn s1_rejects_undeclared_trailing_payload_use() {
        // Data region ends at data_start+data_size; a string terminator may
        // not be satisfied by bytes beyond it.
        let mut blob = strict_header(&[(1000, 6, 0, 1)], b"xxxxx");
        blob.extend_from_slice(b"\0"); // NUL outside the declared payload
        assert!(DnfPackageManager::parse_rpm_header(&blob).is_err());
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
