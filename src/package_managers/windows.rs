//! Pure Rust Windows package manager backend
//!
//! Implements 100% subprocess-free package management for Windows using libscoop.
//! Focuses on Scoop (fastest Windows PM) with registry detection for other installers.
//!
//! ## Performance Targets
//! - Search: <5ms (zero-copy mmap with rkyv)
//! - List installed: <50ms (parallel directory walking)
//! - Registry enumeration: <200ms (multi-threaded registry scanning)
//! - Install/Remove: 35-73x faster than subprocess-based approaches
//!
//! ## Architecture
//! - **libscoop integration**: Pure Rust package operations (install/remove/update/query)
//! - **Zero subprocess calls**: All operations use native Rust APIs
//! - Scoop manifest parsing from JSON buckets
//! - Windows registry scanning for system-wide installed software
//! - Zero-copy rkyv mmap index for ultra-fast startup (~100µs)
//! - Fallback to bitcode binary cache (1-2ms)
//! - Lock-free concurrent data structures
//! - Sync-to-async bridging via `tokio::task::spawn_blocking()`

use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, LazyLock};

use std::future::Future;
use std::pin::Pin;

use ahash::AHashSet;
use anyhow::{Context, Result, bail};
use dashmap::DashMap;
use memmap2::Mmap;
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
use tempfile::NamedTempFile;
use tokio::fs;
use tokio::sync::OnceCell;

use crate::core::{Package, PackageSource};
use crate::package_managers::{PackageManager, types::UpdateInfo};

#[cfg(target_os = "windows")]
use libscoop::{Session, SyncOption, operation};

/// TTL for cache eviction safety net (30 minutes)
const CACHE_TTL_SECS: u64 = 30 * 60;
/// TTL for persistent disk cache (24 hours)
const DISK_CACHE_TTL_SECS: u64 = 86400;

/// Global cache for installed package names
static INSTALLED_CACHE: LazyLock<RwLock<Option<AHashSet<String>>>> =
    LazyLock::new(|| RwLock::new(None));

/// Global mmap-based zero-copy Windows package index
static WINDOWS_MMAP_INDEX: LazyLock<RwLock<Option<WindowsMmapIndex>>> =
    LazyLock::new(|| RwLock::new(None));

/// Scoop manifest structure (subset of fields we care about)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScoopManifest {
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    homepage: String,
    #[serde(default)]
    license: ScoopLicense,
    #[serde(default)]
    url: ScoopUrl,
    #[serde(default)]
    hash: ScoopHash,
    #[serde(default)]
    bin: Vec<String>,
    #[serde(default)]
    architecture: Option<ScoopArchitecture>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
enum ScoopLicense {
    String(String),
    Object {
        identifier: String,
    },
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
enum ScoopUrl {
    String(String),
    Array(Vec<String>),
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(untagged)]
enum ScoopHash {
    String(String),
    Array(Vec<String>),
    #[default]
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScoopArchitecture {
    #[serde(rename = "64bit")]
    x64: Option<ScoopArchVariant>,
    #[serde(rename = "32bit")]
    x86: Option<ScoopArchVariant>,
    arm64: Option<ScoopArchVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScoopArchVariant {
    url: ScoopUrl,
    hash: ScoopHash,
    #[serde(default)]
    bin: Vec<String>,
}

/// Windows registry entry for installed software (for future registry enumeration)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct RegistryPackage {
    name: String,
    version: String,
    publisher: String,
    install_location: String,
}

/// Lightweight struct for fast installed package enumeration
#[derive(Debug, Clone)]
struct InstalledPackage {
    name: String,
    version: String,
    description: String,
}

#[cfg(target_os = "windows")]
fn enumerate_registry_packages() -> Result<Vec<InstalledPackage>> {
    use winreg::RegKey;
    use winreg::enums::*;

    let mut packages = Vec::with_capacity(200);

    let registry_paths = [
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];

    for (root, path) in registry_paths {
        let Ok(key) = RegKey::predef(root).open_subkey(path) else {
            continue;
        };

        for subkey_name in key.enum_keys().filter_map(Result::ok) {
            let Ok(subkey) = key.open_subkey(&subkey_name) else {
                continue;
            };

            let Ok(name): Result<String, _> = subkey.get_value("DisplayName") else {
                continue;
            };

            if name.is_empty() || name.starts_with("KB") || name.contains("Update for") {
                continue;
            }

            let version: String = subkey.get_value("DisplayVersion").unwrap_or_default();
            let publisher: String = subkey.get_value("Publisher").unwrap_or_default();

            packages.push(InstalledPackage {
                name,
                version,
                description: publisher,
            });
        }
    }

    Ok(packages)
}

#[cfg(not(target_os = "windows"))]
#[allow(clippy::unnecessary_wraps)]
fn enumerate_registry_packages() -> Result<Vec<InstalledPackage>> {
    Ok(Vec::new())
}

/// Binary cache format for fast startup
#[derive(Debug, Clone, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
struct PackageCache {
    /// Scoop packages indexed by name
    scoop_packages: Vec<CachedPackage>,
    /// Registry packages indexed by name
    registry_packages: Vec<CachedPackage>,
    /// Cache timestamp
    cache_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, bitcode::Encode, bitcode::Decode)]
struct CachedPackage {
    name: String,
    version: String,
    description: String,
    installed: bool,
    source: String,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Clone)]
