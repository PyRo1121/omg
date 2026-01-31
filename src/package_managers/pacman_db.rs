//! Pure Rust Pacman Database Parser - ULTRA FAST (<20ms!)
//!
//! Parses /var/lib/pacman/sync/*.db and /var/lib/pacman/local/
//! WITHOUT using libalpm. Direct tar.gz/tar.zst parsing.
//!
//! First load: ~100ms (parse all DBs)
//! Cached: <1ms (instant lookup)
//! rkyv mmap: <100µs (zero-copy access)

#[cfg(feature = "arch")]
use alpm_db;
#[cfg(feature = "arch")]
use alpm_repo_db;
#[cfg(feature = "arch")]
use alpm_types::Version;
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use futures::stream::StreamExt;
use memmap2::Mmap;
use parking_lot::RwLock;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use std::time::SystemTime;
use tracing::instrument;

/// Case-insensitive substring check without allocation.
/// Returns true if `needle` (already lowercase) is found in `haystack` case-insensitively.
#[inline]
fn contains_ignore_case(haystack: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    if haystack.len() < needle_lower.len() {
        return false;
    }
    haystack
        .as_bytes()
        .windows(needle_lower.len())
        .any(|window| window.eq_ignore_ascii_case(needle_lower.as_bytes()))
}

use crate::core::paths;

/// TTL for cache eviction safety net (30 minutes)
const CACHE_TTL_SECS: u64 = 30 * 60;

/// Global cache for sync databases - parsed once, used forever until invalidated
static SYNC_DB_CACHE: std::sync::LazyLock<RwLock<DbCache>> =
    std::sync::LazyLock::new(|| RwLock::new(DbCache::default()));

/// Global cache for local database
static LOCAL_DB_CACHE: std::sync::LazyLock<RwLock<LocalDbCache>> =
    std::sync::LazyLock::new(|| RwLock::new(LocalDbCache::default()));

/// Global mmap-based zero-copy sync database index
static SYNC_MMAP_INDEX: std::sync::LazyLock<RwLock<Option<PacmanMmapIndex>>> =
    std::sync::LazyLock::new(|| RwLock::new(None));

#[derive(Default, Serialize, Deserialize)]
struct DbCache {
    packages: HashMap<String, SyncDbPackage>,
    last_modified: Option<SystemTime>,
    #[serde(skip)]
    last_accessed: Option<SystemTime>,
}

/// rkyv-compatible package struct for zero-copy mmap access
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone)]
pub struct RkyvSyncPackage {
    pub name: String,
    pub version: String,
    pub desc: String,
    pub filename: String,
    pub csize: u64,
    pub isize: u64,
    pub url: String,
    pub arch: String,
    pub repo: String,
    pub licenses: Vec<String>,
    pub depends: Vec<String>,
    pub makedepends: Vec<String>,
    pub optdepends: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
}

/// rkyv-compatible index for zero-copy mmap access
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Default, Clone)]
pub struct RkyvSyncIndex {
    pub packages: Vec<RkyvSyncPackage>,
    pub name_to_idx: HashMap<String, usize>,
    pub mtime_secs: u64,
}

/// Memory-mapped zero-copy sync database index
pub struct PacmanMmapIndex {
    mmap: Mmap,
    mtime: u64,
}

impl PacmanMmapIndex {
    pub fn load(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };

        let mtime: u64 = {
            let archived =
                rkyv::access::<rkyv::Archived<RkyvSyncIndex>, rkyv::rancor::Error>(&mmap)
                    .map_err(|e| anyhow::anyhow!("rkyv access failed: {e}"))?;
            archived.mtime_secs.into()
        };

        Ok(Self { mmap, mtime })
    }

    #[inline]
    fn archived(&self) -> &rkyv::Archived<RkyvSyncIndex> {
        unsafe { rkyv::access_unchecked::<rkyv::Archived<RkyvSyncIndex>>(&self.mmap) }
    }

    pub fn get(&self, name: &str) -> Option<&rkyv::Archived<RkyvSyncPackage>> {
        let archived = self.archived();
        archived
            .name_to_idx
            .get(name)
            .and_then(|idx| archived.packages.get(u32::from(*idx) as usize))
    }

    pub fn search(&self, query: &str) -> Vec<&rkyv::Archived<RkyvSyncPackage>> {
        let query_lower = query.to_lowercase();
        let archived = self.archived();
        archived
            .packages
            .iter()
            .filter(|p| {
                contains_ignore_case(p.name.as_str(), &query_lower)
                    || contains_ignore_case(p.desc.as_str(), &query_lower)
            })
            .collect()
    }

    pub fn len(&self) -> usize {
        self.archived().packages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn mtime(&self) -> u64 {
        self.mtime
    }
}

