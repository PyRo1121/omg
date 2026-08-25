//! Pure Rust pacman database parser.
//!
//! Parses /var/lib/pacman/sync/*.db and /var/lib/pacman/local/ without
//! libalpm via direct tar.gz/tar.zst parsing. First load parses all DBs;
//! subsequent lookups are served from the in-memory cache.

use alpm_db;
use alpm_repo_db;
use alpm_types::Version;
use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::RwLock;

use std::time::SystemTime;
use tracing::instrument;

use crate::core::paths;

/// TTL for cache eviction safety net (30 minutes)
const CACHE_TTL_SECS: u64 = 30 * 60;

/// Global cache for sync databases - parsed once, used forever until invalidated
static SYNC_DB_CACHE: std::sync::LazyLock<RwLock<DbCache>> =
    std::sync::LazyLock::new(|| RwLock::new(DbCache::default()));

/// Global cache for local database
static LOCAL_DB_CACHE: std::sync::LazyLock<RwLock<LocalDbCache>> =
    std::sync::LazyLock::new(|| RwLock::new(LocalDbCache::default()));

#[derive(Default, Serialize, Deserialize)]
struct DbCache {
    packages: HashMap<String, SyncDbPackage>,
    last_modified: Option<SystemTime>,
    #[serde(skip)]
    last_accessed: Option<SystemTime>,
}

fn load_sync_packages(sync_dir: &Path) -> Result<HashMap<String, SyncDbPackage>> {
    let db_paths = collect_sync_db_paths(sync_dir)?;
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

fn collect_sync_db_paths(sync_dir: &Path) -> Result<Vec<(PathBuf, String)>> {
    // Pre-allocate for standard repos (core, extra, multilib) plus potential custom repos
    let mut dbs = Vec::with_capacity(8);

    if !sync_dir.exists() {
        return Ok(dbs);
    }

    for db_name in &["core", "extra", "multilib"] {
        let db_path = sync_dir.join(format!("{db_name}.db"));
        if db_path.exists() {
            dbs.push((db_path, (*db_name).to_string()));
        }
    }

    for entry in std::fs::read_dir(sync_dir).with_context(|| {
        format!(
            "Failed to read pacman sync directory {}",
            sync_dir.display()
        )
    })? {
        let entry = entry.with_context(|| {
            format!(
                "Failed to read pacman sync directory entry in {}",
                sync_dir.display()
            )
        })?;
        let path = entry.path();
        let meta = entry.metadata().with_context(|| {
            format!(
                "Failed to read pacman sync file metadata {}",
                path.display()
            )
        })?;
        if !meta.is_file() {
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
            && !matches!(name.as_str(), "core" | "extra" | "multilib")
        {
            dbs.push((path, name));
        }
    }

    Ok(dbs)
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
            version: super::super::types::zero_version(),
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
    /// Packages that hard-require this package (`%REQUIREDBY%`).
    pub required_by: Vec<String>,
    /// Packages that optionally depend on this package (`%OPTFOR%`).
    pub optional_for: Vec<String>,
}

impl Default for LocalDbPackage {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: super::super::types::zero_version(),
            desc: String::new(),
            install_date: String::new(),
            licenses: Vec::new(),
            explicit: false,
            required_by: Vec::new(),
            optional_for: Vec::new(),
        }
    }
}

