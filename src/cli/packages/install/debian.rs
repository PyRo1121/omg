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

    let pb = modern_ui::modern_spinner("Resolving", "debian packages");
    modern_ui::finish_clear(&pb);

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

    use crate::package_managers::debian_db;
    use crate::package_managers::debian_db::resolver::DependencyResolver;

    modern_ui::print_phase_header("📋", "Install Preview", "dry run");

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec!["Package", "Version", "Size", "Status"]);

    let mut resolver = DependencyResolver::new()?;
    for pkg in packages {
        resolver.add_package(pkg)?;
    }
    let resolution = resolver.resolve()?;

    let upgrade_set: std::collections::HashSet<&str> = resolution
        .to_upgrade
        .iter()
        .map(|(name, _, _)| name.as_str())
        .collect();

    for pkg_name in packages {
        match debian_db::get_info_fast(pkg_name)? {
            Some(info) => {
                let status = if upgrade_set.contains(pkg_name.as_str()) {
                    format!("{} Upgrade", "↺".yellow())
                } else if debian_db::is_installed_fast(pkg_name)? {
                    format!("{} Installed", "•".blue())
                } else {
                    format!("{} Install", "✓".green())
                };

                table.add_row(vec![
                    info.name.bold().to_string(),
                    info.version.clone().cyan().to_string(),
                    "--".to_string(),
                    status,
                ]);
            }
            None => {
                table.add_row(vec![
                    pkg_name.bold().to_string(),
                    String::new(),
                    String::new(),
                    format!("{} Not found", "!".red()),
                ]);
            }
        }
    }

    println!("{table}");
    println!();
    println!(
        "  {} Total download size: {}",
        "→".cyan().bold(),
        format!(
            "{:.2} MB",
            resolution.download_size as f64 / 1024.0 / 1024.0
        )
        .bold()
    );
    if !resolution.to_install.is_empty() || !resolution.to_upgrade.is_empty() {
        println!(
            "  {} Plan: {} install, {} upgrade",
            "→".cyan().bold(),
            resolution.to_install.len(),
            resolution.to_upgrade.len()
        );
    }
    println!();
    println!(
        "  {} {} No changes will be made (dry run)",
        "ℹ".blue(),
        "•".dimmed()
    );
    println!();

    Ok(())
}
