//! Install functionality for packages

use anyhow::Result;
use dialoguer::Select;

use crate::cli::ui;
use crate::core::client::DaemonClient;
use crate::package_managers::get_package_manager;

use futures::future::BoxFuture;

#[cfg(feature = "arch")]
use crate::package_managers::AurClient;

fn extract_missing_package(msg: &str, packages: &[String]) -> Option<String> {
    // Match pattern: "Package {name} not found in any repository" from alpm_ops.rs
    if msg.contains("not found in any repository") || msg.contains("Package not found:") {
        for pkg in packages {
            if msg.contains(pkg.as_str()) {
                return Some(pkg.clone());
            }
        }
    }

    packages.iter().find(|p| msg.contains(p.as_str())).cloned()
}

pub async fn install(packages: &[String], yes: bool, dry_run: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    if dry_run {
        return install_dry_run(packages);
    }

    let pm = get_package_manager();

    // Beautiful header with package count
    print_install_header(packages.len());

    if let Err(e) = pm.install(packages).await {
        let msg = e.to_string();

        if let Some(pkg_name) = extract_missing_package(&msg, packages) {
            return handle_missing_package(pkg_name, e, yes).await;
        }
        return Err(e);
    }

    // Success message
    print_install_success(packages);

    crate::core::usage::track_install(packages);
    Ok(())
}

fn print_install_header(count: usize) {
    use owo_colors::OwoColorize;

    println!();
    println!("  {}", "╭─────────────────────────────────────────╮".cyan());
    println!(
        "  {} {} {}",
        "│".cyan(),
        format!(
            "  Installing {} package{}  ",
            count,
            if count == 1 { "" } else { "s" }
        )
        .bold(),
        "│".cyan()
    );
    println!("  {}", "╰─────────────────────────────────────────╯".cyan());
    println!();
}

fn print_install_success(packages: &[String]) {
    use owo_colors::OwoColorize;

    println!();
    println!(
        "  {}",
        "╭─────────────────────────────────────────╮".green()
    );
    println!(
        "  {} {} {}",
        "│".green(),
        "  ✓ Installation Complete!  ".bold().green(),
        "│".green()
    );
    println!(
        "  {}",
        "╰─────────────────────────────────────────╯".green()
    );

    if packages.len() <= 5 {
        println!();
        for pkg in packages {
            println!("    {} {}", "✓".green().bold(), pkg.bold());
        }
    } else {
        println!();
        println!(
            "    {} {} packages installed successfully",
            "✓".green().bold(),
            packages.len().to_string().bold()
        );
    }
    println!();
}

#[allow(clippy::unnecessary_wraps)] // Result return required: API compat with feature-gated impls
fn install_dry_run(packages: &[String]) -> Result<()> {
    use comfy_table::{Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL};
    use owo_colors::OwoColorize;

    println!();
    println!("  {}", "╭─────────────────────────────────────────╮".blue());
    println!(
        "  {} {} {}",
        "│".blue(),
        "  DRY RUN - Install Preview  ".bold().blue(),
        "│".blue()
    );
    println!("  {}", "╰─────────────────────────────────────────╯".blue());
    println!();

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec!["Package", "Version", "Size", "Status"]);

    #[allow(unused_mut)]
    let mut total_size: u64 = 0;

    for pkg_name in packages {
        #[cfg(feature = "arch")]
        {
            if let Ok(Some(info)) = crate::package_managers::get_sync_pkg_info(pkg_name) {
                let size_mb = info.download_size.unwrap_or(0) as f64 / 1024.0 / 1024.0;
                total_size += info.download_size.unwrap_or(0);

                table.add_row(vec![
                    format!("{}", info.name.bold()),
                    format!("{}", info.version.to_string().cyan()),
                    format!("{:.2} MB", size_mb),
                    format!("{} Official", "✓".green()),
                ]);
            } else {
                table.add_row(vec![
                    format!("{}", pkg_name.bold()),
                    "".to_string(),
                    "".to_string(),
                    format!("{} AUR?", "?".yellow()),
                ]);
            }
        }
        #[cfg(not(feature = "arch"))]
        {
            table.add_row(vec![
                format!("{}", pkg_name.bold()),
                "".to_string(),
                "".to_string(),
                "".to_string(),
            ]);
        }
    }

    println!("{table}");
    println!();
    println!(
        "  {} Total download size: {}",
        "→".cyan().bold(),
        format!("{:.2} MB", total_size as f64 / 1024.0 / 1024.0).bold()
    );
    println!();
    println!(
        "  {} {} No changes will be made (dry run)",
        "ℹ".blue(),
        "•".dimmed()
    );
    println!();

    Ok(())
}

