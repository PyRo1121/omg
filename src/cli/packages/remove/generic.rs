use std::time::Instant;

use anyhow::Result;

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

pub fn remove_dry_run(packages: &[String], recursive: bool) -> Result<()> {
    crate::core::security::validate_package_names(packages)?;

    crate::cli::modern_ui::print_phase_header("🗑️", "Remove Preview", "dry run");
    println!();
    println!(
        "  {} The following packages would be removed:\n",
        style::info("→")
    );

    for pkg_name in packages {
        println!(
            "    {} {} (feature-specific info unavailable)",
            style::dim("○"),
            style::package(pkg_name)
        );
    }

    if recursive {
        println!(
            "\n  {} Orphaned dependencies would also be removed",
            style::info("→")
        );
    }

    ui::print_spacer();
    println!("  {} Space that would be freed: unknown", style::info("→"));
    ui::print_dry_run_footer();
    Ok(())
}
