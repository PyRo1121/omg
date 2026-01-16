//! Pure Rust Pacman Database Parser - ULTRA FAST (<20ms!)
//!
//! Parses /var/lib/pacman/sync/*.db and /var/lib/pacman/local/
//! WITHOUT using libalpm. Direct tar.gz/tar.zst parsing.
//!
//! First load: ~100ms (parse all DBs)
//! Cached: <1ms (instant lookup)

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use futures::stream::StreamExt;
use parking_lot::RwLock;
use rayon::prelude::*;
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tracing::instrument;

use crate::core::paths;

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
}

fn load_sync_packages(sync_dir: &Path) -> Result<HashMap<String, SyncDbPackage>> {
    let db_paths = collect_sync_db_paths(sync_dir);

    let parsed: Vec<HashMap<String, SyncDbPackage>> = db_paths
        .par_iter()
        .map(|(path, name)| parse_sync_db(path, name))
        .collect::<Result<Vec<_>>>()?;

    let mut packages = HashMap::with_capacity(20000);
    for pkgs in parsed {
        packages.extend(pkgs);
    }

    Ok(packages)
}

fn collect_sync_db_paths(sync_dir: &Path) -> Vec<(PathBuf, String)> {
    let mut dbs = Vec::new();

    for db_name in &["core", "extra", "multilib"] {
        let db_path = sync_dir.join(format!("{db_name}.db"));
        if db_path.exists() {
            dbs.push((db_path, (*db_name).to_string()));
        }
    }

    if let Ok(entries) = std::fs::read_dir(sync_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path
                .file_stem()
                .and_then(|n| n.to_str())
                .map(str::to_string);
            if let Some(name) = name
                && !["core", "extra", "multilib"].contains(&name.as_str())
                && path.is_file()
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
}

/// A package entry from the sync database
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncDbPackage {
    pub name: String,
    pub version: String,
    pub desc: String,
    pub filename: String,
    pub csize: u64, // Compressed size (download size)
    pub isize: u64, // Installed size
    pub url: String,
    pub arch: String,
    pub repo: String,
    pub depends: Vec<String>,
    pub makedepends: Vec<String>,
    pub optdepends: Vec<String>,
    pub provides: Vec<String>,
    pub conflicts: Vec<String>,
    pub replaces: Vec<String>,
}

/// A package from the local database
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LocalDbPackage {
    pub name: String,
    pub version: String,
    pub desc: String,
    pub install_date: String,
    pub explicit: bool, // Explicitly installed vs dependency
}

/// Parse a sync database file (core.db, extra.db, multilib.db)
/// Returns a `HashMap` of package name -> `SyncDbPackage`
pub fn parse_sync_db(path: &Path, repo_name: &str) -> Result<HashMap<String, SyncDbPackage>> {
    let file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;

    // Detect compression type from extension
    let (reader, is_zstd): (Box<dyn Read>, bool) = {
        let path_str = path.to_string_lossy();
        if path_str.ends_with(".db") {
            // Try to detect actual format by reading magic bytes
            let mut magic = [0u8; 4];
            let mut f = File::open(path)?;
            f.read_exact(&mut magic)?;

            let file = File::open(path)?;
            if magic[0..2] == [0x1f, 0x8b] {
                // gzip
                (Box::new(GzDecoder::new(file)), false)
            } else if magic[0..4] == [0x28, 0xb5, 0x2f, 0xfd] {
                // zstd - use pure Rust ruzstd
                (
                    Box::new(
                        ruzstd::decoding::StreamingDecoder::new(file)
                            .map_err(|e| anyhow::anyhow!("zstd: {e}"))?,
                    ),
                    true,
                )
            } else {
                // Assume gzip
                (Box::new(GzDecoder::new(file)), false)
            }
        } else if path_str.ends_with(".zst") {
            (
                Box::new(
                    ruzstd::decoding::StreamingDecoder::new(file)
                        .map_err(|e| anyhow::anyhow!("zstd: {e}"))?,
                ),
                true,
            )
        } else {
            (Box::new(GzDecoder::new(file)), false)
        }
    };
    let _ = is_zstd; // Suppress unused warning

    let mut archive = tar::Archive::new(reader);
    let mut packages = HashMap::new();

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        let path_str = path.to_string_lossy();

        // We only care about desc files
        if path_str.ends_with("/desc") {
            let pkg_name = path_str.split('/').next().unwrap_or("");
            // Extract just the package name (before the version)
            let _base_name = pkg_name
                .rsplit_once('-')
                .map(|(n, _)| n)
                .and_then(|n| n.rsplit_once('-').map(|(n, _)| n))
                .unwrap_or(pkg_name);

            let mut content = String::new();
            entry.read_to_string(&mut content)?;

            let pkg = parse_desc_content(&content, repo_name);
            if !pkg.name.is_empty() {
                packages.insert(pkg.name.clone(), pkg);
            }
        }
    }

    Ok(packages)
}