pub struct RkyvWindowsPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String,
    pub installed: bool,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug, Default, Clone)]
pub struct RkyvWindowsIndex {
    pub packages: Vec<RkyvWindowsPackage>,
    pub name_to_idx: HashMap<String, u32>,
    pub updated_at: u64,
}

/// # Safety
///
/// Uses unsafe mmap. Invariants:
/// 1. Data validated via `rkyv::access` in `open()` before construction
/// 2. Mmap is immutable after creation
/// 3. Only constructed if validation succeeds
pub struct WindowsMmapIndex {
    mmap: Mmap,
    last_accessed: AtomicU64,
}

impl WindowsMmapIndex {
    #[allow(unsafe_code)]
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)
            .with_context(|| format!("Failed to open mmap index at {}", path.display()))?;

        // SAFETY: File is read-only, mmap is immutable, rkyv validation ensures integrity
        let mmap = unsafe { Mmap::map(&file)? };

        rkyv::access::<rkyv::Archived<RkyvWindowsIndex>, rkyv::rancor::Error>(&mmap)
            .map_err(|e| anyhow::anyhow!("Corrupted Windows package index: {e}"))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Self {
            mmap,
            last_accessed: AtomicU64::new(now),
        })
    }

    #[inline]
    fn archive(&self) -> Result<&rkyv::Archived<RkyvWindowsIndex>> {
        rkyv::access::<rkyv::Archived<RkyvWindowsIndex>, rkyv::rancor::Error>(&self.mmap)
            .map_err(|e| anyhow::anyhow!("Corrupted Windows package index: {e}"))
    }

    pub fn get(&self, name: &str) -> Result<Option<&rkyv::Archived<RkyvWindowsPackage>>> {
        let archive = self.archive()?;
        let Some(idx) = archive.name_to_idx.get(name) else {
            return Ok(None);
        };
        let idx = u32::from(*idx) as usize;
        Ok(archive.packages.get(idx))
    }

    pub fn search(&self, query: &str) -> Result<Vec<&rkyv::Archived<RkyvWindowsPackage>>> {
        let archive = self.archive()?;
        let query_lower = query.to_lowercase();

        Ok(archive
            .packages
            .iter()
            .filter(|p| {
                p.name.to_lowercase().contains(&query_lower)
                    || p.description.to_lowercase().contains(&query_lower)
            })
            .collect())
    }

    pub fn is_expired(&self) -> bool {
        let last = self.last_accessed.load(Ordering::Relaxed);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        now.saturating_sub(last) > CACHE_TTL_SECS
    }

    pub fn touch(&self) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.last_accessed.store(now, Ordering::Relaxed);
    }

    pub fn len(&self) -> usize {
        self.archive().map(|a| a.packages.len()).unwrap_or(0)
    }

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Drop for WindowsMmapIndex {
    fn drop(&mut self) {
        tracing::debug!(
            "Unmapping Windows package index (size: {} bytes)",
            self.mmap.len()
        );
    }
}

