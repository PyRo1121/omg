//! Mock implementations for isolated integration tests.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use omg_lib::core::{Package, PackageSource};
use omg_lib::package_managers::parse_version_or_zero;
use omg_lib::package_managers::types::UpdateInfo;

/// Mock package database for testing without real system access.
#[derive(Default, Clone)]
pub struct MockPackageDb {
    packages: Arc<Mutex<HashMap<String, MockPackage>>>,
    installed: Arc<Mutex<Vec<String>>>,
}

#[derive(Clone, Debug)]
pub struct MockPackage {
    pub name: String,
    pub version: String,
    pub description: String,
}

impl MockPackage {
    fn into_package(self, installed: bool) -> Package {
        Package {
            installed,
            name: self.name,
            version: parse_version_or_zero(&self.version),
            description: self.description,
            source: PackageSource::Official,
        }
    }
}

impl MockPackageDb {
    pub fn with_packages(packages: Vec<MockPackage>) -> Self {
        let packages = packages
            .into_iter()
            .map(|package| (package.name.clone(), package))
            .collect();
        Self {
            packages: Arc::new(Mutex::new(packages)),
            ..Self::default()
        }
    }

    fn install(&self, name: &str) -> Result<(), String> {
        if self.packages.lock().unwrap().contains_key(name) {
            self.installed.lock().unwrap().push(name.to_owned());
            Ok(())
        } else {
            Err(format!("Package {name} not found"))
        }
    }

    fn remove(&self, name: &str) -> Result<(), String> {
        let mut installed = self.installed.lock().unwrap();
        installed
            .iter()
            .position(|installed| installed == name)
            .map_or_else(
                || Err(format!("Package {name} not installed")),
                |position| {
                    installed.remove(position);
                    Ok(())
                },
            )
    }

    fn is_installed(&self, name: &str) -> bool {
        self.installed
            .lock()
            .unwrap()
            .iter()
            .any(|installed| installed == name)
    }

    fn search(&self, query: &str) -> Vec<MockPackage> {
        self.packages
            .lock()
            .unwrap()
            .values()
            .filter(|package| package.name.contains(query) || package.description.contains(query))
            .cloned()
            .collect()
    }

    fn get(&self, name: &str) -> Option<MockPackage> {
        self.packages.lock().unwrap().get(name).cloned()
    }
}

/// Mock package manager implementing the production package-manager trait.
pub struct MockPackageManager {
    db: MockPackageDb,
}

impl MockPackageManager {
    pub const fn new(db: MockPackageDb) -> Self {
        Self { db }
    }
}

impl omg_lib::package_managers::PackageManager for MockPackageManager {
    fn name(&self) -> &'static str {
        "mock"
    }

    fn search(
        &self,
        query: &str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<Package>>> + Send + '_>> {
        let query = query.to_owned();
        Box::pin(async move {
            Ok(self
                .db
                .search(&query)
                .into_iter()
                .map(|package| {
                    let installed = self.db.is_installed(&package.name);
                    package.into_package(installed)
                })
                .collect())
        })
    }

    fn install(
        &self,
        packages: &[String],
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
            for package in &packages {
                self.db.install(package).map_err(anyhow::Error::msg)?;
            }
            Ok(())
        })
    }

    fn remove(
        &self,
        packages: &[String],
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
            for package in &packages {
                self.db.remove(package).map_err(anyhow::Error::msg)?;
            }
            Ok(())
        })
    }

    fn update(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + '_>> {
        Box::pin(async { Ok(()) })
    }

    fn info(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Option<Package>>> + Send + '_>> {
        let package = package.to_owned();
        Box::pin(async move {
            Ok(self.db.get(&package).map(|package| {
                let installed = self.db.is_installed(&package.name);
                package.into_package(installed)
            }))
        })
    }

    fn list_installed(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<Package>>> + Send + '_>> {
        Box::pin(async move {
            let installed_names = self.db.installed.lock().unwrap().clone();
            Ok(installed_names
                .iter()
                .filter_map(|name| self.db.get(name))
                .map(|package| package.into_package(true))
                .collect())
        })
    }

    fn get_status(
        &self,
        _fast: bool,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<(usize, usize, usize, usize)>> + Send + '_>>
    {
        Box::pin(async move {
            let total = self.db.packages.lock().unwrap().len();
            let explicit = self.db.installed.lock().unwrap().len();
            Ok((total, explicit, 0, 0))
        })
    }

    fn list_explicit(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move { Ok(self.db.installed.lock().unwrap().clone()) })
    }

    fn list_updates(
        &self,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<UpdateInfo>>> + Send + '_>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn is_installed(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send + '_>> {
        let installed = self.db.is_installed(package);
        Box::pin(async move { Ok(installed) })
    }
}
