//! Mock package manager for isolated testing and CLI verification
//!
//! Enabled only when `OMG_TEST_MODE=1` is set.
//! Persists state to a JSON file in `OMG_DATA_DIR` to allow stateful tests across CLI runs.

#![allow(clippy::unwrap_used)]

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};

use crate::core::{Package, PackageSource, paths};
use crate::package_managers::traits::PackageManager;
use crate::package_managers::types::{UpdateInfo, parse_version_or_zero};

#[derive(Serialize, Deserialize, Default, Clone)]
struct MockState {
    installed: HashMap<String, String>,
    available: HashMap<String, String>,
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

    pub fn set_installed_version(&self, name: &str, version: &str) -> Result<()> {
        let mut state = Self::load_state(self.distro_name);
        state
            .installed
            .insert(name.to_string(), version.to_string());
        state
            .available
            .insert(name.to_string(), version.to_string());
        Self::save_state(&state);
        Ok(())
    }

    pub fn set_available_version(&self, name: &str, version: &str) -> Result<()> {
        let mut state = Self::load_state(self.distro_name);
        state
            .available
            .insert(name.to_string(), version.to_string());
        Self::save_state(&state);
        Ok(())
    }

    pub fn create_update_scenario(&self, updates: &[(&str, &str, &str)]) -> Result<()>
    where
        Self: Sized,
    {
        for (name, installed, available) in updates {
            self.set_installed_version(name, installed)?;
            self.set_available_version(name, available)?;
        }
        Ok(())
    }

    fn load_state(distro_name: &str) -> MockState {
        let path = paths::data_dir().join("mock_state.json");
        if let Ok(data) = fs::read_to_string(&path) {
            // Handle migration from old format (HashSet) to new format (HashMap)
            // This is a bit tricky with serde.
            // For now, let's assume clean state or compatible format.
            // If we encounter a HashSet, serde will fail.
            // We can try to parse as Value first.
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&data)
                && let Some(installed_arr) = val.get("installed").and_then(|v| v.as_array())
            {
                // Old format: installed is array of strings
                let mut installed_map = HashMap::new();
                for v in installed_arr {
                    if let Some(s) = v.as_str() {
                        installed_map.insert(s.to_string(), "0".to_string());
                    }
                }
                let available: HashMap<String, String> =
                    serde_json::from_value(val["available"].clone()).unwrap_or_default();
                return MockState {
                    installed: installed_map,
                    available,
                };
            }
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            let mut state = MockState::default();
            // Default installed packages need a version.
            // We don't know the version here easily without db access.
            // But load_state is static.
            // Let's just use "0" for default installed.
            state
                .installed
                .insert(distro_name.to_string(), "0".to_string());
            state
        }
    }

    fn save_state(state: &MockState) {
        let path = paths::data_dir().join("mock_state.json");
        tracing::debug!("Mock saving state to {}", path.display());
        let _ = fs::create_dir_all(path.parent().unwrap());
        if let Ok(data) = serde_json::to_string(state) {
            let _ = fs::write(&path, data);
        }
    }

    // Used only when arch feature is disabled
    #[allow(dead_code)]
    fn is_newer(old: &str, new: &str) -> bool {
        matches!(old.cmp(new), std::cmp::Ordering::Less)
    }
}

