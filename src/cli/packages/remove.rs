//! Remove functionality for packages

use anyhow::Result;

use crate::cli::style;
use crate::package_managers::get_package_manager;

#[cfg(feature = "arch")]
use crate::core::history::PackageChange;
#[cfg(feature = "arch")]
use super::common::log_transaction;

/// Remove packages
pub async fn remove(packages: &[String], recursive: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    let pm = get_package_manager();

    #[cfg(feature = "arch")]
    {
        return remove_arch(packages, recursive, &*pm).await;
    }

    #[cfg(not(feature = "arch"))]
    {
        return pm.remove(packages).await;
    }
}

#[cfg(feature = "arch")]
async fn remove_arch(packages: &[String], recursive: bool, pm: &dyn crate::package_managers::PackageManager) -> Result<()> {
    use crate::core::history::TransactionType;
    let mut changes: Vec<PackageChange> = Vec::new();

    if pm.name() == "pacman" {
        for pkg in packages {
            if let Ok(Some(info)) = crate::package_managers::get_local_package(pkg) {
                changes.push(PackageChange {
                    name: pkg.clone(),
                    old_version: Some(info.version.to_string()),
                    new_version: None,
                    source: "official".to_string(),
                });
            }
        }
    }

    if recursive && pm.name() == "pacman" {
        println!("{}", style::info("Removing with unused dependencies..."));
    }

    let result = pm.remove(packages).await;

    if pm.name() == "pacman" {
        let success = result.is_ok();
        // Log transaction
        log_transaction(TransactionType::Remove, changes, success);
    }

    result
}