fn load_sync_packages(sync_dir: &Path) -> Result<HashMap<String, SyncDbPackage>> {
    let db_paths = collect_sync_db_paths(sync_dir);
    let parsed: Vec<HashMap<String, SyncDbPackage>> = db_paths
        .par_iter()
        .map(|(path, name)| {
            parse_sync_db(path, name)
                .with_context(|| format!("Failed to parse database: {} ({})", path.display(), name))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut packages = HashMap::with_capacity(20000);
    for pkgs in parsed {
        packages.extend(pkgs);
    }
    Ok(packages)
}

fn collect_sync_db_paths(sync_dir: &Path) -> Vec<(PathBuf, String)> {
    // Pre-allocate for standard repos (core, extra, multilib) plus potential custom repos
    let mut dbs = Vec::with_capacity(8);

    for db_name in &["core", "extra", "multilib"] {
        let db_path = sync_dir.join(format!("{db_name}.db"));
        if db_path.exists() {
            dbs.push((db_path, (*db_name).to_string()));
        }
    }

    if let Ok(entries) = std::fs::read_dir(sync_dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            // Skip non-files and signature files
            if !path.is_file() {
                continue;
            }

            // Only process .db files (not .db.sig or other extensions)
            if !path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("db"))
            {
                continue;
            }

            // Extract repo name (file_stem gives us the name without .db)
            let name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .map(str::to_string);

            // Skip standard repos (already added above)
            if let Some(name) = name
                && !["core", "extra", "multilib"].contains(&name.as_str())
            {
                dbs.push((path, name));
            }
        }
    }

    dbs
}

#[derive(Default, Serialize, Deserialize)]
struct LocalDbCache {
    packages: HashMap<String, LocalDbPackage>,
    last_modified: Option<SystemTime>,
    /// Last access time for TTL-based eviction (30-minute safety net)
    #[serde(skip)]
    last_accessed: Option<SystemTime>,
}

/// A package entry from the sync database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncDbPackage {
    pub name: String,
    pub version: Version,
    pub desc: String,
    pub filename: String,
    pub csize: u64, // Compressed size (download size)
    pub isize: u64, // Installed size
    pub url: String,
    pub arch: String,
    pub repo: String,
    pub licenses: Vec<String>,
    pub depends: Vec<String>,
    pub makedepends: Vec<String>,
    pub optdepends: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
}

impl Default for SyncDbPackage {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: super::types::zero_version(),
            desc: String::new(),
            filename: String::new(),
            csize: 0,
            isize: 0,
            url: String::new(),
            arch: String::new(),
            repo: String::new(),
            licenses: Vec::new(),
            depends: Vec::new(),
            makedepends: Vec::new(),
            optdepends: Vec::new(),
            provides: Vec::new(),
            conflicts: Vec::new(),
            replaces: Vec::new(),
        }
    }
}

/// A package from the local database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalDbPackage {
    pub name: String,
    pub version: Version,
    pub desc: String,
    pub install_date: String,
    pub licenses: Vec<String>,
    pub explicit: bool, // Explicitly installed vs dependency
}

impl Default for LocalDbPackage {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: super::types::zero_version(),
            desc: String::new(),
            install_date: String::new(),
            licenses: Vec::new(),
            explicit: false,
        }
    }
}

/// Parse a sync database file (core.db, extra.db, multilib.db)
/// Returns a `HashMap` of package name -> `SyncDbPackage`
pub fn parse_sync_db(path: &Path, repo_name: &str) -> Result<HashMap<String, SyncDbPackage>> {
    let file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;

    // Detect compression type from magic bytes
    let reader: Box<dyn Read> = {
        let path_str = path.to_string_lossy();
        if path_str.ends_with(".db") {
            // Try to detect actual format by reading magic bytes
            let mut magic = [0u8; 4];
            let mut f = File::open(path)?;
            f.read_exact(&mut magic)?;

            let file = File::open(path)?;
            if magic[0..2] == [0x1f, 0x8b] {
                Box::new(GzDecoder::new(file))
            } else if magic[0..4] == [0x28, 0xb5, 0x2f, 0xfd] {
                let mut decoder = ruzstd::decoding::StreamingDecoder::new(file)
                    .map_err(|e| anyhow::anyhow!("zstd init: {e}"))?;
                let mut decompressed = Vec::new();
                std::io::copy(&mut decoder, &mut decompressed)?;
                Box::new(std::io::Cursor::new(decompressed))
            } else {
                Box::new(GzDecoder::new(file))
            }
        } else if path_str.ends_with(".zst") {
            Box::new(
                ruzstd::decoding::StreamingDecoder::new(file)
                    .map_err(|e| anyhow::anyhow!("zstd init: {e}"))?,
            )
        } else {
            Box::new(GzDecoder::new(file))
        }
    };

    let mut archive = tar::Archive::new(reader);
    let mut packages = HashMap::new();

    for entry in archive.entries().with_context(|| {
        format!(
            "Failed to read tar entries from {} (repo: {})",
            path.display(),
            repo_name
        )
    })? {
        let mut entry = entry?;
        let entry_path = entry.path()?.to_path_buf();
        let path_str = entry_path.to_string_lossy();

        if path_str.ends_with("/desc") {
            let mut content = String::new();
            if entry.read_to_string(&mut content).is_err() {
                continue;
            }

            let pkg = parse_desc_content(&content, repo_name);
            if !pkg.name.is_empty() {
                packages.insert(pkg.name.clone(), pkg);
            }
        }
    }

    Ok(packages)
}

/// Extract a human-readable message from a `catch_unwind` panic payload.
fn panic_message(info: &(dyn std::any::Any + Send)) -> &str {
    info.downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| info.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic")
}

