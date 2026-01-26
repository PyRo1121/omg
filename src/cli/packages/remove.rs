//! Remove functionality for packages

use anyhow::Result;

use crate::cli::{style, ui};
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;

pub async fn remove(packages: &[String], recursive: bool, _yes: bool, dry_run: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    for pkg in packages {
        if let Err(e) = crate::core::security::validate_package_name(pkg) {
            anyhow::bail!("Invalid package name '{pkg}': {e}");
        }
    }

    if dry_run {
        return remove_dry_run(packages, recursive);
    }

    // Use fallback mode directly (Elm UI has broken confirmation)
    remove_fallback(packages, recursive).await
}

#[allow(clippy::unnecessary_wraps)]
fn remove_dry_run(packages: &[String], recursive: bool) -> Result<()> {
    ui::print_header("OMG", "Dry Run - Remove Preview");
    ui::print_spacer();

    println!(
        "  {} The following packages would be removed:\n",
        style::info("→")
    );

    #[allow(unused_mut)]
    let mut total_size: u64 = 0;

    for pkg_name in packages {
        #[cfg(feature = "arch")]
        {
            if let Ok(Some(info)) = crate::package_managers::get_package_info(pkg_name) {
                let size_mb = info.size as f64 / 1024.0 / 1024.0;
                total_size += info.size;
                println!(
                    "    {} {} {} ({:.2} MB)",
                    style::error("✗"),
                    style::package(&info.name),
                    style::version(&info.version.to_string()),
                    size_mb
                );
            } else {
                println!(
                    "    {} {} (not installed)",
                    style::warning("?"),
                    style::package(pkg_name)
                );
            }
        }
        #[cfg(not(feature = "arch"))]
        {
            // Use debian_db::get_installed_info_fast for Debian/Ubuntu - checks dpkg/status directly
            if let Ok(Some(info)) =
                crate::package_managers::debian_db::get_installed_info_fast(pkg_name)
            {
                println!(
                    "    {} {} {}",
                    style::error("✗"),
                    style::package(&info.name),
                    style::version(&info.version),
                );
            } else {
                println!(
                    "    {} {} (not installed)",
                    style::warning("?"),
                    style::package(pkg_name)
                );
            }
        }
    }

    if recursive {
        println!(
            "\n  {} Orphaned dependencies would also be removed",
            style::info("→")
        );
    }

    ui::print_spacer();
    println!(
        "  {} Space that would be freed: {:.2} MB",
        style::info("→"),
        total_size as f64 / 1024.0 / 1024.0
    );
    println!("\n  {} No changes made (dry run)", style::dim("ℹ"));

    Ok(())
}

async fn remove_fallback(packages: &[String], recursive: bool) -> Result<()> {
    let pm = get_package_manager();
    let service = PackageService::new(pm);

    ui::print_header("OMG", &format!("Removing {} package(s)", packages.len()));
    ui::print_spacer();

    service.remove(packages, recursive).await?;

    ui::print_spacer();
    ui::print_success("Packages removed successfully");
    ui::print_spacer();

    Ok(())
}