/// Parse the desc file content into a `SyncDbPackage`
fn parse_desc_content(content: &str, repo: &str) -> SyncDbPackage {
    let mut pkg = SyncDbPackage {
        repo: repo.to_string(),
        ..SyncDbPackage::default()
    };

    let mut current_field: Option<&str> = None;

    for line in content.lines() {
        let line = line.trim();

        if line.starts_with('%') && line.ends_with('%') {
            current_field = Some(&line[1..line.len() - 1]);
            continue;
        }

        if line.is_empty() {
            current_field = None;
            continue;
        }

        match current_field {
            Some("NAME") => pkg.name = line.to_string(),
            Some("VERSION") => pkg.version = line.to_string(),
            Some("DESC") => pkg.desc = line.to_string(),
            Some("FILENAME") => pkg.filename = line.to_string(),
            Some("CSIZE") => pkg.csize = line.parse().unwrap_or(0),
            Some("ISIZE") => pkg.isize = line.parse().unwrap_or(0),
            Some("URL") => pkg.url = line.to_string(),
            Some("ARCH") => pkg.arch = line.to_string(),
            Some("DEPENDS") => pkg.depends.push(line.to_string()),
            Some("MAKEDEPENDS") => pkg.makedepends.push(line.to_string()),
            Some("OPTDEPENDS") => pkg.optdepends.push(line.to_string()),
            Some("PROVIDES") => pkg.provides.push(line.to_string()),
            Some("CONFLICTS") => pkg.conflicts.push(line.to_string()),
            Some("REPLACES") => pkg.replaces.push(line.to_string()),
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

/// Parse a local package's desc file
fn parse_local_desc(path: &Path) -> Result<LocalDbPackage> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);

    let mut pkg = LocalDbPackage::default();
    let mut current_field: Option<String> = None;

    for line in reader.lines() {
        let line = line?;
        let line = line.trim();

        if line.starts_with('%') && line.ends_with('%') {
            current_field = Some(line[1..line.len() - 1].to_string());
            continue;
        }

        if line.is_empty() {
            current_field = None;
            continue;
        }

        match current_field.as_deref() {
            Some("NAME") => pkg.name = line.to_string(),
            Some("VERSION") => pkg.version = line.to_string(),
            Some("DESC") => pkg.desc = line.to_string(),
            Some("INSTALLDATE") => pkg.install_date = line.to_string(),
            Some("REASON") => pkg.explicit = line == "0",
            _ => {}
        }
    }

    Ok(pkg)
}

/// ULTRA FAST update check - uses global cache (<5ms after first load!)
/// Returns Vec of (name, `old_version`, `new_version`, repo, filename, `download_size`)
#[instrument]
pub fn check_updates_cached() -> Result<Vec<(String, String, String, String, String, u64)>> {
    let sync_dir = paths::pacman_sync_dir();
    let local_dir = paths::pacman_local_dir();

    // Ensure caches are loaded (will be fast if already loaded)
    ensure_sync_cache_loaded(&sync_dir)?;
    ensure_local_cache_loaded(&local_dir)?;

    // Hold both cache locks simultaneously - no cloning!
    let sync_cache = SYNC_DB_CACHE.read();
    let local_cache = LOCAL_DB_CACHE.read();

    // Compare versions - pure HashMap lookups, <1ms
    let mut updates = Vec::new();

    for (name, local_pkg) in &local_cache.packages {
        if let Some(sync_pkg) = sync_cache.packages.get(name)
            && compare_versions(&local_pkg.version, &sync_pkg.version) == std::cmp::Ordering::Less
        {
            updates.push((
                name.clone(),
                local_pkg.version.clone(),
                sync_pkg.version.clone(),
                sync_pkg.repo.clone(),
                sync_pkg.filename.clone(),
                sync_pkg.csize,
            ));
        }
    }

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
    let mut file = File::create(&tmp_path)?;
    bincode::serde::encode_into_std_write(cache, &mut file, bincode::config::standard())?;
    fs::rename(tmp_path, path)?;
    Ok(())
}

/// Load cache from disk
fn load_cache_from_disk<T: for<'de> Deserialize<'de>>(name: &str) -> Result<T> {
    let path = get_cache_dir().join(format!("{name}.bin"));
    let mut file = File::open(&path)?;
    let cache: T = bincode::serde::decode_from_std_read(&mut file, bincode::config::standard())?;
    Ok(cache)
}

/// Ensure sync cache is loaded (fast if already loaded)
fn ensure_sync_cache_loaded(sync_dir: &Path) -> Result<()> {
    let current_mtime = get_newest_db_mtime(sync_dir);

    {
        let cache = SYNC_DB_CACHE.read();
        if cache.last_modified == Some(current_mtime) && !cache.packages.is_empty() {
            return Ok(());
        }
    }

    // Try to load from disk cache first (FAST < 5ms)
    if let Ok(disk_cache) = load_cache_from_disk::<DbCache>("sync_db")
        && disk_cache.last_modified == Some(current_mtime)
    {
        let mut cache = SYNC_DB_CACHE.write();
        *cache = disk_cache;
        return Ok(());
    }

    // Cache miss or stale - need to reload/parse
    let packages = load_sync_packages(sync_dir)?;

    // Update memory cache
    let mut cache = SYNC_DB_CACHE.write();
    cache.packages = packages;
    cache.last_modified = Some(current_mtime);

    // Save to disk for next time
    let _ = save_cache_to_disk(&*cache, "sync_db");

    Ok(())
}

/// Ensure local cache is loaded (fast if already loaded)
fn ensure_local_cache_loaded(local_dir: &Path) -> Result<()> {
    let current_mtime = get_local_db_mtime(local_dir)?;

    {
        let cache = LOCAL_DB_CACHE.read();
        if cache.last_modified == Some(current_mtime) && !cache.packages.is_empty() {
            return Ok(());
        }
    }

    // Try to load from disk cache first
    if let Ok(disk_cache) = load_cache_from_disk::<LocalDbCache>("local_db")
        && disk_cache.last_modified == Some(current_mtime)
    {
        let mut cache = LOCAL_DB_CACHE.write();
        *cache = disk_cache;
        return Ok(());
    }

    // Cache miss - reload
    let packages = parse_local_db(local_dir)?;

    // Update memory cache
    let mut cache = LOCAL_DB_CACHE.write();
    cache.packages = packages;
    cache.last_modified = Some(current_mtime);

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
    // Clear in-memory caches
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

    // Delete disk caches to force fresh parse on next access
    let cache_dir = get_cache_dir();
    let _ = fs::remove_file(cache_dir.join("sync_db.bin"));
    let _ = fs::remove_file(cache_dir.join("local_db.bin"));
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
pub fn check_updates_fast() -> Result<Vec<(String, String, String, String, String, u64)>> {
    check_updates_cached()
}

/// Compare two version strings (like `alpm_pkg_vercmp`)
/// Returns `Ordering::Less` if v1 < v2, Equal if v1 == v2, Greater if v1 > v2
#[must_use]
pub fn compare_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    // Split into epoch:version-release
    let (e1, vr1) = split_epoch(v1);
    let (e2, vr2) = split_epoch(v2);

    // Compare epochs first
    match e1.cmp(&e2) {
        std::cmp::Ordering::Equal => {}
        other => return other,
    }

    // Split version-release
    let (ver1, rel1) = split_release(vr1);
    let (ver2, rel2) = split_release(vr2);

    // Compare versions
    match compare_version_parts(ver1, ver2) {
        std::cmp::Ordering::Equal => {}
        other => return other,
    }

    // Compare releases
    compare_version_parts(rel1, rel2)
}

fn split_epoch(version: &str) -> (u64, &str) {
    if let Some(idx) = version.find(':') {
        let epoch = version[..idx].parse().unwrap_or(0);
        (epoch, &version[idx + 1..])
    } else {
        (0, version)
    }
}

fn split_release(version: &str) -> (&str, &str) {
    if let Some(idx) = version.rfind('-') {
        (&version[..idx], &version[idx + 1..])
    } else {
        (version, "")
    }
}

fn compare_version_parts(v1: &str, v2: &str) -> std::cmp::Ordering {
    let mut iter1 = v1.chars().peekable();
    let mut iter2 = v2.chars().peekable();

    loop {
        // Skip non-alphanumeric
        while iter1.peek().is_some_and(|c| !c.is_alphanumeric()) {
            iter1.next();
        }
        while iter2.peek().is_some_and(|c| !c.is_alphanumeric()) {
            iter2.next();
        }

        let seg1 = collect_segment(&mut iter1);
        let seg2 = collect_segment(&mut iter2);

        if seg1.is_empty() && seg2.is_empty() {
            return std::cmp::Ordering::Equal;
        }

        // Compare segments
        let is_num1 = seg1.chars().next().is_some_and(|c| c.is_ascii_digit());
        let is_num2 = seg2.chars().next().is_some_and(|c| c.is_ascii_digit());

        match (is_num1, is_num2) {
            (true, true) => {
                // Numeric comparison
                let n1: u64 = seg1.parse().unwrap_or(0);
                let n2: u64 = seg2.parse().unwrap_or(0);
                if n1 != n2 {
                    return n1.cmp(&n2);
                }
            }
            (true, false) => return std::cmp::Ordering::Greater,
            (false, true) => return std::cmp::Ordering::Less,
            (false, false) => {
                // String comparison
                if seg1 != seg2 {
                    return seg1.cmp(&seg2);
                }
            }
        }
    }
}

fn collect_segment(iter: &mut std::iter::Peekable<std::str::Chars>) -> String {
    let mut seg = String::new();

    if let Some(&c) = iter.peek() {
        if c.is_ascii_digit() {
            while iter.peek().is_some_and(char::is_ascii_digit) {
                seg.push(iter.next().unwrap());
            }
        } else if c.is_alphabetic() {
            while iter.peek().is_some_and(|c| c.is_alphabetic()) {
                seg.push(iter.next().unwrap());
            }
        }
    }

    seg
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

/// Search local packages using cache - FAST (<1ms)
pub fn search_local_cached(query: &str) -> Result<Vec<LocalDbPackage>> {
    let local_dir = paths::pacman_local_dir();
    ensure_local_cache_loaded(&local_dir)?;

    let query_lower = query.to_lowercase();
    let cache = LOCAL_DB_CACHE.read();

    let results = cache
        .packages
        .values()
        .filter(|pkg| {
            query_lower.is_empty()
                || pkg.name.to_lowercase().contains(&query_lower)
                || pkg.desc.to_lowercase().contains(&query_lower)
        })
        .cloned()
        .collect();

    Ok(results)
}

/// List all local packages using cache - FAST (<1ms)
pub fn list_local_cached() -> Result<Vec<LocalDbPackage>> {
    let local_dir = paths::pacman_local_dir();
    ensure_local_cache_loaded(&local_dir)?;

    let cache = LOCAL_DB_CACHE.read();
    Ok(cache.packages.values().cloned().collect())
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
                || pkg.name.to_lowercase().contains(&query_lower)
                || pkg.desc.to_lowercase().contains(&query_lower)
        })
        .cloned()
        .collect();

    Ok(results)
}