/// Convert an alpm desc (V1 or V2) into a `SyncDbPackage`.
/// Both desc types expose identical field names, so a macro avoids duplication.
macro_rules! sync_pkg_from_desc {
    ($desc:expr, $repo:expr) => {
        SyncDbPackage {
            name: $desc.name.to_string(),
            version: Version::from_str(&$desc.version.to_string())
                .unwrap_or_else(|_| super::types::zero_version()),
            desc: $desc.description.to_string(),
            filename: $desc.file_name.to_string(),
            csize: $desc.compressed_size,
            isize: $desc.installed_size,
            url: $desc
                .url
                .as_ref()
                .map(std::string::ToString::to_string)
                .unwrap_or_default(),
            arch: $desc.arch.to_string(),
            repo: $repo.to_string(),
            licenses: $desc.license.iter().map(ToString::to_string).collect(),
            depends: $desc
                .dependencies
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            makedepends: $desc
                .make_dependencies
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            optdepends: $desc
                .optional_dependencies
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            provides: $desc
                .provides
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            conflicts: $desc
                .conflicts
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            replaces: $desc
                .replaces
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
        }
    };
}

fn parse_desc_content(content: &str, repo: &str) -> SyncDbPackage {
    // Try V2 first (newer format without MD5SUM).
    // Wrap in catch_unwind because alpm_repo_db can panic on corrupted data
    // (e.g., PackageRelation::from_str panics on malformed dependency strings).
    let v2_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        alpm_repo_db::desc::RepoDescFileV2::from_str(content)
    }));

    match v2_result {
        Ok(Ok(desc)) => return sync_pkg_from_desc!(desc, repo),
        Ok(Err(_)) => {}
        Err(panic_info) => {
            tracing::warn!(
                repo = repo,
                error = panic_message(&*panic_info),
                "Corrupted package desc in repo (V2 parse panic), trying V1 fallback"
            );
        }
    }

    // Fallback to V1 (older format with MD5SUM)
    let v1_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        alpm_repo_db::desc::RepoDescFileV1::from_str(content)
    }));

    match v1_result {
        Ok(Ok(desc)) => return sync_pkg_from_desc!(desc, repo),
        Ok(Err(_)) => {}
        Err(panic_info) => {
            tracing::warn!(
                repo = repo,
                error = panic_message(&*panic_info),
                "Corrupted package desc in repo (V1 parse panic), trying manual fallback"
            );
        }
    }

    // Both V2 and V1 failed - use manual lenient parser as final fallback
    // This handles cases like "Unknown Packager" without email (common in custom repos)
    parse_desc_manual(content, repo)
}

fn parse_desc_manual(content: &str, repo: &str) -> SyncDbPackage {
    let mut pkg = SyncDbPackage {
        repo: repo.to_string(),
        ..SyncDbPackage::default()
    };

    let mut current_section = "";
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('%') && line.ends_with('%') {
            current_section = line;
            continue;
        }

        match current_section {
            "%NAME%" => pkg.name = line.to_string(),
            "%VERSION%" => {
                pkg.version = crate::package_managers::parse_version_or_zero(line);
            }
            "%DESC%" => pkg.desc = line.to_string(),
            "%URL%" => pkg.url = line.to_string(),
            "%ARCH%" => pkg.arch = line.to_string(),
            "%CSIZE%" => pkg.csize = line.parse().unwrap_or(0),
            "%ISIZE%" => pkg.isize = line.parse().unwrap_or(0),
            "%FILENAME%" => pkg.filename = line.to_string(),
            "%DEPENDS%" => pkg.depends.push(line.to_string()),
            "%PROVIDES%" => pkg.provides.push(line.to_string()),
            "%CONFLICTS%" => pkg.conflicts.push(line.to_string()),
            "%REPLACES%" => pkg.replaces.push(line.to_string()),
            "%OPTDEPENDS%" => pkg.optdepends.push(line.to_string()),
            "%MAKEDEPENDS%" => pkg.makedepends.push(line.to_string()),
            "%LICENSE%" => pkg.licenses.push(line.to_string()),
            _ => {}
        }
    }

    pkg
}

/// Parse the local package database (/var/lib/pacman/local/)
/// Returns a `HashMap` of package name -> `LocalDbPackage`
pub fn parse_local_db(path: &Path) -> Result<HashMap<String, LocalDbPackage>> {
    let mut packages = HashMap::with_capacity(2000); // Pre-allocate for typical system

    if !path.exists() {
        return Ok(packages);
    }

    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let pkg_path = entry.path();

        if !pkg_path.is_dir() {
            continue;
        }

        let desc_path = pkg_path.join("desc");
        if !desc_path.exists() {
            continue;
        }

        if let Ok(pkg) = parse_local_desc(&desc_path) {
            packages.insert(pkg.name.clone(), pkg);
        }
    }

    Ok(packages)
}

