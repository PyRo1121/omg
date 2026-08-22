use std::time::Instant;

use anyhow::{Context, Result};
use dialoguer::Select;
use futures::future::BoxFuture;

use crate::cli::{modern_ui, ui};
#[cfg(unix)]
use crate::core::client::DaemonClient;
#[cfg(unix)]
use crate::core::client::PooledSyncClient;
use crate::core::security::is_local_package_file;
use crate::core::usage::OperationTimer;
use crate::package_managers::AurClient;
use crate::package_managers::get_package_manager;

/// Marker emitted by the ALPM transaction layer when a requested sync
/// package is absent from every configured repository. This exact diagnostic
/// is the only signal allowed to route a failed install into the AUR
/// fallback; everything else propagates verbatim.
const MISSING_FROM_REPOS_MARKER: &str = "not found in any configured repository";

use super::MAX_REPLACEMENT_HOPS;

pub async fn install(packages: &[String], yes: bool, replacement_hops: u32) -> Result<()> {
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

    let operation_result = if missing_packages.is_empty() {
        match pm.install(packages).await {
            Ok(()) => Ok(()),
            Err(error) => {
                let message = error.to_string();
                if let Some(package_name) = extract_missing_package(&message, packages) {
                    missing_packages.push(package_name.clone());
                    handle_missing_package(package_name, error, yes, replacement_hops).await
                } else {
                    Err(error)
                }
            }
        }
    } else if missing_packages.len() == 1 {
        handle_missing_package(
            missing_packages[0].clone(),
            anyhow::anyhow!("Package not found in official repos"),
            yes,
            replacement_hops,
        )
        .await
    } else {
        async {
            if missing_packages.len() < packages.len() {
                let official: Vec<String> = packages
                    .iter()
                    .filter(|package| !missing_packages.contains(package))
                    .cloned()
                    .collect();
                if !official.is_empty() {
                    pm.install(&official).await?;
                }
            }

            for missing_pkg in &missing_packages {
                handle_missing_package(
                    missing_pkg.clone(),
                    anyhow::anyhow!("Package not found in official repos"),
                    yes,
                    replacement_hops,
                )
                .await?;
            }
            Ok(())
        }
        .await
    };

    record_install_history(packages, &missing_packages, operation_result)?;

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

fn record_install_history(
    packages: &[String],
    aur_packages: &[String],
    operation_result: Result<()>,
) -> Result<()> {
    use crate::core::history::{HistoryManager, PackageChange, TransactionType};

    let changes = packages
        .iter()
        .map(|package| {
            Ok(PackageChange {
                name: history_package_name(package),
                old_version: None,
                new_version: None,
                source: if is_local_package_file(package) {
                    "local"
                } else if aur_packages.contains(package) {
                    "aur"
                } else {
                    "pacman"
                }
                .to_string(),
            })
        })
        .collect::<Result<Vec<_>>>()?;

    HistoryManager::new()?.finish_operation(TransactionType::Install, changes, operation_result)
}

fn history_package_name(package: &str) -> String {
    if !is_local_package_file(package) {
        return package.to_string();
    }

    try_local_package_name(package).unwrap_or_else(|error| {
        // History must retain the package operation's original error even when
        // an invalid local archive has no readable metadata.
        tracing::warn!("Could not read local package name for history: {error}");
        package.to_string()
    })
}

fn try_local_package_name(package: &str) -> Result<String> {
    let package_path = std::fs::canonicalize(package)
        .with_context(|| format!("Failed to resolve installed package file {package}"))?;
    let package_path = package_path
        .to_str()
        .context("Installed package path contains invalid UTF-8")?;
    let alpm = alpm::Alpm::new(
        crate::core::paths::pacman_root()
            .to_string_lossy()
            .into_owned(),
        crate::core::paths::pacman_db_dir()
            .to_string_lossy()
            .into_owned(),
    )
    .context("Failed to initialize ALPM while recording package history")?;
    let loaded = alpm
        .pkg_load(package_path, false, alpm::SigLevel::NONE)
        .with_context(|| format!("Failed to read installed package metadata from {package}"))?;
    Ok(loaded.name().to_string())
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "signature is shared with fallible backend dry-run implementations"
)]
pub async fn install_dry_run(packages: &[String]) -> Result<()> {
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

                        // Unknown official packages fall through to an AUR
                        // lookup instead of being reported as a successful guess.
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
                let aur = AurClient::new()?;
                let info = aur
                    .info(pkg_name)
                    .await?
                    .with_context(|| format!("Package '{pkg_name}' was not found"))?;
                table.add_row(vec![
                    info.name.bold().to_string(),
                    info.version.to_string().magenta().to_string(),
                    String::new(),
                    format!("{} AUR", "✓".green()),
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

/// Identify which requested package caused a transaction failure.
///
/// Only the transaction layer's canonical missing-package diagnostic
/// ([`MISSING_FROM_REPOS_MARKER`]) may redirect an install into the AUR
/// fallback. The package name is extracted from the quoted diagnostic and
/// matched exactly against the request, so unrelated failures that merely
/// mention a package name (conflicts, commit errors, disk-full) are never
/// misrouted.
fn extract_missing_package(msg: &str, packages: &[String]) -> Option<String> {
    let line = msg
        .lines()
        .find(|l| l.contains(MISSING_FROM_REPOS_MARKER))?;
    let reported = line.split("Package '").nth(1)?;
    let reported = reported.split("' not found").next()?;
    packages.iter().find(|p| p.as_str() == reported).cloned()
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

/// Handle a package that could not be resolved in the official repositories:
/// try the AUR (preferring `-bin` builds), offer suggestions, or fail with the
/// original error.
///
/// Returns a boxed future because accepting a suggestion recurses back into
/// [`install`] through the public entry point; the box breaks the otherwise
/// infinitely-sized future type.
fn handle_missing_package(
    pkg_name: String,
    original_error: anyhow::Error,
    yes: bool,
    replacement_hops: u32,
) -> BoxFuture<'static, Result<()>> {
    Box::pin(async move {
        match try_aur_package(&pkg_name).await {
            Ok(aur_pkg) => return handle_aur_package(&pkg_name, aur_pkg, yes).await,
            Err(error) if is_aur_not_found(&error) => {}
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to look up {pkg_name} on the AUR after official lookup missed")
                });
            }
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
            // The interactive picker performs blocking TTY reads; run it on a
            // blocking thread so the async executor is never stalled.
            let (selection, suggestions) = tokio::task::spawn_blocking(move || {
                let selection = Select::with_theme(&ui::prompt_theme())
                    .with_prompt("Select a replacement (or Esc to abort)")
                    .default(0)
                    .items(&suggestions)
                    .interact_opt();
                (selection, suggestions)
            })
            .await
            .map_err(|error| anyhow::anyhow!("Suggestion prompt task failed: {error}"))?;

            if let Some(index) = selection? {
                let new_pkg = suggestions[index].clone();
                println!();
                println!(
                    "  {} Replacing {} with {}",
                    "→".cyan().bold(),
                    pkg_name.bold(),
                    new_pkg.green().bold()
                );
                println!();
                if replacement_hops == 0 {
                    anyhow::bail!(
                        "Aborting after {MAX_REPLACEMENT_HOPS} replacement attempts; \
                         install '{new_pkg}' explicitly if intended"
                    );
                }
                return super::install_with_replacement_budget(
                    &[new_pkg],
                    yes,
                    false,
                    replacement_hops - 1,
                )
                .await;
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

fn is_aur_not_found(error: &anyhow::Error) -> bool {
    error.to_string().contains("Package not found in AUR")
}

async fn try_aur_package(pkg_name: &str) -> Result<crate::core::Package> {
    let aur = AurClient::new()?;
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

    // The confirmation prompt performs blocking TTY reads; run it on a
    // blocking thread so the async executor is never stalled.
    let should_install = if yes {
        modern_ui::print_info("Auto-accepting (--yes flag)");
        true
    } else if console::user_attended() {
        let prompt = format!("Install {} from AUR?", aur_pkg.name);
        tokio::task::spawn_blocking(move || {
            dialoguer::Confirm::with_theme(&ui::prompt_theme())
                .with_prompt(prompt)
                .default(false)
                .interact()
        })
        .await
        .map_err(|error| anyhow::anyhow!("Confirmation prompt task failed: {error}"))??
    } else {
        false
    };

    if !should_install {
        modern_ui::print_error("Installation cancelled");
        anyhow::bail!("Installation cancelled by user");
    }

    modern_ui::print_aur_build_phase("Building", &aur_pkg.name);

    let aur_client = AurClient::new()?;
    aur_client.install(&aur_pkg.name).await?;

    modern_ui::print_success(&format!("Built and installed {} from AUR", aur_pkg.name));
    crate::core::usage::track_install(std::slice::from_ref(&aur_pkg.name));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_target_is_extracted_from_canonical_diagnostic() {
        let error = "✗ Package 'firefox' not found in any configured repository.\n  \
                     → Run 'omg sync' to update package databases";
        assert_eq!(
            extract_missing_package(error, &["firefox".to_string()]).as_deref(),
            Some("firefox")
        );
    }

    #[test]
    fn unrelated_failure_mentioning_a_package_never_triggers_aur_fallback() {
        // Regression: previously ANY error containing a requested name was
        // misrouted into the AUR/suggestion flow.
        let error = "✗ Transaction preparation failed: conflicting files for firefox\n  \
                     → Try running: omg update && omg install <package>";
        assert_eq!(
            extract_missing_package(error, &["firefox".to_string()]),
            None
        );
    }

    #[test]
    fn diagnostic_naming_a_different_package_does_not_match() {
        let error = "✗ Package 'firefox-esr' not found in any configured repository.";
        assert_eq!(
            extract_missing_package(error, &["firefox".to_string()]),
            None
        );
    }

    #[test]
    fn empty_or_unrelated_errors_yield_none() {
        assert_eq!(extract_missing_package("", &["vim".to_string()]), None);
        assert_eq!(
            extract_missing_package(
                "Failed to initialize ALPM (are you root?)",
                &["vim".to_string()]
            ),
            None
        );
    }

    #[test]
    fn replacement_budget_bounds_interactive_recursion() {
        assert_eq!(MAX_REPLACEMENT_HOPS, 3);
    }
}