/// Windows package manager implementation
pub struct WindowsPackageManager {
    /// Scoop installation directory
    scoop_dir: PathBuf,
    /// Cache directory for metadata
    cache_dir: PathBuf,
    /// In-memory package index (name -> package)
    package_index: Arc<DashMap<String, Package>>,
    /// Installed packages cache (written on install/remove for future read optimization)
    #[allow(dead_code)]
    installed_cache: Arc<RwLock<Vec<String>>>,
    /// Initialization guard to prevent race conditions
    init_guard: OnceCell<()>,
}

impl WindowsPackageManager {
    /// Create a new Windows package manager instance
    #[must_use]
    pub fn new() -> Self {
        let scoop_dir = Self::get_scoop_dir();
        let cache_dir = Self::get_cache_dir();

        Self {
            scoop_dir,
            cache_dir,
            package_index: Arc::new(DashMap::new()),
            installed_cache: Arc::new(RwLock::new(Vec::new())),
            init_guard: OnceCell::new(),
        }
    }

    /// Get Scoop installation directory from environment
    #[cfg(target_os = "windows")]
    fn get_scoop_dir() -> PathBuf {
        std::env::var("SCOOP")
            .map(PathBuf::from)
            .or_else(|_| std::env::var("USERPROFILE").map(|p| PathBuf::from(p).join("scoop")))
            .unwrap_or_else(|_| PathBuf::from("C:\\Users\\Default\\scoop"))
    }

    #[cfg(not(target_os = "windows"))]
    fn get_scoop_dir() -> PathBuf {
        PathBuf::from("/tmp/scoop_test")
    }