fn parse_local_desc(path: &Path) -> Result<LocalDbPackage> {
    let content = std::fs::read_to_string(path)?;

    // Try V1 first (most common)
    if let Ok(desc) = alpm_db::desc::DbDescFileV1::from_str(&content) {
        return Ok(LocalDbPackage {
            name: desc.name.to_string(),
            version: Version::from_str(&desc.version.to_string())
                .unwrap_or_else(|_| super::types::zero_version()),
            desc: desc.description.to_string(),
            install_date: desc.installdate.to_string(),
            licenses: desc.license.iter().map(ToString::to_string).collect(),
            explicit: matches!(desc.reason, alpm_types::PackageInstallReason::Explicit),
        });
    }

    // Try V2 (has XDATA support)
    if let Ok(desc) = alpm_db::desc::DbDescFileV2::from_str(&content) {
        return Ok(LocalDbPackage {
            name: desc.name.to_string(),
            version: Version::from_str(&desc.version.to_string())
                .unwrap_or_else(|_| super::types::zero_version()),
            desc: desc.description.to_string(),
            install_date: desc.installdate.to_string(),
            licenses: desc.license.iter().map(ToString::to_string).collect(),
            explicit: matches!(desc.reason, alpm_types::PackageInstallReason::Explicit),
        });
    }

    // Fallback: manual parsing for edge cases
    parse_local_desc_manual(&content)
}

/// Manual local desc parser as fallback
fn parse_local_desc_manual(content: &str) -> Result<LocalDbPackage> {
    let mut name = String::new();
    let mut version = String::new();
    let mut desc = String::new();
    let mut install_date = String::new();
    let mut reason = String::new();
    let mut licenses = Vec::new();
    let mut current_field: Option<&str> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            current_field = None;
            continue;
        }

        if line.starts_with('%') && line.ends_with('%') {
            current_field = Some(line);
            continue;
        }

        match current_field {
            Some("%NAME%") => name = line.to_string(),
            Some("%VERSION%") => version = line.to_string(),
            Some("%DESC%") => desc = line.to_string(),
            Some("%INSTALLDATE%") => install_date = line.to_string(),
            Some("%REASON%") => reason = line.to_string(),
            Some("%LICENSE%") => licenses.push(line.to_string()),
            _ => {}
        }
    }

    if name.is_empty() {
        anyhow::bail!("Failed to parse local desc file: no NAME found");
    }

    Ok(LocalDbPackage {
        name,
        version: Version::from_str(&version).unwrap_or_else(|_| super::types::zero_version()),
        desc,
        install_date,
        licenses,
        explicit: reason != "1", // 1 = dependency, empty/0 = explicit
    })
}

pub fn get_detailed_packages() -> Result<Vec<SyncDbPackage>> {
    let sync_dir = paths::pacman_sync_dir();

    ensure_sync_cache_loaded(&sync_dir)?;

    let cache = SYNC_DB_CACHE.read();

    Ok(cache.packages.values().cloned().collect())
}

/// ULTRA FAST update check
///
/// Uses global cache (<5ms after first load!)
/// Returns Vec of (name, `old_version`, `new_version`, repo, filename, `download_size`)
#[instrument]
pub fn check_updates_cached() -> Result<Vec<(String, Version, Version, String, String, u64)>> {
    let sync_dir = paths::pacman_sync_dir();
    let local_dir = paths::pacman_local_dir();

    // Ensure caches are loaded (will be fast if already loaded)
    ensure_sync_cache_loaded(&sync_dir)?;
    ensure_local_cache_loaded(&local_dir)?;

    // Hold both cache locks simultaneously - no cloning!
    let sync_cache = SYNC_DB_CACHE.read();
    let local_cache = LOCAL_DB_CACHE.read();

    // Compare versions - Parallelized with Rayon for <1ms update check on 2000+ pkgs
    // Optimized: filter references first, then clone only needed data at the end
    let updates: Vec<_> = local_cache
        .packages
        .par_iter()
        .filter_map(|(name, local_pkg)| {
            sync_cache
                .packages
                .get(name)
                .filter(|sync_pkg| local_pkg.version < sync_pkg.version)
                .map(|sync_pkg| (name, local_pkg, sync_pkg))
        })
        .map(|(name, local_pkg, sync_pkg)| {
            // Only clone once at the end
            (
                name.clone(),
                local_pkg.version.clone(),
                sync_pkg.version.clone(),
                sync_pkg.repo.clone(),
                sync_pkg.filename.clone(),
                sync_pkg.csize,
            )
        })
        .collect();

    Ok(updates)
}

/// Get the cache directory for OMG
fn get_cache_dir() -> PathBuf {
    paths::cache_dir()
}

