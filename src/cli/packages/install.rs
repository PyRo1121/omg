//! Install functionality for packages

use anyhow::Result;
use dialoguer::Select;
use futures::future::BoxFuture;

use crate::cli::{modern_ui, ui};
#[cfg(unix)]
use crate::core::client::DaemonClient;
use crate::core::usage::OperationTimer;
#[cfg(feature = "arch")]
use crate::package_managers::AurClient;
use crate::package_managers::get_package_manager;

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

/// Install packages from repositories or AUR
///
/// # Arguments
/// * `packages` - Package names to install
/// * `yes` - Skip confirmation prompts
/// * `dry_run` - Show what would be installed without actually installing
pub async fn install(packages: &[String], yes: bool, dry_run: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    if dry_run {
        return install_dry_run(packages);
    }

    // Start timing the operation
    let timer = OperationTimer::start_with_packages("install", packages);

    let pm = get_package_manager()?;

    // Modern header with package count
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

    // Show resolution phase for user feedback
    let pb = modern_ui::modern_spinner("Resolving", "package sources");
    tokio::time::sleep(std::time::Duration::from_millis(50)).await; // Brief visual feedback

    // Check if all packages exist in official repos BEFORE calling install
    // This avoids unnecessary sudo prompt for packages that don't exist
    #[cfg(feature = "arch")]
    {
        use crate::core::security::is_local_package_file;

        let mut missing_packages = Vec::new();
        for pkg in packages {
            // Skip local package files - they don't need repo lookup
            // These are absolute paths to .pkg.tar.zst files from AUR builds
            if is_local_package_file(pkg) {
                modern_ui::finish_info(&pb, &format!("Local package: {pkg}"));
                continue;
            }
            if crate::package_managers::get_sync_pkg_info(pkg)
                .ok()
                .flatten()
                .is_none()
            {
                missing_packages.push(pkg.clone());
            }
        }
        modern_ui::finish_clear(&pb);

        // If any packages are missing from official repos, try AUR WITHOUT sudo prompt
        if !missing_packages.is_empty() {
            // CRITICAL FIX: Don't call pm.install() for missing packages!
            // That would prompt for sudo unnecessarily for AUR packages.

            if missing_packages.len() == 1 {
                // Single missing package - go straight to AUR
                return handle_missing_package(
                    missing_packages[0].clone(),
                    anyhow::anyhow!("Package not found in official repos"),
                    yes,
                )
                .await;
            }

            // Multiple missing packages - install official ones first, then handle missing
            if missing_packages.len() < packages.len() {
                let official: Vec<String> = packages
                    .iter()
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
                handle_missing_package(
                    missing_pkg,
                    anyhow::anyhow!("Package not found in official repos"),
                    yes,
                )
                .await?;
            }
            return Ok(());
        }
    }

    #[cfg(not(feature = "arch"))]
    {
        modern_ui::finish_clear(&pb);
    }

    if let Err(e) = pm.install(packages).await {
        let msg = e.to_string();

        if let Some(pkg_name) = extract_missing_package(&msg, packages) {
            return handle_missing_package(pkg_name, e, yes).await;
        }
        return Err(e);
    }

    // Modern success message
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

    // Track with timing telemetry
    crate::core::usage::track_install_timed(packages, timer.elapsed_ms(), true, None);
    Ok(())
}

// Removed old box-drawing functions - using modern_ui now

#[expect(clippy::unnecessary_wraps)] // Result return required: API compat with feature-gated impls
fn install_dry_run(packages: &[String]) -> Result<()> {
    use comfy_table::{Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL};
    use owo_colors::OwoColorize;

    // Modern dry-run header
    crate::cli::modern_ui::print_phase_header("📋", "Install Preview", "dry run");

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
                    info.name.bold().to_string(),
                    info.version.to_string().cyan().to_string(),
                    format!("{:.2} MB", size_mb),
                    format!("{} Official", "✓".green()),
                ]);
            } else {
                table.add_row(vec![
                    pkg_name.bold().to_string(),
                    String::new(),
                    String::new(),
                    format!("{} AUR?", "?".yellow()),
                ]);
            }
        }
        #[cfg(not(feature = "arch"))]
        {
            table.add_row(vec![
                pkg_name.bold().to_string(),
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

        modern_ui::print_error(&format!("Package '{pkg_name}' not found"));
        modern_ui::print_info("Did you mean one of these?");
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
    _pkg_name: &str,
    aur_pkg: crate::core::Package,
    yes: bool,
) -> Result<()> {
    // Modern AUR package info display
    modern_ui::print_aur_package_info(
        &aur_pkg.name,
        &aur_pkg.version.to_string(),
        &aur_pkg.description,
    );

    let should_install = if yes {
        modern_ui::print_info("Auto-accepting (--yes flag)");
        true
    } else if console::user_attended() {
        use dialoguer::Confirm;
        Confirm::with_theme(&ui::prompt_theme())
            .with_prompt(format!("Install {} from AUR?", aur_pkg.name))
            .default(false)
            .interact()?
    } else {
        false
    };

    if !should_install {
        modern_ui::print_error("Installation cancelled");
        anyhow::bail!("Installation cancelled by user");
    }

    // Show build phase
    modern_ui::print_aur_build_phase("Building", &aur_pkg.name);

    let aur_client = AurClient::new();
    // CRITICAL: Use aur_pkg.name (e.g., "brave-bin") not original pkg_name (e.g., "brave")
    aur_client.install(&aur_pkg.name).await?;

    // Modern success message for AUR
    modern_ui::print_success(&format!("Built and installed {} from AUR", aur_pkg.name));

    crate::core::usage::track_install(std::slice::from_ref(&aur_pkg.name));
    Ok(())
}
