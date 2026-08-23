use anyhow::Result;

use crate::cli::modern_ui;
use crate::package_managers::get_package_manager;

pub async fn install(packages: &[String]) -> Result<()> {
    let pm = get_package_manager()?;

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

    pm.install(packages).await?;

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
    use owo_colors::OwoColorize;

    crate::core::security::validate_package_names(packages)?;

    modern_ui::print_phase_header("📋", "Install Preview", "dry run");

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec!["Package", "Version", "Size", "Status"]);

    for pkg_name in packages {
        table.add_row(vec![
            pkg_name.bold().to_string(),
            String::new(),
            String::new(),
            format!("{} Pending", "•".blue()),
        ]);
    }

    println!("{table}");
    println!();
    println!(
        "  {} {} No changes will be made (dry run)",
        "ℹ".blue(),
        "•".dimmed()
    );
    println!();

    Ok(())
}
