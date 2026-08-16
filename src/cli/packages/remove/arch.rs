use std::time::Instant;

use anyhow::{Context, Result};

use crate::cli::{style, ui};
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;

pub async fn remove(packages: &[String], recursive: bool) -> Result<()> {
    let start_time = Instant::now();
    let pm = get_package_manager()?;
    let service = PackageService::new(pm)?;

    crate::cli::modern_ui::print_phase_header(
        "🗑️",
        "Remove",
        &format!("{} package(s)", packages.len()),
    );
    println!();

    let result = service.remove(packages, recursive).await;

    let duration_ms = start_time.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => crate::core::usage::track_remove_timed(packages, duration_ms, true, None),
        Err(e) => crate::core::usage::track_remove_timed(
            packages,
            duration_ms,
            false,
            Some(&e.to_string()),
        ),
    }

    result?;

    ui::print_spacer();
    ui::print_success("Packages removed successfully");
    ui::print_spacer();
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
pub fn remove_dry_run(packages: &[String], recursive: bool) -> Result<()> {
    crate::cli::modern_ui::print_phase_header("🗑️", "Remove Preview", "dry run");
    println!();
    println!(
        "  {} The following packages would be removed:\n",
        style::info("→")
    );

    let mut total_size: u64 = 0;
    for pkg_name in packages {
        match crate::package_managers::get_package_info(pkg_name) {
            Ok(Some(info)) => {
                let size_mb = info.size as f64 / 1024.0 / 1024.0;
                total_size += info.size;
                println!(
                    "    {} {} {} ({:.2} MB)",
                    style::error("✗"),
                    style::package(&info.name),
                    style::version(&info.version.to_string()),
                    size_mb
                );
            }
            Ok(None) => {
                println!(
                    "    {} {} (not installed)",
                    style::warning("?"),
                    style::package(pkg_name)
                );
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to look up installed package {pkg_name}"));
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
    ui::print_dry_run_footer();
    Ok(())
}