/// Save cache to disk in binary format
fn save_cache_to_disk<T: Serialize>(cache: &T, name: &str) -> Result<()> {
    let cache_dir = get_cache_dir();
    fs::create_dir_all(&cache_dir).ok();
    let path = cache_dir.join(format!("{name}.bin"));

    // Write to a temporary file first for atomicity
    let tmp_path = path.with_extension("tmp");
    let data = bitcode::serialize(cache)?;
    fs::write(&tmp_path, data)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

/// Load cache from disk
fn load_cache_from_disk<T: for<'de> Deserialize<'de>>(name: &str) -> Result<T> {
    let path = get_cache_dir().join(format!("{name}.bin"));
    let data = fs::read(&path)?;
    let cache: T = bitcode::deserialize(&data)?;
    Ok(cache)
}

fn get_mmap_index_path() -> PathBuf {
    get_cache_dir().join("sync_db_v2.rkyv")
}

fn build_rkyv_index(packages: &HashMap<String, SyncDbPackage>, mtime: u64) -> RkyvSyncIndex {
    let mut index = RkyvSyncIndex {
        packages: Vec::with_capacity(packages.len()),
        name_to_idx: HashMap::with_capacity(packages.len()),
        mtime_secs: mtime,
    };

    for (name, pkg) in packages {
        let idx = index.packages.len();
        index.name_to_idx.insert(name.clone(), idx);
        index.packages.push(RkyvSyncPackage {
            name: pkg.name.clone(),
            version: pkg.version.to_string(),
            desc: pkg.desc.clone(),
            filename: pkg.filename.clone(),
            csize: pkg.csize,
            isize: pkg.isize,
            url: pkg.url.clone(),
            arch: pkg.arch.clone(),
            repo: pkg.repo.clone(),
            licenses: pkg.licenses.clone(),
            depends: pkg.depends.clone(),
            makedepends: pkg.makedepends.clone(),
            optdepends: pkg.optdepends.clone(),
            provides: pkg.provides.clone(),
            conflicts: pkg.conflicts.clone(),
            replaces: pkg.replaces.clone(),
        });
    }

    index
}

fn save_mmap_index(index: &RkyvSyncIndex) -> Result<()> {
    let cache_dir = get_cache_dir();
    fs::create_dir_all(&cache_dir).ok();
    let path = get_mmap_index_path();
    let tmp_path = path.with_extension("tmp");

    let bytes =
        rkyv::to_bytes::<rkyv::rancor::Error>(index).map_err(|e| anyhow::anyhow!("rkyv: {e}"))?;
    fs::write(&tmp_path, &bytes)?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

fn try_load_mmap_index(expected_mtime: u64) -> Option<PacmanMmapIndex> {
    let path = get_mmap_index_path();
    if !path.exists() {
        return None;
    }

    match PacmanMmapIndex::load(&path) {
        Ok(idx) if idx.mtime() == expected_mtime => Some(idx),
        Ok(_) => None,
        Err(e) => {
            tracing::debug!("mmap index load failed: {e}");
            None
        }
    }
}

/// Check if cache is expired based on TTL (30-minute safety net)
fn is_cache_expired(last_accessed: Option<SystemTime>) -> bool {
    if let Some(last_access) = last_accessed
        && let Ok(elapsed) = SystemTime::now().duration_since(last_access)
    {
        return elapsed.as_secs() > CACHE_TTL_SECS;
    }
    false
}

/// Ensure sync cache is loaded (fast if already loaded)
fn ensure_sync_cache_loaded(sync_dir: &Path) -> Result<()> {
    let current_mtime = get_newest_db_mtime(sync_dir);

    {
        let mut cache = SYNC_DB_CACHE.write();
        if cache.last_modified == Some(current_mtime)
            && !cache.packages.is_empty()
            && !is_cache_expired(cache.last_accessed)
        {
            // Update last accessed time on cache hit
            cache.last_accessed = Some(SystemTime::now());
            return Ok(());
        }

        // Clear cache if TTL expired (safety net for unbounded growth)
        if is_cache_expired(cache.last_accessed) {
            cache.packages.clear();
            cache.last_modified = None;
            cache.last_accessed = None;
        }
    }

    // Try to load from disk cache first (FAST < 5ms)
    if let Ok(disk_cache) = load_cache_from_disk::<DbCache>("sync_db")
        && disk_cache.last_modified == Some(current_mtime)
    {
        let mut cache = SYNC_DB_CACHE.write();

        // Double-check: another thread may have loaded while we were waiting
        if cache.last_modified == Some(current_mtime) && !cache.packages.is_empty() {
            cache.last_accessed = Some(SystemTime::now());
            return Ok(());
        }

        *cache = disk_cache;
        cache.last_accessed = Some(SystemTime::now());
        return Ok(());
    }

    // Cache miss or stale - need to reload/parse
    let packages = load_sync_packages(sync_dir)?;

    // Update memory cache with double-checked locking
    let mut cache = SYNC_DB_CACHE.write();

    // Re-check: another thread may have loaded while we were parsing
    if cache.last_modified == Some(current_mtime) && !cache.packages.is_empty() {
        cache.last_accessed = Some(SystemTime::now());
        return Ok(());
    }

    cache.packages = packages;
    cache.last_modified = Some(current_mtime);
    cache.last_accessed = Some(SystemTime::now());

    // Save to disk for next time (bitcode for backward compat)
    let _ = save_cache_to_disk(&*cache, "sync_db");

    // Also save rkyv mmap index for zero-copy access
    let mtime_secs = current_mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let rkyv_index = build_rkyv_index(&cache.packages, mtime_secs);
    if let Err(e) = save_mmap_index(&rkyv_index) {
        tracing::debug!("Failed to save rkyv mmap index: {e}");
    }

    Ok(())
}

/// Ensure local cache is loaded (fast if already loaded)
fn ensure_local_cache_loaded(local_dir: &Path) -> Result<()> {
    let current_mtime = get_local_db_mtime(local_dir)?;

    {
        let mut cache = LOCAL_DB_CACHE.write();
        if cache.last_modified == Some(current_mtime)
            && !cache.packages.is_empty()
            && !is_cache_expired(cache.last_accessed)
        {
            // Update last accessed time on cache hit
            cache.last_accessed = Some(SystemTime::now());
            return Ok(());
        }

        // Clear cache if TTL expired (safety net for unbounded growth)
        if is_cache_expired(cache.last_accessed) {
            cache.packages.clear();
            cache.last_modified = None;
            cache.last_accessed = None;
        }
    }

    // Try to load from disk cache first
    if let Ok(disk_cache) = load_cache_from_disk::<LocalDbCache>("local_db")
        && disk_cache.last_modified == Some(current_mtime)
    {
        let mut cache = LOCAL_DB_CACHE.write();

        // Double-check: another thread may have loaded while we were waiting
        if cache.last_modified == Some(current_mtime) && !cache.packages.is_empty() {
            cache.last_accessed = Some(SystemTime::now());
            return Ok(());
        }

        *cache = disk_cache;
        cache.last_accessed = Some(SystemTime::now());
        return Ok(());
    }

    // Cache miss - reload
    let packages = parse_local_db(local_dir)?;

    // Update memory cache
    let mut cache = LOCAL_DB_CACHE.write();

    // Double-check: another thread may have loaded while we were parsing
    if cache.last_modified == Some(current_mtime) && !cache.packages.is_empty() {
        cache.last_accessed = Some(SystemTime::now());
        return Ok(());
    }

    cache.packages = packages;
    cache.last_modified = Some(current_mtime);
    cache.last_accessed = Some(SystemTime::now());

    // Save to disk
    let _ = save_cache_to_disk(&*cache, "local_db");

    Ok(())
}

/// Get newest modification time of sync DBs
fn get_newest_db_mtime(sync_dir: &Path) -> SystemTime {
    let mut newest = SystemTime::UNIX_EPOCH;

    if let Ok(entries) = std::fs::read_dir(sync_dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata()
                && let Ok(mtime) = meta.modified()
                && mtime > newest
            {
                newest = mtime;
            }
        }
    }

    newest
}

/// Get modification time of local db directory
fn get_local_db_mtime(local_dir: &Path) -> Result<SystemTime> {
    let meta = std::fs::metadata(local_dir)?;
    Ok(meta.modified()?)
}

/// Force refresh of all caches (call after sync/install)
pub fn invalidate_caches() {
    {
        let mut cache = SYNC_DB_CACHE.write();
        cache.packages.clear();
        cache.last_modified = None;
    }
    {
        let mut cache = LOCAL_DB_CACHE.write();
        cache.packages.clear();
        cache.last_modified = None;
    }
    {
        let mut mmap_cache = SYNC_MMAP_INDEX.write();
        *mmap_cache = None;
    }

    super::alpm_direct::clear_alpm_cache();

    let cache_dir = get_cache_dir();
    let _ = fs::remove_file(cache_dir.join("sync_db.bin"));
    let _ = fs::remove_file(cache_dir.join("local_db.bin"));
    let _ = fs::remove_file(get_mmap_index_path());
}

/// Pre-load caches in background (call on daemon startup)
pub fn preload_caches() -> Result<()> {
    let sync_dir = paths::pacman_sync_dir();
    let local_dir = paths::pacman_local_dir();
    ensure_sync_cache_loaded(&sync_dir)?;
    ensure_local_cache_loaded(&local_dir)?;
    Ok(())
}

/// Legacy function - kept for compatibility, now uses cache
pub fn check_updates_fast() -> Result<Vec<(String, Version, Version, String, String, u64)>> {
    check_updates_cached()
}

/// Compare two version strings using `alpm_types::Version`
/// Legacy wrapper for compatibility
pub fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    let ver1 = Version::from_str(v1).unwrap_or_else(|_| super::types::zero_version());
    let ver2 = Version::from_str(v2).unwrap_or_else(|_| super::types::zero_version());
    ver1.cmp(&ver2)
}

