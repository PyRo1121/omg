//! Remove functionality for packages

use anyhow::Result;

use crate::cli::style;
#[cfg(feature = "arch")]
use crate::core::history::PackageChange;

use super::common::use_debian_backend;

#[cfg(feature = "arch")]
use super::common::log_transaction;

#[cfg(feature = "arch")]
use crate::package_managers::OfficialPackageManager;

use crate::package_managers::PackageManager;

#[cfg(feature = "debian")]
use crate::package_managers::AptPackageManager;

/// Remove packages
pub async fn remove(packages: &[String], recursive: bool) -> Result<()> {
    if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            let apt = AptPackageManager::new();
            let _ = recursive;
            return apt.remove(packages).await;
        }
    }
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    #[cfg(feature = "arch")]
    let mut changes: Vec<PackageChange> = Vec::new();
    #[cfg(feature = "arch")]
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

    #[cfg(not(feature = "arch"))]
    {
        anyhow::bail!("Remove not implemented for this backend");
    }

    #[cfg(feature = "arch")]
    {
        use crate::core::history::TransactionType;
        let pacman = OfficialPackageManager::new();

        if recursive {
            println!("{}", style::info("Removing with unused dependencies..."));
        }

        let result = pacman.remove(packages).await;
        let success = result.is_ok();

        // Log transaction
        log_transaction(TransactionType::Remove, changes, success);

        result
    }
}
