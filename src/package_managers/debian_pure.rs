//! Pure Rust Debian/Ubuntu package manager backend
//!
//! Uses `debian_db` for ultra-fast searches and info, and spawns `apt`
//! command for transactions. This allows Debian support without C dependencies.

use anyhow::Result;
use futures::future::{BoxFuture, FutureExt};
use std::process::Command;

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

    fn run_apt(&self, args: &[&str]) -> Result<()> {
        let mut cmd = if is_root() {
            Command::new("apt-get")
        } else {
            let mut c = Command::new("sudo");
            c.arg("apt-get");
            c
        };

        let status = cmd.args(args).status()?;

        if status.success() {
            Ok(())
        } else {
            anyhow::bail!("apt-get failed with status {}", status)
        }
    }
}

impl PackageManager for PureDebianPackageManager {
    fn name(&self) -> &'static str {
        "apt-pure"
    }

    fn search(&self, query: &str) -> BoxFuture<'static, Result<Vec<Package>>> {
        let query = query.to_string();
        async move { debian_db::search_fast(&query) }.boxed()
    }

    fn install(&self, packages: &[String]) -> BoxFuture<'static, Result<()>> {
        let packages = packages.to_vec();
        async move {
            let mut args = vec!["install", "-y"];
            for pkg in &packages {
                args.push(pkg);
            }
            let pm = PureDebianPackageManager::new();
            pm.run_apt(&args)
        }
        .boxed()
    }

    fn remove(&self, packages: &[String]) -> BoxFuture<'static, Result<()>> {
        let packages = packages.to_vec();
        async move {
            let mut args = vec!["remove", "-y"];
            for pkg in &packages {
                args.push(pkg);
            }
            let pm = PureDebianPackageManager::new();
            pm.run_apt(&args)
        }
        .boxed()
    }

    fn update(&self) -> BoxFuture<'static, Result<()>> {
        async move {
            let pm = PureDebianPackageManager::new();
            pm.run_apt(&["upgrade", "-y"])
        }
        .boxed()
    }

    fn sync(&self) -> BoxFuture<'static, Result<()>> {
        async move {
            let pm = PureDebianPackageManager::new();
            pm.run_apt(&["update"])
        }
        .boxed()
    }

    fn info(&self, package: &str) -> BoxFuture<'static, Result<Option<Package>>> {
        let package = package.to_string();
        async move { debian_db::get_info_fast(&package) }.boxed()
    }

    fn list_installed(&self) -> BoxFuture<'static, Result<Vec<Package>>> {
        async move {
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
        .boxed()
    }

    fn get_status(&self, _fast: bool) -> BoxFuture<'static, Result<(usize, usize, usize, usize)>> {
        async move { debian_db::get_counts_fast() }.boxed()
    }

    fn list_explicit(&self) -> BoxFuture<'static, Result<Vec<String>>> {
        async move { debian_db::list_explicit_fast() }.boxed()
    }

    fn list_updates(&self) -> BoxFuture<'static, Result<Vec<UpdateInfo>>> {
        async move {
            // Parse Debian package index and installed packages
            debian_db::ensure_index_loaded()?;

            let installed = debian_db::list_installed_fast()?;
            let index_pkgs = debian_db::get_detailed_packages()?;

            let mut updates = Vec::new();
            let mut installed_map = std::collections::HashMap::new();

            // Build map of installed packages for fast lookup
            for pkg in &installed {
                installed_map.insert(pkg.name.clone(), pkg.version.clone());
            }

            // Find packages with newer versions available
            for pkg in &index_pkgs {
                if let Some(installed_ver) = installed_map.get(&pkg.name) {
                    let available_ver =
                        crate::package_managers::types::parse_version_or_zero(&pkg.version);
                    let installed_v =
                        crate::package_managers::types::parse_version_or_zero(installed_ver);

                    if available_ver > installed_v {
                        updates.push(UpdateInfo {
                            name: pkg.name.clone(),
                            old_version: installed_ver.clone(),
                            new_version: pkg.version.clone(),
                            repo: "official".to_string(),
                        });
                    }
                }
            }

            Ok(updates)
        }
        .boxed()
    }

    fn is_installed(&self, package: &str) -> BoxFuture<'static, bool> {
        let package = package.to_string();
        async move { debian_db::is_installed_fast(&package) }.boxed()
    }
}
