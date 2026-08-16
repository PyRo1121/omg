use std::time::Instant;

use anyhow::{Context, Result};
use dialoguer::Select;
use futures::future::BoxFuture;

use crate::cli::{modern_ui, ui};
#[cfg(unix)]
use crate::core::client::DaemonClient;
#[cfg(unix)]
use crate::core::client::PooledSyncClient;
use crate::core::usage::OperationTimer;
use crate::package_managers::AurClient;
use crate::package_managers::get_package_manager;

pub async fn install(packages: &[String], yes: bool) -> Result<()> {
    let timer = OperationTimer::start_with_packages("install", packages);
    let resolution_start = Instant::now();

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

    let pb = modern_ui::modern_spinner("Resolving", "package sources");

    use crate::core::security::is_local_package_file;

    #[cfg(unix)]
    let mut daemon_client = DaemonClient::connect().await.ok();

    let mut missing_packages = Vec::new();
    for pkg in packages {
        if is_local_package_file(pkg) {
            modern_ui::finish_info(&pb, &format!("Local package: {pkg}"));
            continue;
        }

        let is_official = lookup_official_package(
            #[cfg(unix)]
            daemon_client.as_mut(),
            pkg,
        )
        .await?;

        if !is_official {
            missing_packages.push(pkg.clone());
        }
    }

    modern_ui::finish_clear(&pb);
    tracing::debug!(
        "install resolution finished in {}ms",
        resolution_start.elapsed().as_millis()
    );

    if !missing_packages.is_empty() {
        if missing_packages.len() == 1 {
            return handle_missing_package(
                missing_packages[0].clone(),
                anyhow::anyhow!("Package not found in official repos"),
                yes,
            )
            .await;
        }

        if missing_packages.len() < packages.len() {
            let official: Vec<String> = packages
                .iter()
                .filter(|p| !missing_packages.contains(p))
                .cloned()
                .collect();
            if !official.is_empty() {
                pm.install(&official).await?;
            }
        }

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

    if let Err(e) = pm.install(packages).await {
        let msg = e.to_string();
        if let Some(pkg_name) = extract_missing_package(&msg, packages) {
            return handle_missing_package(pkg_name, e, yes).await;
        }
        return Err(e);
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

    crate::core::usage::track_install_timed(packages, timer.elapsed_ms(), true, None);
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
pub fn install_dry_run(packages: &[String]) -> Result<()> {
    use comfy_table::{Table, modifiers::UTF8_ROUND_CORNERS, presets::UTF8_FULL};
    use owo_colors::OwoColorize;

    modern_ui::print_phase_header("📋", "Install Preview", "dry run");

    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_header(vec!["Package", "Version", "Size", "Status"]);

    let mut total_size: u64 = 0;
    #[cfg(unix)]
    let mut daemon_client = PooledSyncClient::acquire().ok();

    for pkg_name in packages {
        #[cfg(unix)]
        {
            if let Some(client) = daemon_client.as_mut() {
                let search_result = client.search(pkg_name, Some(8));
                match search_result {
                    Ok(search_result) => {
                        let is_official = search_result
                            .packages
                            .iter()
                            .any(|pkg| pkg.name == pkg_name.as_str() && pkg.source != "AUR");

                        if is_official
                            && let Ok(info) = client.info(pkg_name)
                            && info.source == "official"
                        {
                            let size_mb = info.download_size as f64 / 1024.0 / 1024.0;
                            total_size += info.download_size;

                            table.add_row(vec![
                                info.name.bold().to_string(),
                                info.version.cyan().to_string(),
                                format!("{size_mb:.2} MB"),
                                format!("{} Official", "✓".green()),
                            ]);
                            continue;
                        }

                        if !is_official {
                            table.add_row(vec![
                                pkg_name.bold().to_string(),
                                String::new(),
                                String::new(),
                                format!("{} AUR?", "?".yellow()),
                            ]);
                            continue;
                        }
                    }
                    Err(_) => {
                        daemon_client = None;
                    }
                }
            }
        }

        match crate::package_managers::get_sync_pkg_info(pkg_name) {
            Ok(Some(info)) => {
                let size_mb = info.download_size.unwrap_or(0) as f64 / 1024.0 / 1024.0;
                total_size += info.download_size.unwrap_or(0);

                table.add_row(vec![
                    info.name.bold().to_string(),
                    info.version.to_string().cyan().to_string(),
                    format!("{size_mb:.2} MB"),
                    format!("{} Official", "✓".green()),
                ]);
            }
            Ok(None) => {
                table.add_row(vec![
                    pkg_name.bold().to_string(),
                    String::new(),
                    String::new(),
                    format!("{} AUR?", "?".yellow()),
                ]);
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to look up {pkg_name} in official repositories")
                });
            }
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

fn extract_missing_package(msg: &str, packages: &[String]) -> Option<String> {
    if msg.contains("not found in any repository") || msg.contains("Package not found:") {
        for pkg in packages {
            if msg.contains(pkg.as_str()) {
                return Some(pkg.clone());
            }
        }
    }

    packages.iter().find(|p| msg.contains(p.as_str())).cloned()
}

#[cfg(unix)]
async fn daemon_has_official_package(client: &mut DaemonClient, package: &str) -> Result<bool> {
    let result = client.search(package, Some(8)).await?;
    Ok(result
        .packages
        .iter()
        .any(|pkg| pkg.name == package && pkg.source != "AUR"))
}

async fn lookup_official_package(
    #[cfg(unix)] daemon_client: Option<&mut DaemonClient>,
    package: &str,
) -> Result<bool> {
    #[cfg(unix)]
    if let Some(client) = daemon_client {
        match daemon_has_official_package(client, package).await {
            Ok(found) => return Ok(found),
            Err(error) => {
                tracing::debug!("Daemon official-package lookup failed for {package}: {error}");
            }
        }
    }

    crate::package_managers::get_sync_pkg_info(package)
        .map(|info| info.is_some())
        .with_context(|| format!("Failed to look up {package} in official repositories"))
}

fn handle_missing_package(
    pkg_name: String,
    original_error: anyhow::Error,
    yes: bool,
) -> BoxFuture<'static, Result<()>> {
    Box::pin(async move {
        if let Ok(aur_pkg) = try_aur_package(&pkg_name).await {
            return handle_aur_package(&pkg_name, aur_pkg, yes).await;
        }

        let suggestions = try_get_suggestions(&pkg_name).await;
        if suggestions.is_empty() {
            return Err(original_error);
        }

        use owo_colors::OwoColorize;

        modern_ui::print_error(&format!("Package '{pkg_name}' not found"));
        modern_ui::print_info("Did you mean one of these?");
        println!();

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
                return super::install(&[new_pkg], yes, false).await;
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

async fn try_aur_package(pkg_name: &str) -> Result<crate::core::Package> {
    let aur = AurClient::new();
    let results = aur.search(pkg_name).await?;

    let exact_match = results.iter().find(|p| p.name == pkg_name);
    let bin_name = format!("{pkg_name}-bin");
    let bin_match = results.iter().find(|p| p.name == bin_name);

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

async fn handle_aur_package(
    _pkg_name: &str,
    aur_pkg: crate::core::Package,
    yes: bool,
) -> Result<()> {
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

    modern_ui::print_aur_build_phase("Building", &aur_pkg.name);

    let aur_client = AurClient::new();
    aur_client.install(&aur_pkg.name).await?;

    modern_ui::print_success(&format!("Built and installed {} from AUR", aur_pkg.name));
    crate::core::usage::track_install(std::slice::from_ref(&aur_pkg.name));
    Ok(())
}