/// Parse a sync database file (core.db, extra.db, multilib.db)
/// Returns a `HashMap` of package name -> `SyncDbPackage`
pub fn parse_sync_db(path: &Path, repo_name: &str) -> Result<HashMap<String, SyncDbPackage>> {
    let file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;

    // Detect compression type from the first magic bytes, then hand the
    // already-opened file to the matching decoder (single open per DB).
    let reader: Box<dyn Read> = {
        let path_str = path.to_string_lossy();
        if path_str.ends_with(".db") || path_str.ends_with(".zst") {
            let mut magic = [0u8; 4];
            let mut prefix = file.take(4);
            prefix.read_exact(&mut magic)?;
            let mut file = prefix.into_inner();
            // The magic probe advanced the stream; decoders must see byte 0.
            file.rewind()?;

            if magic[0..2] == [0x1f, 0x8b] {
                Box::new(GzDecoder::new(file))
            } else if magic[0..4] == [0x28, 0xb5, 0x2f, 0xfd] {
                let mut decoder = ruzstd::decoding::StreamingDecoder::new(file)
                    .map_err(|e| anyhow::anyhow!("zstd init: {e}"))?;
                let mut decompressed = Vec::new();
                std::io::copy(&mut decoder, &mut decompressed)?;
                Box::new(std::io::Cursor::new(decompressed))
            } else {
                // Unknown magic: fall back to gzip, matching pacman defaults.
                Box::new(GzDecoder::new(file))
            }
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
            entry.read_to_string(&mut content).with_context(|| {
                format!(
                    "Failed to read desc {} from repo {repo_name}",
                    entry_path.display()
                )
            })?;

            let pkg = parse_desc_content(&content, repo_name)?;
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
            version: $desc.version.into(),
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

fn parse_desc_content(content: &str, repo: &str) -> Result<SyncDbPackage> {
    // Try V2 first (newer format without MD5SUM).
    // Wrap in catch_unwind because alpm_repo_db can panic on corrupted data
    // (e.g., PackageRelation::from_str panics on malformed dependency strings).
    let v2_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        alpm_repo_db::desc::RepoDescFileV2::from_str(content)
    }));

    match v2_result {
        Ok(Ok(desc)) => return Ok(sync_pkg_from_desc!(desc, repo)),
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
        Ok(Ok(desc)) => return Ok(sync_pkg_from_desc!(desc, repo)),
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

fn parse_desc_manual(content: &str, repo: &str) -> Result<SyncDbPackage> {
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
                pkg.version = require_package_version(line)?;
            }
            "%DESC%" => pkg.desc = line.to_string(),
            "%URL%" => pkg.url = line.to_string(),
            "%ARCH%" => pkg.arch = line.to_string(),
            "%CSIZE%" => {
                pkg.csize = line
                    .parse()
                    .with_context(|| format!("Invalid compressed size in repo {repo}: {line}"))?;
            }
            "%ISIZE%" => {
                pkg.isize = line
                    .parse()
                    .with_context(|| format!("Invalid installed size in repo {repo}: {line}"))?;
            }
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

    Ok(pkg)
}

/// Parse the local package database (/var/lib/pacman/local/)
/// Returns a `HashMap` of package name -> `LocalDbPackage`
pub fn parse_local_db(path: &Path) -> Result<HashMap<String, LocalDbPackage>> {
    let mut packages = HashMap::with_capacity(2000); // Pre-allocate for typical system

    if !path.exists() {
        return Ok(packages);
    }

    for entry in std::fs::read_dir(path)
        .with_context(|| format!("Failed to read pacman local directory {}", path.display()))?
    {
        let entry = entry.with_context(|| {
            format!(
                "Failed to read pacman local directory entry in {}",
                path.display()
            )
        })?;
        let pkg_path = entry.path();
        let meta = entry.metadata().with_context(|| {
            format!(
                "Failed to read pacman local package metadata {}",
                pkg_path.display()
            )
        })?;
        if !meta.is_dir() {
            continue;
        }

        let desc_path = pkg_path.join("desc");
        if !desc_path.exists() {
            anyhow::bail!(
                "Local package directory {} is missing desc",
                pkg_path.display()
            );
        }

        let pkg = parse_local_desc(&desc_path)?;
        packages.insert(pkg.name.clone(), pkg);
    }

    Ok(packages)
}

fn require_package_version(raw: &str) -> Result<Version> {
    Version::from_str(raw)
        .map_err(|error| anyhow::anyhow!("Invalid package version {raw}: {error}"))
}

fn parse_local_desc(path: &Path) -> Result<LocalDbPackage> {
    let content = std::fs::read_to_string(path)?;

    // The typed `alpm-db` schemas do not model `%REQUIREDBY%`/`%OPTFOR%`, so
    // those sections are always scanned directly; they feed the canonical
    // orphan rule (`types::is_orphan_package`).
    let (required_by, optional_for) = extract_local_relations(&content);

    let mut pkg = if let Ok(desc) = alpm_db::desc::DbDescFileV1::from_str(&content) {
        LocalDbPackage {
            name: desc.name.to_string(),
            version: desc.version.into(),
            desc: desc.description.to_string(),
            install_date: desc.installdate.to_string(),
            licenses: desc.license.iter().map(ToString::to_string).collect(),
            explicit: matches!(desc.reason, alpm_types::PackageInstallReason::Explicit),
            ..LocalDbPackage::default()
        }
    } else if let Ok(desc) = alpm_db::desc::DbDescFileV2::from_str(&content) {
        // V2 (has XDATA support)
        LocalDbPackage {
            name: desc.name.to_string(),
            version: desc.version.into(),
            desc: desc.description.to_string(),
            install_date: desc.installdate.to_string(),
            licenses: desc.license.iter().map(ToString::to_string).collect(),
            explicit: matches!(desc.reason, alpm_types::PackageInstallReason::Explicit),
            ..LocalDbPackage::default()
        }
    } else {
        // Fallback: manual parsing for edge cases
        parse_local_desc_manual(&content)?
    };

    pkg.required_by = required_by;
    pkg.optional_for = optional_for;
    Ok(pkg)
}

/// Extract `%REQUIREDBY%` and `%OPTFOR%`/`%OPTIONALFOR%` sections from a
/// local desc file. Both historical spellings are accepted.
fn extract_local_relations(content: &str) -> (Vec<String>, Vec<String>) {
    let mut required_by = Vec::new();
    let mut optional_for = Vec::new();
    let mut section = "";

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('%') && line.ends_with('%') {
            section = line;
            continue;
        }
        match section {
            "%REQUIREDBY%" => required_by.push(line.to_string()),
            "%OPTFOR%" | "%OPTIONALFOR%" => optional_for.push(line.to_string()),
            _ => {}
        }
    }

    (required_by, optional_for)
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

    // pacman writes 0 for explicitly installed and 1 for dependencies.
    // Anything else is malformed; fail loudly instead of guessing.
    let explicit = match reason.as_str() {
        "" | "0" => true,
        "1" => false,
        other => anyhow::bail!("Invalid %REASON% value in local desc: {other}"),
    };

    Ok(LocalDbPackage {
        name,
        version: require_package_version(&version)?,
        desc,
        install_date,
        licenses,
        explicit,
        ..LocalDbPackage::default()
    })
}

