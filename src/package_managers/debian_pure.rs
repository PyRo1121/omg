//! Pure Rust Debian/Ubuntu package manager backend
//!
//! Uses `debian_db` for ultra-fast searches and info, and spawns `apt`
//! command for transactions. This allows Debian support without C dependencies.

use anyhow::Result;
use async_trait::async_trait;
use tokio::process::Command;

use crate::core::{Package, PackageSource, is_root};
use crate::package_managers::PackageManager;
use crate::package_managers::debian_db;
use crate::package_managers::types::UpdateInfo;

#[derive(Debug, Default)]
pub struct PureDebianPackageManager;

impl PureDebianPackageManager {
    pub fn new() -> Self {
        Self
    }

    /// Run apt-get command asynchronously to avoid blocking the runtime
    #[allow(clippy::unused_self)] // Method for API consistency with other package managers
    async fn run_apt(&self, args: &[&str]) -> Result<()> {
        let mut cmd = if is_root() {
            Command::new("apt-get")
        } else {
            let mut c = Command::new("sudo");
            c.arg("apt-get");
            c
        };

        let status = cmd.args(args).status().await?;

        if status.success() {
            Ok(())
        } else {
            anyhow::bail!("apt-get failed with status {status}")
        }
    }
}

#[async_trait]
impl PackageManager for PureDebianPackageManager {
    fn name(&self) -> &'static str {
        "apt-pure"
    }

    async fn search(&self, query: &str) -> Result<Vec<Package>> {
        debian_db::search_fast(query)
    }

    async fn install(&self, packages: &[String]) -> Result<()> {
        let mut args = vec!["install", "-y"];
        let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
        args.extend_from_slice(&pkg_refs);
        self.run_apt(&args).await
    }

    async fn remove(&self, packages: &[String]) -> Result<()> {
        let mut args = vec!["remove", "-y"];
        let pkg_refs: Vec<&str> = packages.iter().map(String::as_str).collect();
        args.extend_from_slice(&pkg_refs);
        self.run_apt(&args).await
    }

    async fn update(&self) -> Result<()> {
        self.run_apt(&["upgrade", "-y"]).await
    }

    async fn sync(&self) -> Result<()> {
        self.run_apt(&["update"]).await
    }

    async fn info(&self, package: &str) -> Result<Option<Package>> {
        debian_db::get_info_fast(package)
    }

    async fn list_installed(&self) -> Result<Vec<Package>> {
        let installed = debian_db::list_installed_fast()?;
        Ok(installed
            .into_iter()
            .map(|p| Package {
                name: p.name,
                version: crate::package_managers::types::parse_version_or_zero(&p.version),
                description: p.description,
                source: PackageSource::Official,
                installed: true,
            })
            .collect())
    }

    async fn get_status(&self, _fast: bool) -> Result<(usize, usize, usize, usize)> {
        debian_db::get_counts_fast()
    }

    async fn list_explicit(&self) -> Result<Vec<String>> {
        debian_db::list_explicit_fast()
    }

    async fn list_updates(&self) -> Result<Vec<UpdateInfo>> {
        use crate::package_managers::types::parse_version_or_zero;
        use std::collections::HashMap;

        // Parse Debian package index and installed packages
        debian_db::ensure_index_loaded()?;

        let installed = debian_db::list_installed_fast()?;
        let index_pkgs = debian_db::get_detailed_packages()?;

        // Build map of installed packages for fast lookup
        let installed_map: HashMap<_, _> = installed
            .iter()
            .map(|pkg| (pkg.name.clone(), pkg.version.clone()))
            .collect();

        // Find packages with newer versions available
        let updates = index_pkgs
            .iter()
            .filter_map(|pkg| {
                let installed_ver = installed_map.get(&pkg.name)?;
                let available_ver = parse_version_or_zero(&pkg.version);
                let installed_v = parse_version_or_zero(installed_ver);

                (available_ver > installed_v).then(|| UpdateInfo {
                    name: pkg.name.clone(),
                    old_version: installed_ver.clone(),
                    new_version: pkg.version.clone(),
                    repo: "official".to_string(),
                })
            })
            .collect();

        Ok(updates)
    }

    async fn is_installed(&self, package: &str) -> bool {
        debian_db::is_installed_fast(package)
    }
}
