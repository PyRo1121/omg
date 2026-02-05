//! Install functionality for packages

use anyhow::Result;
use dialoguer::Select;

use crate::cli::ui;
#[cfg(unix)]
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

    let pm = get_package_manager()?;

    // Beautiful header with package count
    print_install_header(packages.len());

    // Check if all packages exist in official repos BEFORE calling install
    // This avoids unnecessary sudo prompt for packages that don't exist
    #[cfg(feature = "arch")]
    {
        let mut missing_packages = Vec::new();
        for pkg in packages {
            if crate::package_managers::get_sync_pkg_info(pkg).ok().flatten().is_none() {
                missing_packages.push(pkg.clone());
            }
        }

        // If any packages are missing from official repos, try AUR WITHOUT sudo prompt
        if !missing_packages.is_empty() {
            // CRITICAL FIX: Don't call pm.install() for missing packages!
            // That would prompt for sudo unnecessarily for AUR packages.

            if missing_packages.len() == 1 {
                // Single missing package - go straight to AUR
                return handle_missing_package(missing_packages[0].clone(),
                    anyhow::anyhow!("Package not found in official repos"), yes).await;
            }

            // Multiple missing packages - install official ones first, then handle missing
            if missing_packages.len() < packages.len() {
                let official: Vec<String> = packages.iter()
                    .filter(|p| !missing_packages.contains(p))
                    .cloned()
                    .collect();
                if !official.is_empty() {
                    // Only prompt for sudo for official packages
                    pm.install(&official).await?;
                }
            }

            // Handle missing packages one by one (AUR)
            for missing_pkg in missing_packages {
                handle_missing_package(missing_pkg,
                    anyhow::anyhow!("Package not found in official repos"), yes).await?;
            }
            return Ok(());
        }
    }

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

    println!();
    if packages.len() <= 5 {
        for pkg in packages {
            println!("    {} {}", "✓".green().bold(), pkg.bold());
        }
    } else {
    println!(
        " {} {} packages installed successfully",
        "✓".green().bold(),
        packages.len().bold()
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
                    String::new(),
                    String::new(),
                    format!("{} AUR?", "?".yellow()),
                ]);
            }
        }
        #[cfg(not(feature = "arch"))]
        {
            table.add_row(vec![
                format!("{}", pkg_name.bold()),
                String::new(),
                String::new(),
                String::new(),
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
        println!(
            "  {} {} {}",
            "│".red(),
            format!("  Package '{pkg_name}' Not Found  ").bold().red(),
            "│".red()
        );
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
    #[cfg(unix)]
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

    // Check for exact match
    let exact_match = results.iter().find(|p| p.name == pkg_name);

    // Check for -bin version (pre-built binary - much faster to install)
    let bin_name = format!("{pkg_name}-bin");
    let bin_match = results.iter().find(|p| p.name == bin_name);

    // Prefer -bin package if available (instant install vs hours of compilation)
    // Common patterns: brave -> brave-bin, firefox -> firefox-bin, etc.
    if let Some(bin_pkg) = bin_match {
        if exact_match.is_some() {
            use owo_colors::OwoColorize;
            println!();
            println!(
                "  {} Found pre-built binary package: {}",
                "→".cyan().bold(),
                bin_pkg.name.green().bold()
            );
            println!(
                "  {} This installs in seconds instead of compiling from source",
                "ℹ".blue()
            );
        }
        return Ok(bin_pkg.clone());
    }

    exact_match
        .cloned()
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
        format!("  ⚠ Package '{pkg_name}' not found  ")
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
            .with_prompt(format!("Install {} from AUR?", aur_pkg.name.bold()))
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
        format!("  Building {}  ", aur_pkg.name).bold().magenta(),
        "│".magenta()
    );
    println!(
        "  {}",
        "╰─────────────────────────────────────────╯".magenta()
    );
    println!();

    println!("  {} Cloning from AUR...", "→".cyan().bold());

    let aur_client = AurClient::new();
    // CRITICAL: Use aur_pkg.name (e.g., "brave-bin") not original pkg_name (e.g., "brave")
    aur_client.install(&aur_pkg.name).await?;

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
        aur_pkg.name.bold()
    );
    println!();

    crate::core::usage::track_install(std::slice::from_ref(&aur_pkg.name));
    Ok(())
}