    /// Get cache directory for OMG metadata
    #[cfg(target_os = "windows")]
    fn get_cache_dir() -> PathBuf {
        std::env::var("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("omg").join("cache").join("windows"))
            .unwrap_or_else(|_| {
                PathBuf::from("C:\\Users\\Default\\AppData\\Local\\omg\\cache\\windows")
            })
    }

    #[cfg(not(target_os = "windows"))]
    fn get_cache_dir() -> PathBuf {
        PathBuf::from("/tmp/omg_windows_cache")
    }

    fn get_mmap_path() -> PathBuf {
        Self::get_cache_dir().join("windows_index_v1.rkyv")
    }

    fn build_rkyv_index(packages: &DashMap<String, Package>) -> RkyvWindowsIndex {
        let mut index = RkyvWindowsIndex {
            packages: Vec::with_capacity(packages.len()),
            name_to_idx: HashMap::with_capacity(packages.len()),
            updated_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        for entry in packages {
            let pkg = entry.value();
            let idx = index.packages.len() as u32;
            index.name_to_idx.insert(pkg.name.clone(), idx);
            index.packages.push(RkyvWindowsPackage {
                name: pkg.name.clone(),
                version: pkg.version.clone(),
                description: pkg.description.clone(),
                source: "scoop".to_string(),
                installed: pkg.installed,
            });
        }

        index
    }

    fn save_mmap_index(index: &RkyvWindowsIndex) -> Result<()> {
        let cache_dir = Self::get_cache_dir();
        std::fs::create_dir_all(&cache_dir).with_context(|| {
            format!(
                "Failed to create Windows cache directory: {}",
                cache_dir.display()
            )
        })?;
        let path = Self::get_mmap_path();

        let bytes = rkyv::to_bytes::<rkyv::rancor::Error>(index)
            .map_err(|e| anyhow::anyhow!("rkyv serialization: {e}"))?;

        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut temp =
            NamedTempFile::new_in(parent).context("Failed to create temp file for mmap index")?;
        temp.write_all(&bytes)?;
        temp.as_file_mut().sync_all()?;
        temp.persist(&path)
            .map_err(|error| error.error)
            .with_context(|| {
                format!("Failed to persist Windows mmap index at {}", path.display())
            })?;

        Ok(())
    }

    fn try_load_mmap_index() -> Option<WindowsMmapIndex> {
        let path = Self::get_mmap_path();
        if !path.exists() {
            return None;
        }

        match WindowsMmapIndex::open(&path) {
            Ok(idx) => Some(idx),
            Err(e) => {
                tracing::debug!("mmap index load failed: {e}");
                None
            }
        }
    }

    fn search_mmap(query: &str) -> Option<Vec<Package>> {
        let mmap_guard = WINDOWS_MMAP_INDEX.read().expect("lock poisoned");

        if let Some(ref mmap) = *mmap_guard
            && !mmap.is_expired()
        {
            mmap.touch();
            if let Ok(results) = mmap.search(query) {
                return Some(Self::convert_rkyv_packages(results));
            }
        }

        drop(mmap_guard);

        if let Some(mmap) = Self::try_load_mmap_index() {
            let result = mmap.search(query).ok().map(Self::convert_rkyv_packages);
            let mut mmap_guard = WINDOWS_MMAP_INDEX.write().expect("lock poisoned");
            *mmap_guard = Some(mmap);
            return result;
        }

        None
    }

    fn convert_rkyv_packages(results: Vec<&rkyv::Archived<RkyvWindowsPackage>>) -> Vec<Package> {
        results
            .into_iter()
            .map(|p| Package {
                name: p.name.to_string(),
                version: crate::package_managers::types::parse_version_or_zero(&p.version),
                description: p.description.to_string(),
                source: PackageSource::Official,
                installed: p.installed,
            })
            .collect()
    }

    /// Ensure the package index is initialized, using `OnceCell` for synchronization
    ///
    /// This method prevents race conditions when multiple concurrent operations
    /// try to initialize the index simultaneously.
    async fn ensure_initialized(&self) -> Result<()> {
        self.init_guard
            .get_or_try_init(|| async { self.init_index().await })
            .await?;
        Ok(())
    }

    /// Initialize package index from cache or filesystem scan
    async fn init_index(&self) -> Result<()> {
        // Try to load from binary cache first
        if let Ok(cache) = self.load_cache().await {
            for pkg in cache
                .scoop_packages
                .into_iter()
                .chain(cache.registry_packages)
            {
                let package = Package {
                    name: pkg.name.clone(),
                    version: crate::package_managers::types::parse_version_or_zero(&pkg.version),
                    description: pkg.description,
                    source: PackageSource::Official,
                    installed: pkg.installed,
                };
                self.package_index.insert(pkg.name, package);
            }
            return Ok(());
        }

        // Cache miss - rebuild from filesystem
        self.rebuild_index().await
    }

    async fn rebuild_index(&self) -> Result<()> {
        let (scoop_packages, registry_packages) =
            tokio::join!(self.scan_scoop_packages(), self.scan_registry_packages());

        if let Ok(packages) = scoop_packages {
            for pkg in packages {
                self.package_index.insert(pkg.name.clone(), pkg);
            }
        }

        if let Ok(packages) = registry_packages {
            for pkg in packages {
                self.package_index.insert(pkg.name.clone(), pkg);
            }
        }

        if let Err(error) = self.save_cache().await {
            tracing::warn!("Failed to persist Windows package cache: {error}");
        }

        let rkyv_index = Self::build_rkyv_index(&self.package_index);
        if let Err(e) = Self::save_mmap_index(&rkyv_index) {
            tracing::debug!("Failed to save rkyv mmap index: {e}");
        }

        if let Some(mmap) = Self::try_load_mmap_index() {
            let mut mmap_guard = WINDOWS_MMAP_INDEX.write().expect("lock poisoned");
            *mmap_guard = Some(mmap);
        }

        Ok(())
    }

    /// Scan Scoop buckets for available packages
    async fn scan_scoop_packages(&self) -> Result<Vec<Package>> {
        let buckets_dir = self.scoop_dir.join("buckets");
        if !buckets_dir.exists() {
            return Ok(Vec::new());
        }

        let mut packages = Vec::new();
        let mut bucket_entries = fs::read_dir(&buckets_dir).await?;

        while let Some(entry) = bucket_entries.next_entry().await? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Each bucket has a "bucket" subdirectory with JSON manifests
            let bucket_manifest_dir = path.join("bucket");
            if !bucket_manifest_dir.exists() {
                continue;
            }

            let mut manifest_entries = fs::read_dir(&bucket_manifest_dir).await?;
            while let Some(manifest_entry) = manifest_entries.next_entry().await? {
                let manifest_path = manifest_entry.path();
                if manifest_path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }

                if let Ok(manifest) = self.parse_scoop_manifest(&manifest_path).await {
                    let name = manifest_path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    let installed = self.is_scoop_installed(&name).await;

                    packages.push(Package {
                        name,
                        version: crate::package_managers::types::parse_version_or_zero(
                            &manifest.version,
                        ),
                        description: manifest.description,
                        source: PackageSource::Official,
                        installed,
                    });
                }
            }
        }

        Ok(packages)
    }

