//! Mock implementations for isolated testing

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use futures::future::{BoxFuture, FutureExt};

/// Mock package database for testing without real system access
#[derive(Default, Clone)]
pub struct MockPackageDb {
    pub packages: Arc<Mutex<HashMap<String, MockPackage>>>,
    pub installed: Arc<Mutex<Vec<String>>>,
    pub updates: Arc<Mutex<Vec<omg_lib::package_managers::types::UpdateInfo>>>,
}

#[derive(Clone, Debug)]
pub struct MockPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub repo: String,
    pub dependencies: Vec<String>,
    pub installed_size: u64,
}

impl MockPackageDb {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_packages(packages: Vec<MockPackage>) -> Self {
        let db = Self::new();
        for pkg in packages {
            db.add_package(pkg);
        }
        db
    }

    pub fn add_package(&self, pkg: MockPackage) {
        self.packages.lock().unwrap().insert(pkg.name.clone(), pkg);
    }

    pub fn add_update(&self, update: omg_lib::package_managers::types::UpdateInfo) {
        self.updates.lock().unwrap().push(update);
    }

    pub fn install(&self, name: &str) -> Result<(), String> {
        if self.packages.lock().unwrap().contains_key(name) {
            self.installed.lock().unwrap().push(name.to_string());
            Ok(())
        } else {
            Err(format!("Package {name} not found"))
        }
    }

    pub fn remove(&self, name: &str) -> Result<(), String> {
        let mut installed = self.installed.lock().unwrap();
        if let Some(pos) = installed.iter().position(|n| n == name) {
            installed.remove(pos);
            Ok(())
        } else {
            Err(format!("Package {name} not installed"))
        }
    }

    pub fn is_installed(&self, name: &str) -> bool {
        self.installed.lock().unwrap().contains(&name.to_string())
    }

    pub fn search(&self, query: &str) -> Vec<MockPackage> {
        self.packages
            .lock()
            .unwrap()
            .values()
            .filter(|p| p.name.contains(query) || p.description.contains(query))
            .cloned()
            .collect()
    }

    pub fn get(&self, name: &str) -> Option<MockPackage> {
        self.packages.lock().unwrap().get(name).cloned()
    }

    /// Create a mock database with common Arch packages
    pub fn arch_mock() -> Self {
        Self::with_packages(vec![
            MockPackage {
                name: "pacman".to_string(),
                version: "6.0.2".to_string(),
                description: "A library-based package manager".to_string(),
                repo: "core".to_string(),
                dependencies: vec!["glibc".to_string(), "bash".to_string()],
                installed_size: 1024 * 1024,
            },
            MockPackage {
                name: "firefox".to_string(),
                version: "122.0".to_string(),
                description: "Fast, Private & Safe Web Browser".to_string(),
                repo: "extra".to_string(),
                dependencies: vec!["gtk3".to_string(), "nss".to_string()],
                installed_size: 200 * 1024 * 1024,
            },
            MockPackage {
                name: "git".to_string(),
                version: "2.43.0".to_string(),
                description: "The fast distributed version control system".to_string(),
                repo: "extra".to_string(),
                dependencies: vec!["curl".to_string(), "openssl".to_string()],
                installed_size: 30 * 1024 * 1024,
            },
        ])
    }

    /// Create a mock database with common Debian packages
    pub fn debian_mock() -> Self {
        Self::with_packages(vec![
            MockPackage {
                name: "apt".to_string(),
                version: "2.6.1".to_string(),
                description: "commandline package manager".to_string(),
                repo: "main".to_string(),
                dependencies: vec!["libc6".to_string(), "libstdc++6".to_string()],
                installed_size: 4 * 1024 * 1024,
            },
            MockPackage {
                name: "firefox-esr".to_string(),
                version: "115.6.0esr".to_string(),
                description: "Mozilla Firefox web browser - Extended Support Release".to_string(),
                repo: "main".to_string(),
                dependencies: vec!["libgtk-3-0".to_string()],
                installed_size: 180 * 1024 * 1024,
            },
            MockPackage {
                name: "git".to_string(),
                version: "2.39.2".to_string(),
                description: "fast, scalable, distributed revision control system".to_string(),
                repo: "main".to_string(),
                dependencies: vec!["libcurl4".to_string()],
                installed_size: 28 * 1024 * 1024,
            },
        ])
    }
}