fn handle_missing_package(
    pkg_name: String,
    original_error: anyhow::Error,
    yes: bool,
) -> BoxFuture<'static, Result<()>> {
    Box::pin(async move {
        // Try AUR search first (if feature enabled)
        #[cfg(feature = "arch")]
        {
            if let Ok(aur_pkg) = try_aur_package(&pkg_name).await {
                return handle_aur_package(&pkg_name, aur_pkg, yes).await;
            }
        }

        // Fall back to suggestions from official repos
        let suggestions = try_get_suggestions(&pkg_name).await;

        if suggestions.is_empty() {
            return Err(original_error);
        }

        use owo_colors::OwoColorize;

        println!();
        println!("  {}", "╭─────────────────────────────────────────╮".red());
        println!("  {} {} {}", "│".red(), format!("  Package '{}' Not Found  ", pkg_name).bold().red(), "│".red());
        println!("  {}", "╰─────────────────────────────────────────╯".red());
        println!();
        println!("  {} Did you mean one of these?", "→".cyan().bold());
        println!();

        // Skip interactive prompt when --yes is true
        if !yes && console::user_attended() {
            let selection = Select::with_theme(&ui::prompt_theme())
                .with_prompt("Select a replacement (or Esc to abort)")
                .default(0)
                .items(&suggestions)
                .interact_opt()?;

            if let Some(index) = selection {
                let new_pkg = suggestions[index].clone();
                println!();
                println!(
                    "  {} Replacing {} with {}",
                    "→".cyan().bold(),
                    pkg_name.bold(),
                    new_pkg.green().bold()
                );
                println!();

                return install(&[new_pkg], yes, false).await;
            }
        } else {
            for (i, suggestion) in suggestions.iter().enumerate().take(5) {
                println!("    {}. {}", (i + 1).to_string().cyan(), suggestion.bold());
            }
            println!();
        }

        Err(original_error)
    })
}

async fn try_get_suggestions(query: &str) -> Vec<String> {
    if let Ok(mut client) = DaemonClient::connect().await
        && let Ok(suggestions) = client.suggest(query, Some(5)).await
    {
        return suggestions;
    }
    Vec::new()
}

#[cfg(feature = "arch")]
async fn try_aur_package(pkg_name: &str) -> Result<crate::core::Package> {
    let aur = AurClient::new();

    let results = aur.search(pkg_name).await?;

    results
        .into_iter()
        .find(|p| p.name == pkg_name)
        .ok_or_else(|| anyhow::anyhow!("Package not found in AUR"))
}

#[cfg(feature = "arch")]
async fn handle_aur_package(
    pkg_name: &str,
    aur_pkg: crate::core::Package,
    yes: bool,
) -> Result<()> {
    use comfy_table::{Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL};
    use owo_colors::OwoColorize;

    println!();
    println!(
        "  {}",
        "╭─────────────────────────────────────────╮".yellow()
    );
    println!(
        "  {} {} {}",
        "│".yellow(),
        format!("  ⚠ Package '{}' not found  ", pkg_name)
            .bold()
            .yellow(),
        "│".yellow()
    );
    println!(
        "  {}",
        "╰─────────────────────────────────────────╯".yellow()
    );
    println!();

    // Create beautiful info table
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec!["AUR Package Details"]);

    table.add_row(vec![format!(
        "{} {}",
        "Package:".dimmed(),
        aur_pkg.name.bold()
    )]);
    table.add_row(vec![format!(
        "{} {}",
        "Version:".dimmed(),
        aur_pkg.version.to_string().cyan()
    )]);

    if !aur_pkg.description.is_empty() {
        table.add_row(vec![format!(
            "{} {}",
            "Description:".dimmed(),
            aur_pkg.description
        )]);
    }

    table.add_row(vec![format!(
        "{} {}",
        "Source:".dimmed(),
        "Arch User Repository".magenta()
    )]);

    println!("{table}");
    println!();

    // Security warning
    println!("  {}", "╭─────────────────────────────────────────╮".red());
    println!(
        "  {} {} {}",
        "│".red(),
        "  ⚠ SECURITY NOTICE  ".bold().red(),
        "│".red()
    );
    println!("  {}", "╰─────────────────────────────────────────╯".red());
    println!();
    println!("  {} AUR packages are user-submitted", "•".dimmed());
    println!("  {} Not vetted by Arch Linux", "•".dimmed());
    println!("  {} Review PKGBUILD before installing", "•".dimmed());
    println!();

    let should_install = if yes {
        println!("  {} Auto-accepting (--yes flag)", "→".cyan());
        true
    } else if console::user_attended() {
        use dialoguer::Confirm;
        Confirm::with_theme(&ui::prompt_theme())
            .with_prompt(format!("Install {} from AUR?", pkg_name.bold()))
            .default(false)
            .interact()?
    } else {
        false
    };

    if !should_install {
        println!();
        println!("  {} Installation cancelled", "✗".red().bold());
        println!();
        anyhow::bail!("Installation cancelled by user");
    }

    println!();
    println!(
        "  {}",
        "╭─────────────────────────────────────────╮".magenta()
    );
    println!(
        "  {} {} {}",
        "│".magenta(),
        format!("  Building {}  ", pkg_name).bold().magenta(),
        "│".magenta()
    );
    println!(
        "  {}",
        "╰─────────────────────────────────────────╯".magenta()
    );
    println!();

    println!("  {} Cloning from AUR...", "→".cyan().bold());

    let aur_client = AurClient::new();
    aur_client.install(pkg_name).await?;

    // Success message for AUR
    println!();
    println!(
        "  {}",
        "╭─────────────────────────────────────────╮".green()
    );
    println!(
        "  {} {} {}",
        "│".green(),
        "  ✓ AUR Build Complete!  ".bold().green(),
        "│".green()
    );
    println!(
        "  {}",
        "╰─────────────────────────────────────────╯".green()
    );
    println!();
    println!(
        "    {} {} installed from AUR",
        "✓".green().bold(),
        pkg_name.bold()
    );
    println!();

    crate::core::usage::track_install(&[pkg_name.to_string()]);
    Ok(())
}