#[async_trait]
impl PackageManager for MockPackageManager {
    fn name(&self) -> &'static str {
        self.distro_name
    }

    async fn search(&self, query: &str) -> Result<Vec<Package>> {
        let query = query.to_lowercase();
        let db = self.db.clone();
        let state = Self::load_state(self.distro_name);
        let pkgs = db.packages.lock().unwrap();
        Ok(pkgs
            .values()
            .filter(|p| p.name.contains(&query) || p.description.to_lowercase().contains(&query))
            .map(|p| Package {
                name: p.name.clone(),
                version: parse_version_or_zero(&p.version),
                description: p.description.clone(),
                source: PackageSource::Official,
                installed: state.installed.contains_key(&p.name),
            })
            .collect())
    }

    async fn install(&self, packages: &[String]) -> Result<()> {
        let distro_name = self.distro_name;
        let db = self.db.clone();
        let mut state = Self::load_state(distro_name);
        let pkgs = db.packages.lock().unwrap();
        for pkg in packages {
            // Use available version if present, otherwise db version, otherwise "0"
            let version = state
                .available
                .get(pkg)
                .or_else(|| pkgs.get(pkg).map(|p| &p.version))
                .cloned()
                .unwrap_or_else(|| "0".to_string());
            state.installed.insert(pkg.clone(), version);
        }
        Self::save_state(&state);
        Ok(())
    }

    async fn remove(&self, packages: &[String]) -> Result<()> {
        let distro_name = self.distro_name;
        let mut state = Self::load_state(distro_name);
        for pkg in packages {
            state.installed.remove(pkg);
        }
        Self::save_state(&state);
        Ok(())
    }

    async fn update(&self) -> Result<()> {
        Ok(())
    }

    async fn sync(&self) -> Result<()> {
        Ok(())
    }

    async fn info(&self, package: &str) -> Result<Option<Package>> {
        let db = self.db.clone();
        let state = Self::load_state(self.distro_name);
        let pkgs = db.packages.lock().unwrap();
        Ok(pkgs.get(package).map(|p| Package {
            name: p.name.clone(),
            version: parse_version_or_zero(&p.version),
            description: p.description.clone(),
            source: PackageSource::Official,
            installed: state.installed.contains_key(&p.name),
        }))
    }

    async fn list_installed(&self) -> Result<Vec<Package>> {
        let db = self.db.clone();
        let state = Self::load_state(self.distro_name);
        let pkgs = db.packages.lock().unwrap();
        Ok(state
            .installed
            .iter()
            .map(|(name, version)| {
                if let Some(p) = pkgs.get(name) {
                    Package {
                        name: p.name.clone(),
                        version: parse_version_or_zero(version),
                        description: p.description.clone(),
                        source: PackageSource::Official,
                        installed: true,
                    }
                } else {
                    // Package installed but not in db (e.g. manually added to mock state)
                    Package {
                        name: name.clone(),
                        version: parse_version_or_zero(version),
                        description: "Mock package".to_string(),
                        source: PackageSource::Official,
                        installed: true,
                    }
                }
            })
            .collect())
    }

    async fn get_status(&self, _fast: bool) -> Result<(usize, usize, usize, usize)> {
        let db = self.db.clone();
        let state = Self::load_state(self.distro_name);
        let total = db.packages.lock().unwrap().len();
        let explicit = state.installed.len();
        Ok((total, explicit, 0, 0))
    }

    async fn list_explicit(&self) -> Result<Vec<String>> {
        let state = Self::load_state(self.distro_name);
        Ok(state.installed.keys().cloned().collect())
    }

    async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        let db = self.db.clone();
        let state = Self::load_state(self.distro_name);
        let pkgs = db.packages.lock().unwrap();
        let mut updates = Vec::new();

        for (pkg_name, installed_ver) in &state.installed {
            if let Some(available_ver) = state.available.get(pkg_name) {
                // Use repo from db if available, else "unknown"
                let repo = pkgs
                    .get(pkg_name)
                    .map_or_else(|| "unknown".to_string(), |p| p.repo.clone());

                #[cfg(feature = "arch")]
                let is_update_needed = {
                    use crate::package_managers::types::Version as AlpmVersion;
                    use std::str::FromStr;

                    let installed = AlpmVersion::from_str(installed_ver)
                        .unwrap_or_else(|_| AlpmVersion::from_str("0").unwrap());
                    let available = AlpmVersion::from_str(available_ver)
                        .unwrap_or_else(|_| AlpmVersion::from_str("0").unwrap());

                    available > installed
                };

                #[cfg(not(feature = "arch"))]
                let is_update_needed = Self::is_newer(installed_ver, available_ver);

                if is_update_needed {
                    updates.push(UpdateInfo {
                        name: pkg_name.clone(),
                        old_version: installed_ver.clone(),
                        new_version: available_ver.clone(),
                        repo,
                    });
                }
            }
        }

        Ok(updates)
    }

    async fn is_installed(&self, package: &str) -> bool {
        let state = Self::load_state(self.distro_name);
        state.installed.contains_key(package)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_mock_persistence() {
        let dir = tempdir().unwrap();
        temp_env::with_var("OMG_DATA_DIR", Some(dir.path()), || {
            let pm1 = MockPackageManager::new("arch");
            futures::executor::block_on(pm1.install(&["test-pkg".to_string()])).unwrap();

            // New instance should see the change
            let pm2 = MockPackageManager::new("arch");
            let installed = futures::executor::block_on(pm2.list_explicit()).unwrap();
            assert!(installed.contains(&"test-pkg".to_string()));
        });
    }
}