/// Mock runtime version manager
#[derive(Default)]
pub struct MockRuntimeManager {
    installed: Arc<Mutex<HashMap<String, Vec<String>>>>,
    active: Arc<Mutex<HashMap<String, String>>>,
}

impl MockRuntimeManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install_version(&self, runtime: &str, version: &str) {
        self.installed
            .lock()
            .unwrap()
            .entry(runtime.to_string())
            .or_default()
            .push(version.to_string());
    }

    pub fn set_active(&self, runtime: &str, version: &str) {
        self.active
            .lock()
            .unwrap()
            .insert(runtime.to_string(), version.to_string());
    }

    pub fn get_active(&self, runtime: &str) -> Option<String> {
        self.active.lock().unwrap().get(runtime).cloned()
    }

    pub fn list_installed(&self, runtime: &str) -> Vec<String> {
        self.installed
            .lock()
            .unwrap()
            .get(runtime)
            .cloned()
            .unwrap_or_default()
    }

    pub fn is_installed(&self, runtime: &str, version: &str) -> bool {
        self.installed
            .lock()
            .unwrap()
            .get(runtime)
            .is_some_and(|v| v.contains(&version.to_string()))
    }
}

/// Mock network client for testing without real network access
pub struct MockNetworkClient {
    responses: HashMap<String, MockResponse>,
    request_log: Arc<Mutex<Vec<String>>>,
}

pub struct MockResponse {
    pub status: u16,
    pub body: String,
}

impl MockNetworkClient {
    pub fn new() -> Self {
        Self {
            responses: HashMap::new(),
            request_log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn mock_response(&mut self, url: &str, status: u16, body: &str) {
        self.responses.insert(
            url.to_string(),
            MockResponse {
                status,
                body: body.to_string(),
            },
        );
    }

    pub fn get(&self, url: &str) -> Result<MockResponse, String> {
        self.request_log.lock().unwrap().push(url.to_string());
        self.responses
            .get(url)
            .cloned()
            .ok_or_else(|| format!("No mock response for {url}"))
    }

    pub fn requests(&self) -> Vec<String> {
        self.request_log.lock().unwrap().clone()
    }
}

impl Clone for MockResponse {
    fn clone(&self) -> Self {
        Self {
            status: self.status,
            body: self.body.clone(),
        }
    }
}

impl Default for MockNetworkClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock daemon for testing daemon-dependent features
pub struct MockDaemon {
    running: Arc<Mutex<bool>>,
    cache: Arc<Mutex<HashMap<String, String>>>,
}

impl MockDaemon {
    pub fn new() -> Self {
        Self {
            running: Arc::new(Mutex::new(false)),
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start(&self) {
        *self.running.lock().unwrap() = true;
    }

    pub fn stop(&self) {
        *self.running.lock().unwrap() = false;
    }

    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    pub fn cache_set(&self, key: &str, value: &str) {
        self.cache
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_string());
    }

    pub fn cache_get(&self, key: &str) -> Option<String> {
        self.cache.lock().unwrap().get(key).cloned()
    }
}

impl Default for MockDaemon {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock file system for testing file operations
pub struct MockFileSystem {
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockFileSystem {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn write(&self, path: &str, content: &[u8]) {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_string(), content.to_vec());
    }

    pub fn write_str(&self, path: &str, content: &str) {
        self.write(path, content.as_bytes());
    }

    pub fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.files.lock().unwrap().get(path).cloned()
    }

    pub fn read_str(&self, path: &str) -> Option<String> {
        self.read(path)
            .map(|b| String::from_utf8_lossy(&b).to_string())
    }

    pub fn exists(&self, path: &str) -> bool {
        self.files.lock().unwrap().contains_key(path)
    }

    pub fn delete(&self, path: &str) -> bool {
        self.files.lock().unwrap().remove(path).is_some()
    }

    pub fn list(&self, prefix: &str) -> Vec<String> {
        self.files
            .lock()
            .unwrap()
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect()
    }
}

impl Default for MockFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Mock package manager implementing the PackageManager trait
pub struct MockPackageManager {
    pub db: MockPackageDb,
}

impl MockPackageManager {
    pub fn new(db: MockPackageDb) -> Self {
        Self { db }
    }
}

impl omg_lib::package_managers::PackageManager for MockPackageManager {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn search(&self, query: &str) -> BoxFuture<'static, anyhow::Result<Vec<omg_lib::core::Package>>> {
        use omg_lib::core::{Package, PackageSource};
        use omg_lib::package_managers::parse_version_or_zero;
        let query = query.to_string();
        let db = self.db.clone();
        async move {
            Ok(db
                .search(&query)
                .into_iter()
                .map(|p| Package {
                    installed: db.is_installed(&p.name),
                    name: p.name,
                    version: parse_version_or_zero(&p.version),
                    description: p.description,
                    source: PackageSource::Official,
                })
                .collect())
        }
        .boxed()
    }

    fn install(&self, packages: &[String]) -> BoxFuture<'static, anyhow::Result<()>> {
        let packages = packages.to_vec();
        let db = self.db.clone();
        async move {
            for pkg in &packages {
                db.install(pkg).map_err(|e| anyhow::anyhow!(e))?;
            }
            Ok(())
        }
        .boxed()
    }

    fn remove(&self, packages: &[String]) -> BoxFuture<'static, anyhow::Result<()>> {
        let packages = packages.to_vec();
        let db = self.db.clone();
        async move {
            for pkg in &packages {
                db.remove(pkg).map_err(|e| anyhow::anyhow!(e))?;
            }
            Ok(())
        }
        .boxed()
    }

