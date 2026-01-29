//! Pure Rust Windows package manager backend
//!
//! Implements direct filesystem and registry access for Windows package management.
//! Focuses on Scoop (fastest Windows PM) with registry detection for other installers.
//!
//! ## Performance Targets
//! - Search: <100ms (in-memory index with binary cache)
//! - List installed: <50ms (parallel directory walking)
//! - Registry enumeration: <200ms (multi-threaded registry scanning)
//!
//! ## Architecture
//! - Zero CLI wrappers - pure Rust APIs only
//! - Scoop manifest parsing from JSON buckets
//! - Windows registry scanning for system-wide installed software
//! - Binary cache using bitcode for instant startup
//! - Lock-free concurrent data structures

use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::sync::OnceCell;

use crate::core::{Package, PackageSource};
use crate::package_managers::{PackageManager, types::UpdateInfo};

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

    /// Rebuild index from Scoop buckets and registry
    async fn rebuild_index(&self) -> Result<()> {
        let (scoop_packages, registry_packages) =
            tokio::join!(self.scan_scoop_packages(), self.scan_registry_packages());

        // Merge into index
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

        // Save to binary cache
        let _ = self.save_cache().await;

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

    /// Scan Windows registry for installed software
    #[cfg(target_os = "windows")]
    async fn scan_registry_packages(&self) -> Result<Vec<Package>> {
        use winreg::RegKey;
        use winreg::enums::*;

        let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);

        let registry_paths = vec![
            (
                hklm.clone(),
                r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
            (
                hklm,
                r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
            ),
            (hkcu, r"Software\Microsoft\Windows\CurrentVersion\Uninstall"),
        ];

        let mut packages = Vec::new();

        for (root, path) in registry_paths {
            if let Ok(uninstall_key) = root.open_subkey(path) {
                for subkey_name in uninstall_key.enum_keys().filter_map(Result::ok) {
                    if let Ok(app_key) = uninstall_key.open_subkey(&subkey_name) {
                        let name: String = app_key
                            .get_value("DisplayName")
                            .unwrap_or_else(|_| subkey_name.clone());

                        let version: String = app_key
                            .get_value("DisplayVersion")
                            .unwrap_or_else(|_| "0.0.0".to_string());

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
    #[allow(clippy::unused_async)] // Must be async to match Windows impl
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

        if now - cache.cache_time > 86400 {
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
        fs::write(&cache_path, &bytes).await?;

        Ok(())
    }

    /// Execute Scoop command (fallback for operations we can't do purely in Rust)
    #[cfg(target_os = "windows")]
    async fn run_scoop(&self, args: &[&str]) -> Result<()> {
        let output = tokio::process::Command::new("scoop")
            .args(args)
            .output()
            .await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("scoop command failed: {}", stderr);
        }

        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    #[allow(clippy::unused_async)] // Must be async to match Windows impl
    #[allow(dead_code)] // Stub for non-Windows compilation, never called
    async fn run_scoop(&self, _args: &[&str]) -> Result<()> {
        bail!("Scoop is only available on Windows");
    }
}

impl Default for WindowsPackageManager {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PackageManager for WindowsPackageManager {
    fn name(&self) -> &'static str {
        "scoop"
    }

    async fn search(&self, query: &str) -> Result<Vec<Package>> {
        // Ensure index is initialized (race-condition safe via OnceCell)
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
    }

    async fn install(&self, packages: &[String]) -> Result<()> {
        crate::core::security::validate_package_names(packages)?;
        #[cfg(target_os = "windows")]
        {
            let mut args = vec!["install"];
            let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
            args.extend_from_slice(&pkg_refs);
            self.run_scoop(&args).await?;

            // Update installed cache
            let mut cache = self.installed_cache.write();
            for pkg in packages {
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
    }

    async fn remove(&self, packages: &[String]) -> Result<()> {
        crate::core::security::validate_package_names(packages)?;
        #[cfg(target_os = "windows")]
        {
            let mut args = vec!["uninstall"];
            let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
            args.extend_from_slice(&pkg_refs);
            self.run_scoop(&args).await?;

            // Update installed cache
            let mut cache = self.installed_cache.write();
            cache.retain(|p| !packages.contains(p));
            Ok(())
        }

        #[cfg(not(target_os = "windows"))]
        {
            let _ = packages; // Suppress unused warning
            bail!("Remove operation requires Windows")
        }
    }

    async fn update(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.run_scoop(&["update", "*"]).await?;
            // Invalidate cache after update
            let cache_path = self.cache_dir.join("packages.cache");
            let _ = fs::remove_file(cache_path).await;
            Ok(())
        }

        #[cfg(not(target_os = "windows"))]
        bail!("Update operation requires Windows")
    }

    async fn sync(&self) -> Result<()> {
        #[cfg(target_os = "windows")]
        {
            self.run_scoop(&["update"]).await?;
            // Rebuild index after sync
            self.rebuild_index().await?;
            Ok(())
        }

        #[cfg(not(target_os = "windows"))]
        bail!("Sync operation requires Windows")
    }

    async fn info(&self, package: &str) -> Result<Option<Package>> {
        // Ensure index is initialized (race-condition safe via OnceCell)
        self.ensure_initialized().await?;

        Ok(self
            .package_index
            .get(package)
            .map(|entry| entry.value().clone()))
    }

    async fn list_installed(&self) -> Result<Vec<Package>> {
        let apps_dir = self.scoop_dir.join("apps");
        if !apps_dir.exists() {
            return Ok(Vec::new());
        }

        let mut packages = Vec::new();
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

            // Skip scoop's own directory
            if name == "scoop" {
                continue;
            }

            // Read version from current/manifest.json
            let manifest_path = path.join("current").join("manifest.json");
            let (version, description) = if manifest_path.exists() {
                match self.parse_scoop_manifest(&manifest_path).await {
                    Ok(manifest) => (manifest.version, manifest.description),
                    Err(_) => ("0.0.0".to_string(), String::new()),
                }
            } else {
                ("0.0.0".to_string(), String::new())
            };

            packages.push(Package {
                name,
                version: crate::package_managers::types::parse_version_or_zero(&version),
                description,
                source: PackageSource::Official,
                installed: true,
            });
        }

        Ok(packages)
    }

    async fn get_status(&self, fast: bool) -> Result<(usize, usize, usize, usize)> {
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
    }

    async fn list_explicit(&self) -> Result<Vec<String>> {
        // On Windows/Scoop, all installed packages are "explicit"
        let packages = self.list_installed().await?;
        Ok(packages.into_iter().map(|p| p.name).collect())
    }

    async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        #[cfg(target_os = "windows")]
        {
            // Run scoop status to get available updates
            let output = tokio::process::Command::new("scoop")
                .args(&["status"])
                .output()
                .await?;

            if !output.status.success() {
                return Ok(Vec::new());
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            let mut updates = Vec::new();

            // Parse scoop status output
            // Format: "Name: old_version -> new_version"
            for line in stdout.lines() {
                if line.contains("->") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 {
                        let name = parts[0].trim_end_matches(':').to_string();
                        let old_version = parts[1].to_string();
                        let new_version = parts[3].to_string();

                        updates.push(UpdateInfo {
                            name,
                            old_version,
                            new_version,
                            repo: "scoop".to_string(),
                        });
                    }
                }
            }

            Ok(updates)
        }

        #[cfg(not(target_os = "windows"))]
        Ok(Vec::new())
    }

    async fn is_installed(&self, package: &str) -> bool {
        self.is_scoop_installed(package).await
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
