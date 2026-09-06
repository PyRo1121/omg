use anyhow::{Context, Result};

use crate::cli::{modern_ui, style};
use crate::package_managers::get_package_manager;

use super::enforce_install_policy;

pub async fn install(packages: &[String]) -> Result<()> {
    let pm = get_package_manager()?;
    let policy =
        crate::core::security::SecurityPolicy::load_default().map_err(anyhow::Error::from)?;
    let vulnerability_scanner = crate::core::security::vulnerability::VulnerabilityScanner::new();
    for package in packages {
        let info = pm
            .info(package)
            .await?
            .with_context(|| format!("Package not found: {package}"))?;
        enforce_install_policy(
            &policy,
            &vulnerability_scanner,
            &info.name,
            &info.version,
            false,
            None,
        )
        .await?;
    }

    modern_ui::print_phase_header(
        "📦",
        "Install",
        &format!(
            "{} {}",
            packages.len(),
            if packages.len() == 1 {
                "package"
            } else {
                "packages"
            }
        ),
    );

    let history = crate::core::history::HistoryManager::new()?;
    if let Some(operation) = pm.transact_with_history(
        crate::core::history::TransactionType::Install,
        packages,
        Some(&history),
    ) {
        operation.await?;
    } else {
        pm.install(packages).await?;
    }

    modern_ui::print_success_with_packages(
        &format!(
            "Installed {} {}",
            packages.len(),
            if packages.len() == 1 {
                "package"
            } else {
                "packages"
            }
        ),
        packages,
    );
    crate::core::usage::track_install_result(packages, true);
    Ok(())
}

pub fn install_dry_run(packages: &[String]) -> Result<()> {
    use comfy_table::{Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL};

    crate::core::security::validate_package_names(packages)?;

    modern_ui::print_phase_header("📋", "Install Preview", "dry run");

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec!["Package", "Version", "Size", "Status"]);

    for pkg_name in packages {
        table.add_row(vec![
            style::emphasis(pkg_name),
            String::new(),
            String::new(),
            format!("{} Pending", style::informative("•")),
        ]);
    }

    println!("{table}");
    println!();
    println!(
        "  {} {} No changes will be made (dry run)",
        style::info("ℹ"),
        style::dim("•")
    );
    println!();

    Ok(())
}
