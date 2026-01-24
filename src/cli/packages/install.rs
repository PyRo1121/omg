//! Install functionality for packages

use anyhow::Result;
use dialoguer::{Select, theme::ColorfulTheme};
use std::sync::Arc;

use crate::cli::style;
use crate::core::client::DaemonClient;
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;

/// Install packages with Graded Security
pub async fn install(packages: &[String], yes: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    let pm = Arc::from(get_package_manager());
    let service = PackageService::new(pm);

    println!(
        "{} Analyzing {} package(s) with {} model...\n",
        style::header("OMG"),
        packages.len(),
        style::success("Graded Security")
    );

    let mut packages_to_install = packages.to_vec();

    loop {
        match service.install(&packages_to_install, yes).await {
            Ok(()) => {
                // Track usage
                crate::core::usage::track_install(&packages_to_install);
                return Ok(());
            }
            Err(e) => {
                let msg = e.to_string();
                // Check for "Package not found" error from service.rs
                if let Some(pkg_name) = msg.strip_prefix("Package not found: ") {
                    // It's a missing package error. Try to get suggestions.
                    let suggestions = try_get_suggestions(pkg_name).await;

                    if !suggestions.is_empty() {
                        println!(
                            "\n{}",
                            style::warning(&format!("Package '{pkg_name}' not found."))
                        );
                        println!("{}", style::arrow("Did you mean one of these?"));

                        // Check if we're in interactive mode
                        if console::user_attended() {
                            let selection = Select::with_theme(&ColorfulTheme::default())
                                .with_prompt("Select a replacement (or Esc to abort)")
                                .default(0)
                                .items(&suggestions)
                                .interact_opt()?;

                            if let Some(index) = selection {
                                let new_pkg = suggestions[index].clone();
                                println!(
                                    "{} Replacing '{}' with '{}'\n",
                                    style::success("✓"),
                                    pkg_name,
                                    new_pkg
                                );

                                // Replace in list
                                if let Some(pos) =
                                    packages_to_install.iter().position(|x| x == pkg_name)
                                {
                                    packages_to_install[pos] = new_pkg;
                                    continue; // Retry loop with new package list
                                }
                            }
                        } else {
                            // Non-interactive mode: show suggestions and abort with helpful message
                            println!("\n  Suggested alternatives:");
                            for (i, suggestion) in suggestions.iter().enumerate().take(5) {
                                println!("    {}. {}", i + 1, suggestion);
                            }
                            if suggestions.len() > 5 {
                                println!("    ... and {} more", suggestions.len() - 5);
                            }
                            anyhow::bail!(
                                "Package '{pkg_name}' not found. Re-run in interactive mode to select a replacement, or use the correct package name."
                            );
                        }
                    }
                }

                // If we couldn't handle it or user cancelled, return the original error
                return Err(e);
            }
        }
    }
}

/// Try to get fuzzy suggestions from the daemon
async fn try_get_suggestions(query: &str) -> Vec<String> {
    if let Ok(mut client) = DaemonClient::connect().await
        && let Ok(suggestions) = client.suggest(query, Some(5)).await
    {
        return suggestions;
    }
    Vec::new()
}
