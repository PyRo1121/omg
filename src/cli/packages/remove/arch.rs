use anyhow::{Context, Result};

use crate::cli::packages::common;
use crate::cli::{style, ui};

pub async fn remove(packages: &[String]) -> Result<()> {
    common::remove_via_service(packages).await
}

/// Arch dry run: accurate, because the ALPM transaction always sets
/// `RECURSE | UNNEEDED`, so orphaned dependencies really are removed.
#[allow(
    clippy::unnecessary_wraps,
    reason = "signature is shared with fallible backend dry-run implementations"
)]
pub fn remove_dry_run(packages: &[String], recursive: bool) -> Result<()> {
    let package_info = packages
        .iter()
        .map(|package| {
            crate::package_managers::get_package_info(package)
                .with_context(|| format!("Failed to look up installed package {package}"))?
                .ok_or_else(|| anyhow::anyhow!("Package '{package}' is not installed"))
        })
        .collect::<Result<Vec<_>>>()?;

    crate::cli::modern_ui::print_phase_header("🗑️", "Remove Preview", "dry run");
    println!();
    println!(
        "  {} The following packages would be removed:\n",
        style::info("→")
    );

    let mut total_size: u64 = 0;
    for info in package_info {
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
