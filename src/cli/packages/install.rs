//! Install functionality for packages

use anyhow::Result;
#[cfg(feature = "arch")]
use dialoguer::{Confirm, theme::ColorfulTheme};

use crate::cli::style;
#[cfg(feature = "arch")]
use crate::core::history::PackageChange;
use crate::core::security::SecurityPolicy;

use crate::package_managers::get_package_manager;

#[cfg(feature = "arch")]
use super::common::{fuzzy_suggest, log_transaction};
#[cfg(feature = "arch")]
use super::local::extract_local_metadata;

#[cfg(feature = "arch")]
use crate::package_managers::{
    AurClient, get_sync_pkg_info, search_sync,
};

/// Install packages (auto-detects AUR) with Graded Security
pub async fn install(packages: &[String], yes: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    let pm = get_package_manager();

    if pm.name() == "apt" {
        return pm.install(packages).await;
    }

    let policy = SecurityPolicy::load_default().unwrap_or_default();

    println!(
        "{} Analyzing {} package(s) with {} model...\n",
        style::header("OMG"),
        packages.len(),
        style::success("Graded Security")
    );

    #[cfg(not(feature = "arch"))]
    {
        let _ = policy;
        let _ = yes;
        anyhow::bail!("Install not fully implemented for this backend - use debian backend");
    }

    #[cfg(feature = "arch")]
    install_arch(packages, yes, policy, &*pm).await
}

