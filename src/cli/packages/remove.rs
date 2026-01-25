//! Remove functionality for packages

use anyhow::Result;
use std::sync::Arc;

use crate::cli::tea::run_remove_elm;
use crate::cli::ui;
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;

/// Remove packages
pub async fn remove(packages: &[String], recursive: bool, yes: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    // SECURITY: Validate package names
    for pkg in packages {
        if let Err(e) = crate::core::security::validate_package_name(pkg) {
            anyhow::bail!("Invalid package name '{pkg}': {e}");
        }
    }

    // Try modern Elm UI first
    if let Err(e) = run_remove_elm(packages.to_vec(), recursive, yes) {
        eprintln!("Warning: Elm UI failed, falling back to basic mode: {e}");
        remove_fallback(packages, recursive).await
    } else {
        Ok(())
    }
}

async fn remove_fallback(packages: &[String], recursive: bool) -> Result<()> {
    let pm = Arc::from(get_package_manager());
    let service = PackageService::new(pm);

    ui::print_header("OMG", &format!("Removing {} package(s)", packages.len()));
    ui::print_spacer();

    service.remove(packages, recursive).await?;

    ui::print_spacer();
    ui::print_success("Packages removed successfully");
    ui::print_spacer();

    Ok(())
}