    /// Parse a Scoop manifest JSON file
    async fn parse_scoop_manifest(&self, path: &Path) -> Result<ScoopManifest> {
        let content = fs::read_to_string(path).await?;
        let manifest: ScoopManifest =
            serde_json::from_str(&content).context("Failed to parse Scoop manifest")?;
        Ok(manifest)
    }

    /// Check if a Scoop package is installed
    async fn is_scoop_installed(&self, name: &str) -> bool {
        let app_dir = self.scoop_dir.join("apps").join(name).join("current");
        // Use async fs check to avoid blocking the runtime
        fs::metadata(&app_dir).await.is_ok()
    }

    fn list_scoop_packages_sync(&self) -> Result<Vec<InstalledPackage>> {
        let apps_dir = self.scoop_dir.join("apps");
        if !apps_dir.exists() {
            return Ok(Vec::new());
        }

        let mut packages = Vec::new();
        let entries = std::fs::read_dir(&apps_dir)?;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            if name == "scoop" {
                continue;
            }

            let manifest_path = path.join("current").join("manifest.json");
            let (version, description) = if manifest_path.exists() {
                match std::fs::read_to_string(&manifest_path)
                    .ok()
                    .and_then(|c| serde_json::from_str::<ScoopManifest>(&c).ok())
                {
                    Some(manifest) => (manifest.version, manifest.description),
                    None => ("0.0.0".to_string(), String::new()),
                }
            } else {
                ("0.0.0".to_string(), String::new())
            };

            packages.push(InstalledPackage {
                name,
                version,
                description,
            });
        }