/// Save cache to disk in binary format
fn save_cache_to_disk<T: Serialize>(cache: &T, name: &str) -> Result<()> {
    save_cache_to_disk_in(cache, &paths::cache_dir(), name)
}

fn save_cache_to_disk_in<T: Serialize>(cache: &T, cache_dir: &Path, name: &str) -> Result<()> {
    fs::create_dir_all(cache_dir).with_context(|| {
        format!(
            "Failed to create package cache directory: {}",
            cache_dir.display()
        )
    })?;
    let path = cache_dir.join(format!("{name}.bin"));
    let data = bitcode::serialize(cache)?;
    let mut file = tempfile::NamedTempFile::new_in(cache_dir).with_context(|| {
        format!(
            "Failed to create temporary package cache in {}",
            cache_dir.display()
        )
    })?;
    file.write_all(&data)?;
    file.as_file_mut().sync_all()?;
    file.persist(&path)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to persist package cache at {}", path.display()))?;
    Ok(())
}

fn persist_cache_best_effort<T: Serialize>(cache: &T, name: &str) {
    if let Err(error) = save_cache_to_disk(cache, name) {
        tracing::warn!("Failed to persist {name} package cache: {error}");
    }
}

/// Load cache from disk.
///
/// SECURITY (audit sec04 F1): when omg runs elevated (root via sudo), the
/// cache directory still belongs to the ORIGINAL user, who is fully
/// adversarial relative to the root process. Parsing attacker-writable
/// bytes as trusted derived state lets them poison what root reads/writes.
/// Elevated runs therefore bypass user-owned derived caches entirely and go
/// to ground truth (ALPM/dpkg); the cache remains a fast path for the
/// unprivileged user's own sessions.
fn load_cache_from_disk<T: for<'de> Deserialize<'de>>(name: &str) -> Result<T> {
    #[cfg(unix)]
    if crate::core::is_root() && !paths::test_mode() {
        use std::os::unix::fs::MetadataExt as _;
        let dir = paths::cache_dir();
        if let Ok(meta) = std::fs::metadata(&dir)
            && meta.uid() != 0
        {
            tracing::debug!(
                "Elevated run: ignoring user-owned derived cache {}",
                dir.display()
            );
            anyhow::bail!("derived cache skipped under elevation");
        }
    }
    load_cache_from_disk_in(&paths::cache_dir(), name)
}

