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

/// Install packages
pub async fn install(packages: &[String], _yes: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    let pm = get_package_manager();

    ui::print_header("OMG", &format!("Installing {} package(s)", packages.len()));
    ui::print_spacer();

    if let Err(e) = pm.install(packages).await {
        let msg = e.to_string();

        if msg.contains("not found") {
            if let Some(pkg_name) = packages.iter().find(|p| msg.contains(*p)) {
                return handle_missing_package(pkg_name.to_string(), e).await;
            }
        }
        return Err(e);
    }

    crate::core::usage::track_install(packages);
    Ok(())
}

fn handle_missing_package(
    pkg_name: String,
    original_error: anyhow::Error,
) -> BoxFuture<'static, Result<()>> {
    Box::pin(async move {
        let suggestions = try_get_suggestions(&pkg_name).await;

        if suggestions.is_empty() {
            return Err(original_error);
        }

        println!();
        ui::print_error(&format!("Package '{pkg_name}' not found."));
        println!("Did you mean one of these?");
        println!();

        if console::user_attended() {
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

                return install(&[new_pkg], false).await;
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
    if let Ok(mut client) = DaemonClient::connect().await {
        if let Ok(suggestions) = client.suggest(query, Some(5)).await {
            return suggestions;
        }
    }
    Vec::new()
}