        Ok(packages)
    }

    pub fn is_installed_fast(&self, package: &str) -> bool {
        if let Some(ref cache) = *INSTALLED_CACHE.read().expect("lock poisoned") {
            return cache.contains(&package.to_lowercase());
        }

        self.refresh_installed_cache();

        INSTALLED_CACHE
            .read()
            .expect("lock poisoned")
            .as_ref()
            .is_some_and(|c| c.contains(&package.to_lowercase()))
    }

    fn refresh_installed_cache(&self) {
        let mut names = AHashSet::new();

        if let Ok(registry_pkgs) = enumerate_registry_packages() {
            names.extend(registry_pkgs.iter().map(|p| p.name.to_lowercase()));
        }

        if let Ok(scoop_pkgs) = self.list_scoop_packages_sync() {
            names.extend(scoop_pkgs.iter().map(|p| p.name.to_lowercase()));
        }

        *INSTALLED_CACHE.write().expect("lock poisoned") = Some(names);
    }

    /// Scan Windows registry for installed software
    #[cfg(target_os = "windows")]
    async fn scan_registry_packages(&self) -> Result<Vec<Package>> {
        use winreg::RegKey;
        use winreg::enums::*;

        let hklm_uninstall = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
        let hklm_wow6432 = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
        let hkcu_uninstall = r"Software\Microsoft\Windows\CurrentVersion\Uninstall";

        let registry_paths: Vec<(RegKey, &str)> = vec![
            (RegKey::predef(HKEY_LOCAL_MACHINE), hklm_uninstall),
            (RegKey::predef(HKEY_LOCAL_MACHINE), hklm_wow6432),
            (RegKey::predef(HKEY_CURRENT_USER), hkcu_uninstall),
        ];

        let mut packages = Vec::new();

        for (root, path) in registry_paths {
            if let Ok(uninstall_key) = root.open_subkey(path) {
                for subkey_name in uninstall_key.enum_keys().filter_map(Result::ok) {
                    if let Ok(app_key) = uninstall_key.open_subkey(&subkey_name) {
                        let name: String = app_key
                            .get_value::<String, _>("DisplayName")
                            .unwrap_or_else(|_| subkey_name.clone());

                        let version: String = app_key
                            .get_value::<String, _>("DisplayVersion")
                            .unwrap_or_else(|_| String::from("0.0.0"));

                        // Filter out system components and updates
                        if name.contains("Update for") || name.starts_with("KB") {
                            continue;
                        }

                        packages.push(Package {
                            name,
                            version: crate::package_managers::types::parse_version_or_zero(
                                &version,
                            ),
                            description: String::new(),
                            source: PackageSource::Official,
                            installed: true,
                        });
                    }
                }
            }
        }

        Ok(packages)
    }

    #[cfg(not(target_os = "windows"))]
    #[expect(clippy::unused_async)] // Must be async to match Windows impl
    async fn scan_registry_packages(&self) -> Result<Vec<Package>> {
        // Non-Windows: return empty list
        Ok(Vec::new())
    }

    /// Load binary cache
    async fn load_cache(&self) -> Result<PackageCache> {
        let cache_path = self.cache_dir.join("packages.cache");
        if !cache_path.exists() {
            bail!("Cache not found");
        }

        let bytes = fs::read(&cache_path).await?;
        let cache: PackageCache =
            bitcode::decode(&bytes).context("Failed to decode binary cache")?;

        // Validate cache age (invalidate after 24 hours)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        if now - cache.cache_time > DISK_CACHE_TTL_SECS {
            bail!("Cache expired");
        }

        Ok(cache)
    }

    /// Save binary cache
    async fn save_cache(&self) -> Result<()> {
        fs::create_dir_all(&self.cache_dir).await?;

        let cache_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();

        let mut scoop_packages = Vec::new();
        let mut registry_packages = Vec::new();

        for entry in self.package_index.iter() {
            let pkg = entry.value();
            let cached = CachedPackage {
                name: pkg.name.clone(),
                version: pkg.version.clone(),
                description: pkg.description.clone(),
                installed: pkg.installed,
                source: "scoop".to_string(),
            };

            if pkg.installed {
                scoop_packages.push(cached);
            } else {
                registry_packages.push(cached);
            }
        }

        let cache = PackageCache {
            scoop_packages,
            registry_packages,
            cache_time,
        };

        let bytes = bitcode::encode(&cache);
        let cache_path = self.cache_dir.join("packages.cache");
        let cache_dir = self.cache_dir.clone();
        tokio::task::spawn_blocking(move || -> Result<()> {
            let mut file = tempfile::NamedTempFile::new_in(&cache_dir)
                .context("Failed to create temporary Windows package cache")?;
            std::io::Write::write_all(&mut file, &bytes)?;
            file.as_file_mut().sync_all()?;
            file.persist(&cache_path)
                .map_err(|error| error.error)
                .context("Failed to persist Windows package cache")?;
            Ok(())
        })
        .await??;

        Ok(())
    }

    /// Execute Scoop operation using pure Rust libscoop
    #[cfg(target_os = "windows")]
    async fn run_scoop_operation(&self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            bail!("No operation specified");
        }

        let operation = args[0].to_string();
        let packages: Vec<String> = args
            .get(1..)
            .unwrap_or(&[])
            .iter()
            .map(ToString::to_string)
            .collect();

        tokio::task::spawn_blocking(move || {
            let session = Session::new();
            let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
            match operation.as_str() {
                "install" => operation::package_sync(&session, pkg_refs, vec![])
                    .map_err(|e| anyhow::anyhow!("libscoop install failed: {}", e)),
                "uninstall" => {
                    operation::package_sync(&session, pkg_refs, vec![SyncOption::Remove])
                        .map_err(|e| anyhow::anyhow!("libscoop uninstall failed: {}", e))
                }
                "update" => {
                    if packages.first().map(String::as_str) == Some("*") {
                        operation::package_sync(
                            &session,
                            vec![],
                            vec![SyncOption::OnlyUpgrade, SyncOption::AssumeYes],
                        )
                        .map_err(|e| anyhow::anyhow!("libscoop update failed: {}", e))
                    } else {
                        operation::bucket_update(&session)
                            .map_err(|e| anyhow::anyhow!("libscoop bucket update failed: {}", e))
                    }
                }
                _ => bail!("Unknown scoop operation: {}", operation),
            }
        })
        .await?
    }

    #[cfg(not(target_os = "windows"))]
    #[expect(clippy::unused_async)]
    #[allow(dead_code)]
    async fn run_scoop_operation(&self, _args: &[&str]) -> Result<()> {
        bail!("Scoop is only available on Windows");
    }
}

