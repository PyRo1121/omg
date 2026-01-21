//! Remove functionality for packages

use anyhow::Result;
use std::sync::Arc;

use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;

/// Remove packages
pub async fn remove(packages: &[String], recursive: bool, _yes: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    // SECURITY: Validate package names
    for pkg in packages {
        if let Err(e) = crate::core::security::validate_package_name(pkg) {
            anyhow::bail!("Invalid package name '{pkg}': {e}");
        }
    }

    let pm = Arc::from(get_package_manager());
    let service = PackageService::new(pm);

    service.remove(packages, recursive).await?;

    Ok(())
}