#[cfg(feature = "arch")]
async fn install_arch(
    packages: &[String],
    yes: bool,
    policy: SecurityPolicy,
    pm: &dyn crate::package_managers::PackageManager,
) -> Result<()> {
    use crate::core::history::TransactionType;
    let aur = AurClient::new();
    let mut official = Vec::new();
    let mut aur_pkgs = Vec::new();
    let mut local_pkgs = Vec::new();
    let mut not_found = Vec::new();
    let mut changes: Vec<PackageChange> = Vec::new();

    for pkg_name in packages {
        // Check if it's a local package file
        if std::path::Path::new(pkg_name).exists() && pkg_name.contains(".pkg.tar.") {
            let path = std::path::Path::new(pkg_name);

            // Extract metadata for robust security check
            let (name, version, license, grade) = match extract_local_metadata(path) {
                Ok(info) => {
                    let license = info.licenses.first().cloned();
                    // Local packages are Risk unless signed (signature check TODO)
                    // But now we have valid metadata for policy checks (banned licenses, etc.)
                    (info.name, info.version.to_string(), license, crate::core::security::SecurityGrade::Risk)
                }
                Err(e) => {
                    println!("{}", style::warning(&format!("Failed to parse local package metadata: {}", e)));
                    // Fallback to filename parsing or just raw input, keeping Risk grade
                    (pkg_name.clone(), "local".to_string(), None, crate::core::security::SecurityGrade::Risk)
                }
            };

            // Perform policy check for local file
            if let Err(e) = policy.check_package(&name, false, license.as_deref(), grade) {
                 println!("{}", style::error(&format!("Security Block (Local File): {e}")));
                 anyhow::bail!("Installation aborted due to security policy on local file");
            }

            local_pkgs.push(pkg_name.clone());

            println!(
                "{} Local:    {} {} [{}]",
                style::arrow("→"),
                style::package(&name),
                style::dim(&format!("({})", version)),
                style::error("Risk") // Display as Risk
            );

            changes.push(PackageChange {
                name,
                old_version: None,
                new_version: Some(version),
                source: "local".to_string(),
            });

            continue;
        }

        let mut target_pkg_name = pkg_name.clone();

        // Try to find package info
        let mut sync_info = get_sync_pkg_info(&target_pkg_name).ok().flatten();
        let mut aur_info = if sync_info.is_none() {
            aur.info(&target_pkg_name).await.ok().flatten()
        } else {
            None
        };

        // If not found, try to resolve typo
        if sync_info.is_none() && aur_info.is_none() {
            // Only if interactive and not auto-confirming
            if console::user_attended() && !yes {
                // 1. Try Fuzzy Matching (Best for typos like 'frfx' -> 'firefox')
                let suggestion = fuzzy_suggest(&target_pkg_name);

                if let Some(best_match) = suggestion {
                    if Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt(format!(
                            "Package '{}' not found. Did you mean '{}'?",
                            style::package(&target_pkg_name),
                            style::package(&best_match)
                        ))
                        .default(false)
                        .interact()?
                    {
                        target_pkg_name = best_match;
                        // Retry finding info
                        sync_info = get_sync_pkg_info(&target_pkg_name).ok().flatten();
                        aur_info = if sync_info.is_none() {
                            aur.info(&target_pkg_name).await.ok().flatten()
                        } else {
                            None
                        };
                    }
                } else {
                    // 2. Try Substring Search (Fallback)
                    // Search official
                    let results = search_sync(&target_pkg_name).unwrap_or_default();
                    if let Some(best_match) = results.first() {
                        if Confirm::with_theme(&ColorfulTheme::default())
                            .with_prompt(format!(
                                "Package '{}' not found. Did you mean '{}' ?",
                                style::package(&target_pkg_name),
                                style::package(&best_match.name)
                            ))
                            .default(true)
                            .interact()?
                        {
                            target_pkg_name.clone_from(&best_match.name);
                            sync_info = get_sync_pkg_info(&target_pkg_name).ok().flatten();
                        }
                    } else {
                        // Try AUR search
                        if let Ok(results) = aur.search(&target_pkg_name).await
                            && let Some(best_match) = results.first()
                            && Confirm::with_theme(&ColorfulTheme::default())
                                .with_prompt(format!(
                                    "Package '{}' not found. Did you mean '{}' (AUR)?",
                                    style::package(&target_pkg_name),
                                    style::package(&best_match.name)
                                ))
                                .default(true)
                                .interact()?
                        {
                            target_pkg_name.clone_from(&best_match.name);
                            aur_info = Some(best_match.clone());
                        }
                    }
                }
            }
        }

        let (grade, is_aur, is_official, license, change) = if let Some(info) = sync_info {
            let grade = policy
                .assign_grade(&info.name, &info.version, false, true)
                .await;
            let license = info.licenses.first().cloned();
            let change = PackageChange {
                name: info.name.clone(),
                old_version: None, // Simplified for now
                new_version: Some(info.version.to_string()),
                source: "official".to_string(),
            };
            (grade, false, true, license, change)
        } else if let Some(info) = aur_info {
            let grade = policy
                .assign_grade(&info.name, &info.version, true, false)
                .await;
            let change = PackageChange {
                name: info.name.clone(),
                old_version: None,
                new_version: Some(info.version.to_string()),
                source: "aur".to_string(),
            };
            (grade, true, false, None, change)
        } else {
            not_found.push(pkg_name.clone());
            continue;
        };

        // Check policy
        if let Err(e) =
            policy.check_package(&target_pkg_name, is_aur, license.as_deref(), grade)
        {
            println!("{}", style::error(&format!("Security Block: {e}")));
            anyhow::bail!("Installation aborted due to security policy");
        }

        let grade_colored = match grade {
            crate::core::security::SecurityGrade::Locked => style::success(&grade.to_string()),
            crate::core::security::SecurityGrade::Verified => style::info(&grade.to_string()),
            crate::core::security::SecurityGrade::Community => {
                style::warning(&grade.to_string())
            }
            crate::core::security::SecurityGrade::Risk => style::error(&grade.to_string()),
        };

        if is_official {
            official.push(target_pkg_name.clone());
            println!(
                "{} Official: {} [{}]",
                style::arrow("→"),
                style::package(&target_pkg_name),
                grade_colored
            );
        } else if is_aur {
            aur_pkgs.push(target_pkg_name.clone());
            println!(
                "{} AUR:      {} [{}]",
                style::warning("→"),
                style::package(&target_pkg_name),
                grade_colored
            );
        }
        changes.push(change);
    }

    if !not_found.is_empty() {
        println!(
            "{}",
            style::error(&format!("Not found: {}", not_found.join(", ")))
        );
    }
    println!();

    // Install official packages (Arch only for now)
    if !official.is_empty() {
        if let Err(e) = pm.install(&official).await {
            log_transaction(TransactionType::Install, changes.clone(), false);
            return Err(e);
        }
    }

    // Install local packages (Arch only)
    if !local_pkgs.is_empty() {
        if let Err(e) = pm.install(&local_pkgs).await {
            log_transaction(TransactionType::Install, changes.clone(), false);
            return Err(e);
        }
    }

    // Install AUR packages (Arch only)
    for pkg in &aur_pkgs {
        if let Err(e) = aur.install(pkg).await {
            log_transaction(TransactionType::Install, changes.clone(), false);
            return Err(e);
        }
    }

    if official.is_empty() && aur_pkgs.is_empty() && local_pkgs.is_empty() {
        println!("{}", style::dim("No packages to install"));
        return Ok(());
    }

    // Log transaction
    log_transaction(TransactionType::Install, changes, true);

    // Track usage
    crate::core::usage::track_install();

    Ok(())
}