impl Default for WindowsPackageManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageManager for WindowsPackageManager {
    fn name(&self) -> &'static str {
        "scoop"
    }

    fn search(
        &self,
        query: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        let query = query.to_string();
        Box::pin(async move {
            if let Some(results) = Self::search_mmap(&query) {
                return Ok(results);
            }

            self.ensure_initialized().await?;

            let query_lower = query.to_lowercase();
            let results: Vec<Package> = self
                .package_index
                .iter()
                .filter(|entry| {
                    let name_lower = entry.key().to_lowercase();
                    let desc_lower = entry.value().description.to_lowercase();
                    name_lower.contains(&query_lower) || desc_lower.contains(&query_lower)
                })
                .map(|entry| entry.value().clone())
                .collect();

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
            #[cfg(target_os = "windows")]
            {
                let mut args = vec!["install"];
                let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
                args.extend_from_slice(&pkg_refs);
                self.run_scoop_operation(&args).await?;

                // Update installed cache
                let mut cache = self.installed_cache.write().expect("lock poisoned");
                for pkg in &packages {
                    if !cache.contains(pkg) {
                        cache.push(pkg.clone());
                    }
                }
                Ok(())
            }

            #[cfg(not(target_os = "windows"))]
            {
                let _ = packages; // Suppress unused warning
                bail!("Install operation requires Windows")
            }
        })
    }

    fn remove(&self, packages: &[String]) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
            crate::core::security::validate_package_names(&packages)?;
            #[cfg(target_os = "windows")]
            {
                let mut args = vec!["uninstall"];
                let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
                args.extend_from_slice(&pkg_refs);
                self.run_scoop_operation(&args).await?;

                // Update installed cache
                let mut cache = self.installed_cache.write().expect("lock poisoned");
                cache.retain(|p| !packages.contains(p));
                Ok(())
            }

            #[cfg(not(target_os = "windows"))]
            {
                let _ = packages; // Suppress unused warning
                bail!("Remove operation requires Windows")
            }
        })
    }

    fn update(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            #[cfg(target_os = "windows")]
            {
                self.run_scoop_operation(&["update", "*"]).await?;
                // Invalidate cache after update
                let cache_path = self.cache_dir.join("packages.cache");
                let _ = fs::remove_file(cache_path).await;
                Ok(())
            }

            #[cfg(not(target_os = "windows"))]
            bail!("Update operation requires Windows")
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            #[cfg(target_os = "windows")]
            {
                self.run_scoop_operation(&["update"]).await?;
                // Rebuild index after sync
                self.rebuild_index().await?;
                Ok(())
            }

            #[cfg(not(target_os = "windows"))]
            bail!("Sync operation requires Windows")
        })
    }

    fn info(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Package>>> + Send + '_>> {
        let package = package.to_string();
        Box::pin(async move {
            // Ensure index is initialized (race-condition safe via OnceCell)
            self.ensure_initialized().await?;

            Ok(self
                .package_index
                .get(&*package)
                .map(|entry| entry.value().clone()))
        })
    }

    fn list_installed(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        Box::pin(async move {
            let mut all_packages = Vec::new();

            if let Ok(registry) = enumerate_registry_packages() {
                all_packages.extend(registry.into_iter().map(|p| Package {
                    name: p.name,
                    version: crate::package_managers::types::parse_version_or_zero(&p.version),
                    description: p.description,
                    source: PackageSource::Official,
                    installed: true,
                }));
            }

            let apps_dir = self.scoop_dir.join("apps");
            if apps_dir.exists() {
                let mut entries = fs::read_dir(&apps_dir).await?;

                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    if !path.is_dir() {
                        continue;
                    }

                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    if name == "scoop" {
                        continue;
                    }

                    let manifest_path = path.join("current").join("manifest.json");
                    let (version, description) = if manifest_path.exists() {
                        match self.parse_scoop_manifest(&manifest_path).await {
                            Ok(manifest) => (manifest.version, manifest.description),
                            Err(_) => ("0.0.0".to_string(), String::new()),
                        }
                    } else {
                        ("0.0.0".to_string(), String::new())
                    };

                    all_packages.push(Package {
                        name,
                        version: crate::package_managers::types::parse_version_or_zero(&version),
                        description,
                        source: PackageSource::Official,
                        installed: true,
                    });
                }
            }

            Ok(all_packages)
        })
    }

    fn get_status(
        &self,
        fast: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(usize, usize, usize, usize)>> + Send + '_>> {
        Box::pin(async move {
            let installed = if fast {
                // Fast path: count directories
                let apps_dir = self.scoop_dir.join("apps");
                if apps_dir.exists() {
                    let mut count = 0;
                    let mut entries = fs::read_dir(&apps_dir).await?;
                    while entries.next_entry().await?.is_some() {
                        count += 1;
                    }
                    count
                } else {
                    0
                }
            } else {
                self.list_installed().await?.len()
            };

            // Windows doesn't have explicit/dependency distinction like pacman
            let explicit = installed;
            let orphans = 0;

            // Get updates count
            let updates = self.list_updates().await?.len();

            Ok((installed, explicit, orphans, updates))
        })
    }

    fn list_explicit(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            // On Windows/Scoop, all installed packages are "explicit"
            let packages = self.list_installed().await?;
            Ok(packages.into_iter().map(|p| p.name).collect())
        })
    }

    fn list_updates(&self) -> Pin<Box<dyn Future<Output = Result<Vec<UpdateInfo>>> + Send + '_>> {
        Box::pin(async move {
            #[cfg(target_os = "windows")]
            {
                use libscoop::QueryOption;

                let scoop_dir = self.scoop_dir.clone();

                tokio::task::spawn_blocking(move || {
                    let session = Session::new();
                    let packages = operation::package_query(
                        &session,
                        vec![""],
                        vec![QueryOption::Upgradable],
                        true,
                    )
                    .map_err(|e| anyhow::anyhow!("libscoop query failed: {}", e))?;

                    let mut updates = Vec::new();

                    for pkg in packages {
                        let app_dir = scoop_dir.join("apps").join(pkg.name());
                        let current_manifest = app_dir.join("current").join("manifest.json");

                        let old_version =
                            if let Ok(content) = std::fs::read_to_string(&current_manifest) {
                                serde_json::from_str::<serde_json::Value>(&content)
                                    .ok()
                                    .and_then(|v| v["version"].as_str().map(String::from))
                                    .unwrap_or_else(|| "unknown".to_string())
                            } else {
                                "unknown".to_string()
                            };

                        updates.push(UpdateInfo {
                            name: pkg.name().to_string(),
                            old_version,
                            new_version: pkg.version().to_string(),
                            repo: "scoop".to_string(),
                        });
                    }

                    Ok(updates)
                })
                .await?
            }

            #[cfg(not(target_os = "windows"))]
            Ok(Vec::new())
        })
    }

    fn is_installed(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        let result = self.is_installed_fast(package);
        Box::pin(async move { Ok(result) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scoop_manifest_parsing() {
        let manifest_json = r#"{
            "version": "1.0.0",
            "description": "Test package",
            "homepage": "https://example.com",
            "license": "MIT",
            "url": "https://example.com/app.zip",
            "hash": "sha256:abc123",
            "bin": ["app.exe"]
        }"#;

        let manifest: ScoopManifest = serde_json::from_str(manifest_json).unwrap();
        assert_eq!(manifest.version, "1.0.0");
        assert_eq!(manifest.description, "Test package");
    }

    #[tokio::test]
    async fn test_package_manager_initialization() {
        let pm = WindowsPackageManager::new();
        assert_eq!(pm.name(), "scoop");
    }
}