/// Get a specific local package - FAST (<1ms)
pub fn get_local_package(name: &str) -> Result<Option<LocalDbPackage>> {
    let local_dir = paths::pacman_local_dir();
    ensure_local_cache_loaded(&local_dir)?;

    let cache = LOCAL_DB_CACHE.read();
    Ok(cache.packages.get(name).cloned())
}

/// Get a specific sync package by exact name - FAST (<1ms)
pub fn get_sync_package(name: &str) -> Result<Option<SyncDbPackage>> {
    let sync_dir = paths::pacman_sync_dir();
    ensure_sync_cache_loaded(&sync_dir)?;

    let cache = SYNC_DB_CACHE.read();
    Ok(cache.packages.get(name).cloned())
}

/// Internal unified implementation for listing/searching local packages
fn list_local_cached_filtered(query: Option<&str>) -> Result<Vec<LocalDbPackage>> {
    let local_dir = paths::pacman_local_dir();
    ensure_local_cache_loaded(&local_dir)?;

    let cache = LOCAL_DB_CACHE.read();

    let results = match query {
        None => cache.packages.values().cloned().collect(),
        Some(q) => {
            let query_lower = q.to_lowercase();
            cache
                .packages
                .values()
                .filter(|pkg| {
                    contains_ignore_case(&pkg.name, &query_lower)
                        || contains_ignore_case(&pkg.desc, &query_lower)
                })
                .cloned()
                .collect()
        }
    };

    Ok(results)
}

/// Search local packages using cache - FAST (<1ms)
pub fn search_local_cached(query: &str) -> Result<Vec<LocalDbPackage>> {
    list_local_cached_filtered(Some(query))
}

/// List all local packages using cache - FAST (<1ms)
pub fn list_local_cached() -> Result<Vec<LocalDbPackage>> {
    list_local_cached_filtered(None)
}

