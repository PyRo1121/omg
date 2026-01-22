//! Mock package manager for isolated testing and CLI verification
//!
//! Enabled only when OMG_TEST_MODE=1 is set.
//! Persists state to a JSON file in OMG_DATA_DIR to allow stateful tests across CLI runs.

use anyhow::Result;
use futures::future::{BoxFuture, FutureExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::sync::{Arc, Mutex};

use crate::core::{Package, PackageSource, paths};
use crate::package_managers::traits::PackageManager;
use crate::package_managers::types::{UpdateInfo, parse_version_or_zero};

#[derive(Serialize, Deserialize, Default, Clone)]
struct MockState {
    installed: HashSet<String>,
}

/// Mock package database
#[derive(Default, Clone)]
pub struct MockPackageDb {
    pub packages: Arc<Mutex<HashMap<String, MockPackage>>>,
}

#[derive(Clone, Debug)]
pub struct MockPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub repo: String,
}

impl MockPackageDb {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_package(&self, name: &str, version: &str, description: &str, repo: &str) {
        self.packages.lock().unwrap().insert(
            name.to_string(),
            MockPackage {
                name: name.to_string(),
                version: version.to_string(),
                description: description.to_string(),
                repo: repo.to_string(),
            },
        );
    }

    pub fn arch_defaults() -> Self {
        let db = Self::new();
        db.add_package("pacman", "6.0.2", "Arch package manager", "core");
        db.add_package("firefox", "122.0", "Web browser", "extra");
        db.add_package("git", "2.43.0", "Version control", "extra");
        db
    }

    pub fn debian_defaults() -> Self {
        let db = Self::new();
        db.add_package("apt", "2.6.1", "Debian package manager", "main");
        db.add_package("firefox-esr", "115.6.0", "Web browser", "main");
        db.add_package("git", "2.39.2", "Version control", "main");
        db
    }
}

pub struct MockPackageManager {
    pub db: MockPackageDb,
    pub distro_name: &'static str,
}

impl MockPackageManager {
    pub fn new(distro: &str) -> Self {
        let (db, name) = match distro {
            "arch" => (MockPackageDb::arch_defaults(), "pacman"),
            "debian" | "ubuntu" => (MockPackageDb::debian_defaults(), "apt"),
            _ => (MockPackageDb::new(), "mock"),
        };
        Self {
            db,
            distro_name: name,
        }
    }

    fn load_state(distro_name: &str) -> MockState {
        let path = paths::data_dir().join("mock_state.json");
        eprintln!("Mock loading state from {:?}", path);
        if let Ok(data) = fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            // Default installed packages
            let mut state = MockState::default();
            state.installed.insert(distro_name.to_string());
            state
        }
    }

    fn save_state(state: &MockState) {
        let path = paths::data_dir().join("mock_state.json");
        eprintln!("Mock saving state to {:?}", path);
        let _ = fs::create_dir_all(path.parent().unwrap());
        if let Ok(data) = serde_json::to_string(state) {
            let _ = fs::write(&path, data);
        }
    }
}

impl PackageManager for MockPackageManager {
    fn name(&self) -> &'static str {
        self.distro_name
    }

    fn search(&self, query: &str) -> BoxFuture<'static, Result<Vec<Package>>> {
        let query = query.to_lowercase();
        let db = self.db.clone();
        let state = Self::load_state(self.distro_name);
        async move {
            let pkgs = db.packages.lock().unwrap();
            Ok(pkgs
                .values()
                .filter(|p| {
                    p.name.contains(&query) || p.description.to_lowercase().contains(&query)
                })
                .map(|p| Package {
                    name: p.name.clone(),
                    version: parse_version_or_zero(&p.version),
                    description: p.description.clone(),
                    source: PackageSource::Official,
                    installed: state.installed.contains(&p.name),
                })
                .collect())
        }
        .boxed()
    }

    fn install(&self, packages: &[String]) -> BoxFuture<'static, Result<()>> {
        let packages = packages.to_vec();
        let distro_name = self.distro_name;
        async move {
            let mut state = Self::load_state(distro_name);
            for pkg in packages {
                state.installed.insert(pkg);
            }
            Self::save_state(&state);
            Ok(())
        }
        .boxed()
    }

    fn remove(&self, packages: &[String]) -> BoxFuture<'static, Result<()>> {
        let packages = packages.to_vec();
        let distro_name = self.distro_name;
        async move {
            let mut state = Self::load_state(distro_name);
            for pkg in packages {
                state.installed.remove(&pkg);
            }
            Self::save_state(&state);
            Ok(())
        }
        .boxed()
    }

    fn update(&self) -> BoxFuture<'static, Result<()>> {
        async move { Ok(()) }.boxed()
    }

    fn sync(&self) -> BoxFuture<'static, Result<()>> {
        async move { Ok(()) }.boxed()
    }

    fn info(&self, package: &str) -> BoxFuture<'static, Result<Option<Package>>> {
        let package = package.to_string();
        let db = self.db.clone();
        let state = Self::load_state(self.distro_name);
        async move {
            let pkgs = db.packages.lock().unwrap();
            Ok(pkgs.get(&package).map(|p| Package {
                name: p.name.clone(),
                version: parse_version_or_zero(&p.version),
                description: p.description.clone(),
                source: PackageSource::Official,
                installed: state.installed.contains(&p.name),
            }))
        }
        .boxed()
    }

    fn list_installed(&self) -> BoxFuture<'static, Result<Vec<Package>>> {
        let db = self.db.clone();
        let state = Self::load_state(self.distro_name);
        async move {
            let pkgs = db.packages.lock().unwrap();
            Ok(state
                .installed
                .iter()
                .filter_map(|name| pkgs.get(name))
                .map(|p| Package {
                    name: p.name.clone(),
                    version: parse_version_or_zero(&p.version),
                    description: p.description.clone(),
                    source: PackageSource::Official,
                    installed: true,
                })
                .collect())
        }
        .boxed()
    }

    fn get_status(&self, _fast: bool) -> BoxFuture<'static, Result<(usize, usize, usize, usize)>> {
        let db = self.db.clone();
        let state = Self::load_state(self.distro_name);
        async move {
            let total = db.packages.lock().unwrap().len();
            let explicit = state.installed.len();
            Ok((total, explicit, 0, 0))
        }
        .boxed()
    }

    fn list_explicit(&self) -> BoxFuture<'static, Result<Vec<String>>> {
        let state = Self::load_state(self.distro_name);
        async move { Ok(state.installed.into_iter().collect()) }.boxed()
    }

    fn list_updates(&self) -> BoxFuture<'static, Result<Vec<UpdateInfo>>> {
        async move { Ok(Vec::new()) }.boxed()
    }

    fn is_installed(&self, package: &str) -> BoxFuture<'static, bool> {
        let package = package.to_string();
        let state = Self::load_state(self.distro_name);
        async move { state.installed.contains(&package) }.boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_mock_persistence() {
        let dir = tempdir().unwrap();
        // SAFETY: Unit test is single-threaded
        unsafe {
            std::env::set_var("OMG_DATA_DIR", dir.path());
        }

        let pm1 = MockPackageManager::new("arch");
        futures::executor::block_on(pm1.install(&["test-pkg".to_string()])).unwrap();

        // New instance should see the change
        let pm2 = MockPackageManager::new("arch");
        let installed = futures::executor::block_on(pm2.list_explicit()).unwrap();
        assert!(installed.contains(&"test-pkg".to_string()));
    }
}
