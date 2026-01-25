//! Install functionality for packages
//!
//! Handles package installation with graded security checks.
//! Provides interactive suggestions for missing packages.

use anyhow::Result;
use dialoguer::Select;

use crate::cli::{style, ui};
use crate::core::client::DaemonClient;
use crate::package_managers::get_package_manager;

use futures::future::BoxFuture;

/// Extract package name from a "Package not found" error message.
///
/// Error format: "Package not found: {name}"
/// Falls back to substring matching for other error formats.
fn extract_missing_package(msg: &str, packages: &[String]) -> Option<String> {
    // Try to parse the standard error format first
    if let Some(prefix) = msg.strip_prefix("Package not found: ") {
        let pkg_name = prefix.trim();
        // Verify this is one of the packages we tried to install
        if packages.iter().any(|p| p == pkg_name) {
            return Some(pkg_name.to_string());
        }
    }

    // Fallback to substring check for other error formats
    packages.iter().find(|p| msg.contains(p.as_str())).cloned()
}

/// Install packages
pub async fn install(packages: &[String], yes: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    let pm = get_package_manager();

    ui::print_header("OMG", &format!("Installing {} package(s)", packages.len()));
    ui::print_spacer();

    if let Err(e) = pm.install(packages).await {
        let msg = e.to_string();

        if let Some(pkg_name) = extract_missing_package(&msg, packages) {
            return handle_missing_package(pkg_name, e, yes).await;
        }
        return Err(e);
    }

    crate::core::usage::track_install(packages);
    Ok(())
}

fn handle_missing_package(
    pkg_name: String,
    original_error: anyhow::Error,
    yes: bool,
) -> BoxFuture<'static, Result<()>> {
    Box::pin(async move {
        let suggestions = try_get_suggestions(&pkg_name).await;

        if suggestions.is_empty() {
            return Err(original_error);
        }

        println!();
        ui::print_error(format!("Package '{pkg_name}' not found."));
        println!("Did you mean one of these?");
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
                println!(
                    "{} Replacing '{}' with '{}'\n",
                    style::success("✓"),
                    pkg_name,
                    new_pkg
                );

                return install(&[new_pkg], yes).await;
            }
        } else {
            println!("  Suggested alternatives:");
            for (i, suggestion) in suggestions.iter().enumerate().take(5) {
                println!("    {}. {}", i + 1, suggestion);
            }
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