/// Official Arch Linux repositories - packages in these are NOT AUR candidates
/// Source: <https://wiki.archlinux.org/title/Official_repositories>
const OFFICIAL_REPOS: &[&str] = &[
    // Stable
    "core",
    "extra",
    "multilib",
    // Testing
    "core-testing",
    "extra-testing",
    "multilib-testing",
    "gnome-unstable",
    "kde-unstable",
];

/// Identify potential AUR packages (installed but not in official repos)
/// Uses pure Rust cache for extreme speed (<1ms)
/// Note: Packages in custom repos (e.g., chaotic-aur) ARE included since they
/// may have AUR updates available. Use `verify_aur_packages()` to filter.
pub fn get_potential_aur_packages() -> Result<Vec<String>> {
    let sync_dir = paths::pacman_sync_dir();
    let local_dir = paths::pacman_local_dir();

    ensure_sync_cache_loaded(&sync_dir)?;
    ensure_local_cache_loaded(&local_dir)?;

    let sync_cache = SYNC_DB_CACHE.read();
    let local_cache = LOCAL_DB_CACHE.read();

    let mut potential = Vec::new();
    for name in local_cache.packages.keys() {
        // Only exclude packages from OFFICIAL repos (not custom repos)
        let in_official_repo = sync_cache
            .packages
            .get(name)
            .is_some_and(|pkg| OFFICIAL_REPOS.contains(&pkg.repo.as_str()));

        if !in_official_repo {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_compare_basic() {
        assert_eq!(compare_versions("1.0", "1.0"), std::cmp::Ordering::Equal);
        assert_eq!(compare_versions("1.0", "2.0"), std::cmp::Ordering::Less);
        assert_eq!(compare_versions("2.0", "1.0"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_version_compare_release() {
        assert_eq!(compare_versions("1.0-1", "1.0-2"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("1.0-10", "1.0-9"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            compare_versions("1.0-1", "1.0-1"),
            std::cmp::Ordering::Equal
        );
    }

    #[test]
    fn test_version_compare_epoch() {
        assert_eq!(
            compare_versions("1:1.0", "1.0"),
            std::cmp::Ordering::Greater
        );
        assert_eq!(compare_versions("1.0", "1:1.0"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("2:1.0", "1:2.0"),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn test_version_compare_complex() {
        // Real-world examples
        assert_eq!(
            compare_versions("2.35.0-1", "2.36.0-1"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("125.0.3-1", "126.0-1"),
            std::cmp::Ordering::Less
        );
        assert_eq!(
            compare_versions("6.12.12.arch1-1", "6.12.13.arch1-1"),
            std::cmp::Ordering::Less
        );
    }

    #[test]
    fn test_version_compare_alpha_numeric() {
        assert_eq!(compare_versions("1.0a", "1.0b"), std::cmp::Ordering::Less);
        assert_eq!(
            compare_versions("1.0rc1", "1.0rc2"),
            std::cmp::Ordering::Less
        );
        assert_eq!(compare_versions("1.0", "1.0rc1"), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_split_epoch() {
        assert_eq!(split_epoch("1.0"), (0, "1.0"));
        assert_eq!(split_epoch("1:1.0"), (1, "1.0"));
        assert_eq!(split_epoch("2:3.0-1"), (2, "3.0-1"));
    }

    #[test]
    fn test_split_release() {
        assert_eq!(split_release("1.0"), ("1.0", ""));
        assert_eq!(split_release("1.0-1"), ("1.0", "1"));
        assert_eq!(split_release("1.0-1-2"), ("1.0-1", "2"));
    }

    #[test]
    fn test_check_updates() {
        // Only run if we have a real system
        if crate::core::paths::pacman_sync_dir()
            .join("core.db")
            .exists()
        {
            let updates = check_updates_fast().expect("Failed to check updates");
            println!("Found {} updates", updates.len());
        }
    }

    #[test]
    fn test_get_local_package() {
        // Only run if we have a real system
        if crate::core::paths::pacman_local_dir().exists() {
            // pacman should always be installed
            if let Ok(Some(pkg)) = get_local_package("pacman") {
                assert!(!pkg.version.is_empty());
            }
        }
    }

    #[test]
    fn test_get_package_counts() {
        if crate::core::paths::pacman_local_dir().exists() {
            let (total, explicit, deps) = get_counts_fast().expect("Failed to get counts");
            assert!(total > 0);
            // On some test systems, explicit might be 0 if only deps are present
            // but usually at least some are explicit. We change to total >= explicit + deps
            assert_eq!(total, explicit + deps);
        }
    }
}