    fn update(&self) -> BoxFuture<'static, anyhow::Result<()>> {
        async move { Ok(()) }.boxed()
    }

    fn sync(&self) -> BoxFuture<'static, anyhow::Result<()>> {
        async move { Ok(()) }.boxed()
    }

    fn info(&self, package: &str) -> BoxFuture<'static, anyhow::Result<Option<omg_lib::core::Package>>> {
        use omg_lib::core::{Package, PackageSource};
        use omg_lib::package_managers::parse_version_or_zero;
        let package = package.to_string();
        let db = self.db.clone();
        async move {
            Ok(db.get(&package).map(|p| Package {
                installed: db.is_installed(&p.name),
                name: p.name,
                version: parse_version_or_zero(&p.version),
                description: p.description,
                source: PackageSource::Official,
            }))
        }
        .boxed()
    }

    fn list_installed(&self) -> BoxFuture<'static, anyhow::Result<Vec<omg_lib::core::Package>>> {
        use omg_lib::core::{Package, PackageSource};
        use omg_lib::package_managers::parse_version_or_zero;
        let db = self.db.clone();
        async move {
            let installed_names = db.installed.lock().unwrap().clone();
            let mut results = Vec::new();
            for name in installed_names {
                if let Some(p) = db.get(&name) {
                    results.push(Package {
                        installed: true,
                        name: p.name,
                        version: parse_version_or_zero(&p.version),
                        description: p.description,
                        source: PackageSource::Official,
                    });
                }
            }
            Ok(results)
        }
        .boxed()
    }

    fn get_status(&self) -> BoxFuture<'static, anyhow::Result<(usize, usize, usize, usize)>> {
        let db = self.db.clone();
        async move {
            let total = db.packages.lock().unwrap().len();
            let explicit = db.installed.lock().unwrap().len();
            Ok((total, explicit, 0, 0))
        }
        .boxed()
    }

    fn list_explicit(&self) -> BoxFuture<'static, anyhow::Result<Vec<String>>> {
        let db = self.db.clone();
        async move {
            let installed = db.installed.lock().unwrap().clone();
            Ok(installed)
        }
        .boxed()
    }

    fn list_updates(&self) -> BoxFuture<'static, anyhow::Result<Vec<omg_lib::package_managers::types::UpdateInfo>>> {
        let db = self.db.clone();
        async move { Ok(db.updates.lock().unwrap().clone()) }.boxed()
    }

    fn is_installed(&self, package: &str) -> BoxFuture<'static, bool> {
        let db = self.db.clone();
        let package = package.to_string();
        async move {
            db.is_installed(&package)
        }
        .boxed()
    }
}