fn load_cache_from_disk_in<T: for<'de> Deserialize<'de>>(
    cache_dir: &Path,
    name: &str,
) -> Result<T> {
    let path = cache_dir.join(format!("{name}.bin"));
    let data = fs::read(&path)?;
    let cache: T = bitcode::deserialize(&data)?;
    Ok(cache)
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

fn is_cache_reusable(
    cache_mtime: Option<SystemTime>,
    current_mtime: SystemTime,
    has_packages: bool,
    last_accessed: Option<SystemTime>,
) -> bool {
    cache_mtime == Some(current_mtime) && has_packages && !is_cache_expired(last_accessed)
}

/// Ensure sync cache is loaded (fast if already loaded)
fn ensure_sync_cache_loaded(sync_dir: &Path) -> Result<()> {
    let current_mtime = get_newest_db_mtime(sync_dir)?;

    {
        let mut cache = SYNC_DB_CACHE
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if is_cache_reusable(
            cache.last_modified,
            current_mtime,
            !cache.packages.is_empty(),
            cache.last_accessed,
        ) {
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
        && is_cache_reusable(
            disk_cache.last_modified,
            current_mtime,
            !disk_cache.packages.is_empty(),
            disk_cache.last_accessed,
        )
    {
        let mut cache = SYNC_DB_CACHE
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // Double-check: another thread may have loaded while we were waiting
        if is_cache_reusable(
            cache.last_modified,
            current_mtime,
            !cache.packages.is_empty(),
            cache.last_accessed,
        ) {
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
    let mut cache = SYNC_DB_CACHE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    // Re-check: another thread may have loaded while we were parsing
    if is_cache_reusable(
        cache.last_modified,
        current_mtime,
        !cache.packages.is_empty(),
        cache.last_accessed,
    ) {
        cache.last_accessed = Some(SystemTime::now());
        return Ok(());
    }

    cache.packages = packages;
    cache.last_modified = Some(current_mtime);
    cache.last_accessed = Some(SystemTime::now());

    // Persist for faster restarts; the in-memory cache is authoritative
    // for this process, so a disk write failure is logged, not fatal.
    persist_cache_best_effort(&*cache, "sync_db");

    Ok(())
}

/// Ensure local cache is loaded (fast if already loaded)
fn ensure_local_cache_loaded(local_dir: &Path) -> Result<()> {
    let current_mtime = get_local_db_mtime(local_dir)?;

    {
        let mut cache = LOCAL_DB_CACHE
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if is_cache_reusable(
            cache.last_modified,
            current_mtime,
            !cache.packages.is_empty(),
            cache.last_accessed,
        ) {
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
        && is_cache_reusable(
            disk_cache.last_modified,
            current_mtime,
            !disk_cache.packages.is_empty(),
            disk_cache.last_accessed,
        )
    {
        let mut cache = LOCAL_DB_CACHE
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        // Double-check: another thread may have loaded while we were waiting
        if is_cache_reusable(
            cache.last_modified,
            current_mtime,
            !cache.packages.is_empty(),
            cache.last_accessed,
        ) {
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
    let mut cache = LOCAL_DB_CACHE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    // Double-check: another thread may have loaded while we were parsing
    if is_cache_reusable(
        cache.last_modified,
        current_mtime,
        !cache.packages.is_empty(),
        cache.last_accessed,
    ) {
        cache.last_accessed = Some(SystemTime::now());
        return Ok(());
    }

    cache.packages = packages;
    cache.last_modified = Some(current_mtime);
    cache.last_accessed = Some(SystemTime::now());

    persist_cache_best_effort(&*cache, "local_db");

    Ok(())
}

/// Get newest modification time of sync DBs
fn get_newest_db_mtime(sync_dir: &Path) -> Result<SystemTime> {
    if !sync_dir.exists() {
        return Ok(SystemTime::UNIX_EPOCH);
    }

    let mut newest = SystemTime::UNIX_EPOCH;
    for entry in std::fs::read_dir(sync_dir).with_context(|| {
        format!(
            "Failed to read pacman sync directory {}",
            sync_dir.display()
        )
    })? {
        let entry = entry.with_context(|| {
            format!(
                "Failed to read pacman sync directory entry in {}",
                sync_dir.display()
            )
        })?;
        let path = entry.path();
        let meta = entry.metadata().with_context(|| {
            format!(
                "Failed to read pacman sync file metadata {}",
                path.display()
            )
        })?;
        let mtime = meta
            .modified()
            .with_context(|| format!("Failed to read modification time for {}", path.display()))?;
        if mtime > newest {
            newest = mtime;
        }
    }

    Ok(newest)
}

/// Get modification time of local db directory
fn get_local_db_mtime(local_dir: &Path) -> Result<SystemTime> {
    let meta = std::fs::metadata(local_dir)?;
    Ok(meta.modified()?)
}

/// Force refresh of all caches (call after sync/install)
pub fn invalidate_caches() -> Result<()> {
    {
        let mut cache = SYNC_DB_CACHE
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.packages.clear();
        cache.last_modified = None;
    }
    {
        let mut cache = LOCAL_DB_CACHE
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        cache.packages.clear();
        cache.last_modified = None;
    }
    super::super::alpm_direct::clear_alpm_cache();

    let cache_dir = paths::cache_dir();
    remove_cache_file(&cache_dir.join("sync_db.bin"))?;
    remove_cache_file(&cache_dir.join("local_db.bin"))?;
    Ok(())
}

fn remove_cache_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to remove package cache file {}", path.display())),
    }
}

/// Detailed sync packages for indexing consumers (daemon, resolver).
pub fn get_detailed_packages() -> Result<Vec<SyncDbPackage>> {
    let sync_dir = paths::pacman_sync_dir();
    ensure_sync_cache_loaded(&sync_dir)?;

    let cache = SYNC_DB_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok(cache.packages.values().cloned().collect())
}

/// A pending update derived from the pure-Rust pacman caches.
#[derive(Debug, Clone)]
pub struct CachedUpdate {
    pub name: String,
    pub old_version: Version,
    pub new_version: Version,
    pub repo: String,
}

/// Cached update check across all configured repositories.
#[instrument]
pub fn check_updates_cached() -> Result<Vec<CachedUpdate>> {
    let sync_dir = paths::pacman_sync_dir();
    let local_dir = paths::pacman_local_dir();

    // Ensure caches are loaded (will be fast if already loaded)
    ensure_sync_cache_loaded(&sync_dir)?;
    ensure_local_cache_loaded(&local_dir)?;

    // Hold both cache locks simultaneously - no cloning!
    let sync_cache = SYNC_DB_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let local_cache = LOCAL_DB_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

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
            CachedUpdate {
                name: name.clone(),
                old_version: local_pkg.version.clone(),
                new_version: sync_pkg.version.clone(),
                repo: sync_pkg.repo.clone(),
            }
        })
        .collect();

    Ok(updates)
}

/// Get a specific local package - FAST (<1ms)
#[inline]
pub fn get_local_package(name: &str) -> Result<Option<LocalDbPackage>> {
    let local_dir = paths::pacman_local_dir();
    ensure_local_cache_loaded(&local_dir)?;

    let cache = LOCAL_DB_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok(cache.packages.get(name).cloned())
}

/// Get a specific sync package by exact name - FAST (<1ms)
#[inline]
pub fn get_sync_package(name: &str) -> Result<Option<SyncDbPackage>> {
    let sync_dir = paths::pacman_sync_dir();
    ensure_sync_cache_loaded(&sync_dir)?;

    let cache = SYNC_DB_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok(cache.packages.get(name).cloned())
}

/// List all local packages using cache - FAST (<1ms)
pub fn list_local_cached() -> Result<Vec<LocalDbPackage>> {
    let local_dir = paths::pacman_local_dir();
    ensure_local_cache_loaded(&local_dir)?;

    let cache = LOCAL_DB_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    Ok(cache.packages.values().cloned().collect())
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

    let sync_cache = SYNC_DB_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let local_cache = LOCAL_DB_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let mut potential = Vec::with_capacity(local_cache.packages.len() / 10);
    for name in local_cache.packages.keys() {
        if !sync_cache.packages.contains_key(name) {
            potential.push(name.clone());
        }
    }

    Ok(potential)
}

/// Get total package counts - INSTANT (<1ms with cache)
///
/// The third element counts true orphans under the canonical rule
/// (`types::is_orphan_package`): dependencies that nothing requires or
/// optionally requires.
#[inline]
pub fn get_counts_fast() -> Result<(usize, usize, usize)> {
    let local_dir = paths::pacman_local_dir();
    ensure_local_cache_loaded(&local_dir)?;

    let cache = LOCAL_DB_CACHE
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let total = cache.packages.len();
    let mut explicit = 0;
    let mut orphans = 0;
    for pkg in cache.packages.values() {
        if pkg.explicit {
            explicit += 1;
        } else if crate::package_managers::types::is_orphan_package(
            pkg.explicit,
            pkg.required_by.is_empty(),
            pkg.optional_for.is_empty(),
        ) {
            orphans += 1;
        }
    }

    Ok((total, explicit, orphans))
}

/// Get explicit package count only - INSTANT
#[inline]
pub fn get_explicit_count() -> Result<usize> {
    let (_, explicit, _) = get_counts_fast()?;
    Ok(explicit)
}

#[cfg(test)]
#[expect(clippy::unwrap_used, clippy::expect_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;

    /// Regression: the compression-format probe must rewind the stream before
    /// handing it to a decoder. It previously consumed the first 4 bytes, so
    /// every real sync database failed to parse ("invalid gzip header" /
    /// "zstd init: BadMagicNumber").
    #[test]
    fn parse_sync_db_handles_gzip_and_zstd_after_magic_probe() {
        let temp = tempfile::TempDir::new().unwrap();

        // Modeled on a real core.db desc entry (single-percent sections).
        let desc = b"%FILENAME%\nbash-5.3.15-1-x86_64.pkg.tar.zst\n\n\
%NAME%\nbash\n\n\
%BASE%\nbash\n\n\
%VERSION%\n5.3.15-1\n\n\
%DESC%\nThe GNU Bourne Again shell\n\n\
%CSIZE%\n150821\n\n\
%ISIZE%\n353496\n\n\
%SHA256SUM%\ndca9ec50cf51243b86a67367a55e78b1851d31240b1edaf9c011f8511765d999\n\n\
%URL%\nhttps://www.gnu.org/software/bash/\n\n\
%LICENSE%\nGPL-3.0-or-later\n\n\
%ARCH%\nx86_64\n\n\
%BUILDDATE%\n1700000000\n\n\
%PACKAGER%\nArch Linux <archlinux@archlinux.org>\n\n";

        let build_tar = || -> Vec<u8> {
            let mut tar = tar::Builder::new(Vec::new());
            let mut header = tar::Header::new_gnu();
            header.set_size(desc.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, "bash-5.3.15-1/desc", &desc[..])
                .unwrap();
            tar.into_inner().unwrap()
        };
        let raw = build_tar();

        let mut gzip_bytes = Vec::new();
        flate2::write::GzEncoder::new(&mut gzip_bytes, flate2::Compression::fast())
            .write_all(&raw)
            .unwrap();
        let gzip_path = temp.path().join("core.db");
        std::fs::write(&gzip_path, &gzip_bytes).unwrap();
        let parsed = parse_sync_db(&gzip_path, "core").expect("gzip db must parse after probe");
        assert!(
            parsed.contains_key("bash"),
            "bash must be found, got keys {:?}",
            parsed.keys()
        );

        let zstd_bytes = zstd::stream::encode_all(&raw[..], 3).unwrap();
        let zstd_path = temp.path().join("custom.db");
        std::fs::write(&zstd_path, &zstd_bytes).unwrap();
        let parsed = parse_sync_db(&zstd_path, "custom").expect("zstd db must parse after probe");
        assert!(
            parsed.contains_key("bash"),
            "bash must be found, got keys {:?}",
            parsed.keys()
        );
    }

    #[test]
    fn test_collect_sync_db_paths_excludes_sig_files() {
        let temp_dir = tempfile::TempDir::new().unwrap();

        std::fs::write(temp_dir.path().join("core.db"), b"dummy").unwrap();
        std::fs::write(temp_dir.path().join("core.db.sig"), b"signature").unwrap();
        std::fs::write(temp_dir.path().join("extra.db"), b"dummy").unwrap();
        std::fs::write(temp_dir.path().join("extra.db.sig"), b"signature").unwrap();
        std::fs::write(temp_dir.path().join("custom-repo.db"), b"dummy").unwrap();
        std::fs::write(temp_dir.path().join("custom-repo.db.sig"), b"signature").unwrap();
        std::fs::write(temp_dir.path().join("not-a-db.txt"), b"text").unwrap();

        let db_paths = collect_sync_db_paths(temp_dir.path()).unwrap();

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
    }

    #[test]
    fn test_collect_sync_db_paths_missing_dir_is_empty() {
        let missing = tempfile::TempDir::new()
            .unwrap()
            .path()
            .join("does-not-exist");
        let db_paths = collect_sync_db_paths(&missing).unwrap();
        assert!(db_paths.is_empty());
    }

    #[test]
    fn test_collect_sync_db_paths_unreadable_dir_errors() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let original = std::fs::metadata(temp_dir.path()).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(temp_dir.path(), std::fs::Permissions::from_mode(0o000))
                .unwrap();
        }
        let blocked = std::fs::read_dir(temp_dir.path()).is_err();
        let result = collect_sync_db_paths(temp_dir.path());
        let _ = std::fs::set_permissions(temp_dir.path(), original);
        if !blocked {
            return;
        }
        assert!(
            result.is_err(),
            "unreadable sync dir must fail closed, got {result:?}"
        );
    }

    #[test]
    fn test_parse_local_db_missing_dir_is_empty() {
        let missing = tempfile::TempDir::new()
            .unwrap()
            .path()
            .join("does-not-exist");
        let packages = parse_local_db(&missing).unwrap();
        assert!(packages.is_empty());
    }

    #[test]
    fn test_parse_local_db_missing_desc_errors() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(temp_dir.path().join("vim-9.1.0-1")).unwrap();
        let error = parse_local_db(temp_dir.path()).unwrap_err();
        assert!(
            error.to_string().contains("missing desc"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn test_parse_local_desc_invalid_version_errors() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let pkg_dir = temp_dir.path().join("vim-bad");
        std::fs::create_dir(&pkg_dir).unwrap();
        std::fs::write(
            pkg_dir.join("desc"),
            "%NAME%\nvim\n\n%VERSION%\nnot a version!!!\n\n%DESC%\nVi Improved\n",
        )
        .unwrap();
        let error = parse_local_db(temp_dir.path()).unwrap_err();
        assert!(
            error.to_string().contains("Invalid package version"),
            "unexpected error: {error:#}"
        );
    }

    #[test]
    fn test_get_newest_db_mtime_missing_dir_is_epoch() {
        let missing = tempfile::TempDir::new()
            .unwrap()
            .path()
            .join("does-not-exist");
        let mtime = get_newest_db_mtime(&missing).unwrap();
        assert_eq!(mtime, SystemTime::UNIX_EPOCH);
    }

    #[test]
    fn test_get_newest_db_mtime_unreadable_dir_errors() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let original = std::fs::metadata(temp_dir.path()).unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(temp_dir.path(), std::fs::Permissions::from_mode(0o000))
                .unwrap();
        }
        let blocked = std::fs::read_dir(temp_dir.path()).is_err();
        let result = get_newest_db_mtime(temp_dir.path());
        let _ = std::fs::set_permissions(temp_dir.path(), original);
        if !blocked {
            return;
        }
        assert!(
            result.is_err(),
            "unreadable sync dir must fail closed, got {result:?}"
        );
    }

    #[test]
    fn test_remove_cache_file_allows_missing() {
        let missing = tempfile::TempDir::new()
            .unwrap()
            .path()
            .join("does-not-exist.bin");
        remove_cache_file(&missing).unwrap();
    }

    #[test]
    fn test_remove_cache_file_rejects_directory() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let error = remove_cache_file(temp_dir.path()).expect_err("directory is not a cache file");
        assert!(
            error
                .to_string()
                .contains("Failed to remove package cache file"),
            "unexpected error: {error:#}"
        );
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
    fn test_get_package_counts_consistent_on_real_systems() {
        if crate::core::paths::pacman_local_dir().exists() {
            let (total, explicit, deps) = match get_counts_fast() {
                Ok(counts) => counts,
                Err(e) => unreachable!("Failed to get counts: {e}"),
            };
            assert!(total > 0);
            // `deps` counts TRUE ORPHANS only, which are a subset of the
            // non-explicit packages — not a disjoint class that closes the
            // accounting equation (most dependencies have dependants).
            assert!(explicit <= total);
            assert!(explicit + deps <= total);
        }
    }

    /// Isolated fixture pinning the orphan-accounting rule end to end:
    /// exactly one of {explicit pkg, required dependency, unrequired
    /// dependency} is a true orphan under `is_orphan_package`.
    #[test]
    fn orphan_accounting_matches_is_orphan_package_on_synthetic_local_db() {
        let temp = tempfile::TempDir::new().unwrap();
        let write_desc = |name: &str, reason: &str, extra: &str| {
            let dir = temp.path().join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("desc"),
                format!("%NAME%\n{name}\n\n%VERSION%\n1.0-1\n\n%REASON%\n{reason}\n{extra}"),
            )
            .unwrap();
        };
        write_desc("explicit-pkg", "0", "");
        write_desc("required-dep", "1", "\n%REQUIREDBY%\nexplicit-pkg\n");
        write_desc("lonely-dep", "1", "");

        let packages = parse_local_db(temp.path()).unwrap();
        assert_eq!(packages.len(), 3);
        let orphans: Vec<&LocalDbPackage> = packages
            .values()
            .filter(|pkg| {
                crate::package_managers::types::is_orphan_package(
                    pkg.explicit,
                    pkg.required_by.is_empty(),
                    pkg.optional_for.is_empty(),
                )
            })
            .collect();
        assert_eq!(
            orphans.len(),
            1,
            "only lonely-dep is an orphan: {packages:?}"
        );
        assert_eq!(orphans[0].name, "lonely-dep");
    }

    #[test]
    fn test_is_cache_reusable_rejects_expired_entries() {
        let now = SystemTime::now();
        let expired = now - std::time::Duration::from_secs(CACHE_TTL_SECS + 5);

        assert!(!is_cache_reusable(Some(now), now, true, Some(expired)));
    }

    #[test]
    fn test_is_cache_reusable_rejects_empty_entries() {
        let now = SystemTime::now();
        let fresh = now - std::time::Duration::from_secs(1);

        assert!(!is_cache_reusable(Some(now), now, false, Some(fresh)));
    }

    #[test]
    fn package_cache_round_trips_through_atomic_persist() {
        let temp = tempfile::TempDir::new().unwrap();
        let cache = LocalDbCache {
            packages: HashMap::from([(
                "firefox".to_string(),
                LocalDbPackage {
                    name: "firefox".to_string(),
                    ..Default::default()
                },
            )]),
            last_modified: Some(SystemTime::UNIX_EPOCH),
            last_accessed: None,
        };

        save_cache_to_disk_in(&cache, temp.path(), "local_db").unwrap();
        let loaded: LocalDbCache = load_cache_from_disk_in(temp.path(), "local_db").unwrap();
        assert!(loaded.packages.contains_key("firefox"));
        assert_eq!(loaded.last_modified, Some(SystemTime::UNIX_EPOCH));
    }

    #[test]
    fn package_cache_persist_refuses_to_clobber_a_directory() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(temp.path().join("local_db.bin")).unwrap();

        let error = save_cache_to_disk_in(&LocalDbCache::default(), temp.path(), "local_db")
            .expect_err("must refuse to persist over a directory");
        assert!(!error.to_string().is_empty());
    }

    #[test]
    fn test_is_cache_reusable_accepts_fresh_matching_cache() {
        let now = SystemTime::now();
        let fresh = now - std::time::Duration::from_secs(2);

        assert!(is_cache_reusable(Some(now), now, true, Some(fresh)));
    }
}