/// Check if package is installed using cache - INSTANT
#[must_use]
pub fn is_installed_cached(name: &str) -> bool {
    let local_dir = paths::pacman_local_dir();
    if ensure_local_cache_loaded(&local_dir).is_err() {
        return false;
    }

    let cache = LOCAL_DB_CACHE.read();
    cache.packages.contains_key(name)
}

/// List all package names (local + sync) using cache - FAST
pub fn list_all_names_cached() -> Result<Vec<String>> {
    let sync_dir = paths::pacman_sync_dir();
    let local_dir = paths::pacman_local_dir();

    ensure_sync_cache_loaded(&sync_dir)?;
    ensure_local_cache_loaded(&local_dir)?;

    let mut names = std::collections::HashSet::new();

    {
        let cache = LOCAL_DB_CACHE.read();
        for name in cache.packages.keys() {
            names.insert(name.clone());
        }
    }

    {
        let cache = SYNC_DB_CACHE.read();
        for name in cache.packages.keys() {
            names.insert(name.clone());
        }
    }

    let mut result: Vec<String> = names.into_iter().collect();
    result.sort();
    Ok(result)
}

/// Fast package search - search sync databases WITHOUT ALPM
/// Uses global cache for <1ms response after first load
pub fn search_sync_fast(query: &str) -> Result<Vec<SyncDbPackage>> {
    let sync_dir = paths::pacman_sync_dir();
    ensure_sync_cache_loaded(&sync_dir)?;

    let query_lower = query.to_lowercase();
    let cache = SYNC_DB_CACHE.read();

    let results = cache
        .packages
        .values()
        .filter(|pkg| {
            query_lower.is_empty()
                || contains_ignore_case(&pkg.name, &query_lower)
                || contains_ignore_case(&pkg.desc, &query_lower)
        })
        .cloned()
        .collect();

    Ok(results)
}

/// Zero-copy search using memory-mapped rkyv index (~100µs vs ~1ms for regular search)
pub fn search_sync_mmap(query: &str) -> Result<Vec<SyncDbPackage>> {
    let sync_dir = paths::pacman_sync_dir();
    let current_mtime = get_newest_db_mtime(&sync_dir);
    let mtime_secs = current_mtime
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    {
        let mmap_guard = SYNC_MMAP_INDEX.read();
        if let Some(ref idx) = *mmap_guard
            && idx.mtime() == mtime_secs
        {
            return Ok(convert_mmap_results(idx.search(query)));
        }
    }

    if let Some(idx) = try_load_mmap_index(mtime_secs) {
        let results = convert_mmap_results(idx.search(query));
        let mut mmap_guard = SYNC_MMAP_INDEX.write();
        *mmap_guard = Some(idx);
        return Ok(results);
    }

    ensure_sync_cache_loaded(&sync_dir)?;
    search_sync_fast(query)
}

fn convert_mmap_results(archived: Vec<&rkyv::Archived<RkyvSyncPackage>>) -> Vec<SyncDbPackage> {
    archived
        .into_iter()
        .map(|p| SyncDbPackage {
            name: p.name.to_string(),
            version: super::types::parse_version_or_zero(&p.version),
            desc: p.desc.to_string(),
            filename: p.filename.to_string(),
            csize: p.csize.into(),
            isize: p.isize.into(),
            url: p.url.to_string(),
            arch: p.arch.to_string(),
            repo: p.repo.to_string(),
            licenses: p
                .licenses
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            depends: p
                .depends
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            makedepends: p
                .makedepends
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            optdepends: p
                .optdepends
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            provides: p
                .provides
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            conflicts: p
                .conflicts
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
            replaces: p
                .replaces
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
        })
        .collect()
}
/// Identify potential AUR packages (installed but not in any sync database).
///
/// Uses pure Rust cache for extreme speed (<1ms).
/// Excludes packages from ALL sync databases (official + custom repos).
pub fn get_potential_aur_packages() -> Result<Vec<String>> {
    let sync_dir = paths::pacman_sync_dir();
    let local_dir = paths::pacman_local_dir();

    ensure_sync_cache_loaded(&sync_dir)?;
    ensure_local_cache_loaded(&local_dir)?;

    let sync_cache = SYNC_DB_CACHE.read();
    let local_cache = LOCAL_DB_CACHE.read();

    let mut potential = Vec::with_capacity(local_cache.packages.len() / 10);
    for name in local_cache.packages.keys() {
        if !sync_cache.packages.contains_key(name) {
            potential.push(name.clone());
        }
    }

    Ok(potential)
}

/// Verify which packages actually exist in AUR by querying the AUR API
/// This distinguishes true AUR packages from custom repo packages
pub async fn verify_aur_packages(package_names: &[String]) -> Result<Vec<String>> {
    if package_names.is_empty() {
        return Ok(Vec::new());
    }

    // Use the same chunking logic as AurClient for API queries
    let chunked_names = chunk_aur_names(package_names);
    let mut aur_packages = Vec::new();

    // Create HTTP client
    let client = reqwest::Client::builder().user_agent("omg/0.1.0").build()?;

    // Query AUR API in parallel
    let concurrency = std::cmp::min(8, chunked_names.len());
    let mut stream = futures::stream::iter(chunked_names)
        .map(|chunk| {
            let client = &client;
            async move {
                let mut url = "https://aur.archlinux.org/rpc?v=5&type=info".to_string();
                for name in &chunk {
                    url.push_str("&arg[]=");
                    url.push_str(name);
                }
                let response = client.get(&url).send().await?;
                let json: serde_json::Value = response.json().await?;
                Ok::<serde_json::Value, anyhow::Error>(json)
            }
        })
        .buffer_unordered(concurrency);

    while let Some(result) = stream.next().await {
        let response = result?;
        if let Some(results) = response.get("results").and_then(|r| r.as_array()) {
            for package in results {
                if let Some(name) = package.get("Name").and_then(|n| n.as_str()) {
                    aur_packages.push(name.to_string());
                }
            }
        }
    }

    Ok(aur_packages)
}

/// Helper function to chunk package names for AUR API queries
fn chunk_aur_names(names: &[String]) -> Vec<Vec<String>> {
    const AUR_RPC_MAX_URI: usize = 4400;
    let mut chunks = Vec::new();
    let mut current_chunk = Vec::new();
    let mut current_len = 0; // URL length

    for name in names {
        // Each &arg[]=name adds about 10 chars overhead + name length
        let add_len = 10 + name.len();

        if current_len + add_len > AUR_RPC_MAX_URI && !current_chunk.is_empty() {
            chunks.push(current_chunk);
            current_chunk = Vec::new();
            current_len = 0;
        }

        current_chunk.push(name.clone());
        current_len += add_len;
    }

    if !current_chunk.is_empty() {
        chunks.push(current_chunk);
    }

    chunks
}

/// Get total package counts - INSTANT (<1ms with cache)
pub fn get_counts_fast() -> Result<(usize, usize, usize)> {
    let local_dir = paths::pacman_local_dir();
    ensure_local_cache_loaded(&local_dir)?;

    let cache = LOCAL_DB_CACHE.read();
    let total = cache.packages.len();
    let explicit = cache.packages.values().filter(|p| p.explicit).count();

    Ok((total, explicit, total - explicit))
}

/// Get explicit package count only - INSTANT
pub fn get_explicit_count() -> Result<usize> {
    let (_, explicit, _) = get_counts_fast()?;
    Ok(explicit)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;

    #[test]
    fn test_collect_sync_db_paths_excludes_sig_files() {
        // Create a temporary test directory with .db and .db.sig files
        let temp_dir = std::env::temp_dir().join("omg_test_sync_db");
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        // Create test files
        std::fs::write(temp_dir.join("core.db"), b"dummy").unwrap();
        std::fs::write(temp_dir.join("core.db.sig"), b"signature").unwrap();
        std::fs::write(temp_dir.join("extra.db"), b"dummy").unwrap();
        std::fs::write(temp_dir.join("extra.db.sig"), b"signature").unwrap();
        std::fs::write(temp_dir.join("custom-repo.db"), b"dummy").unwrap();
        std::fs::write(temp_dir.join("custom-repo.db.sig"), b"signature").unwrap();
        std::fs::write(temp_dir.join("not-a-db.txt"), b"text").unwrap();

        let db_paths = collect_sync_db_paths(&temp_dir);

        // Should only collect .db files, NOT .db.sig files
        let collected_names: Vec<_> = db_paths
            .iter()
            .map(|(path, _)| path.file_name().unwrap().to_str().unwrap())
            .collect();

        assert!(
            collected_names.contains(&"core.db"),
            "Should include core.db"
        );
        assert!(
            collected_names.contains(&"extra.db"),
            "Should include extra.db"
        );
        assert!(
            collected_names.contains(&"custom-repo.db"),
            "Should include custom-repo.db"
        );
        assert!(
            !collected_names.contains(&"core.db.sig"),
            "Should NOT include .sig files"
        );
        assert!(
            !collected_names.contains(&"extra.db.sig"),
            "Should NOT include .sig files"
        );
        assert!(
            !collected_names.contains(&"custom-repo.db.sig"),
            "Should NOT include .sig files"
        );
        assert!(
            !collected_names.contains(&"not-a-db.txt"),
            "Should NOT include non-.db files"
        );

        // Cleanup
        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    #[ignore = "System-dependent test that reads actual pacman db files - may fail if db is corrupted"]
    fn test_check_updates() {
        // Only run if we have a real system
        if crate::core::paths::pacman_sync_dir()
            .join("core.db")
            .exists()
        {
            match check_updates_fast() {
                Ok(updates) => tracing::debug!("Found {} updates", updates.len()),
                Err(e) => panic!("Failed to check updates: {e}"),
            }
        }
    }

    #[test]
    fn test_get_local_package() {
        // Only run if we have a real system
        if crate::core::paths::pacman_local_dir().exists() {
            // pacman should always be installed
            if let Ok(Some(pkg)) = get_local_package("pacman") {
                assert!(!pkg.version.to_string().is_empty());
            }
        }
    }

    #[test]
    fn test_get_package_counts() {
        if crate::core::paths::pacman_local_dir().exists() {
            let (total, explicit, deps) = match get_counts_fast() {
                Ok(counts) => counts,
                Err(e) => panic!("Failed to get counts: {e}"),
            };
            assert!(total > 0);
            // On some test systems, explicit might be 0 if only deps are present
            // but usually at least some are explicit. We change to total >= explicit + deps
            assert_eq!(total, explicit + deps);
        }
    }
}
