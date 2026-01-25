//! Update functionality for packages

use anyhow::Result;
use dialoguer::Confirm;
use owo_colors::OwoColorize;
use std::sync::Arc;

use crate::cli::tea::run_update_elm;
use crate::cli::{style, ui};
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;

/// Update all packages using modern Elm Architecture
pub async fn update(check_only: bool, yes: bool) -> Result<()> {
    // Use the modern Bubble Tea-inspired UI
    if let Err(e) = run_update_elm(check_only, yes) {
        // Fallback to original implementation on error
        eprintln!("Warning: Elm UI failed, falling back to basic mode: {e}");
        update_fallback(check_only, yes).await
    } else {
        Ok(())
    }
}

/// Fallback implementation using original approach
async fn update_fallback(check_only: bool, yes: bool) -> Result<()> {
    let pm = Arc::from(get_package_manager());
    let service = PackageService::new(pm);

    let pb = style::spinner("Checking for updates...");
    let updates = service.list_updates().await?;
    pb.finish_and_clear();

    if updates.is_empty() {
        ui::print_spacer();
        ui::print_success("System is up to date!");
        ui::print_spacer();
        return Ok(());
    }

    ui::print_header("OMG", &format!("Found {} update(s)", updates.len()));
    ui::print_spacer();

    for up in &updates {
        let update_label = match (
            semver::Version::parse(up.old_version.trim_start_matches(|c: char| !c.is_numeric())),
            semver::Version::parse(up.new_version.trim_start_matches(|c: char| !c.is_numeric())),
        ) {
            (Ok(old), Ok(new)) => {
                if new.major > old.major {
                    "MAJOR".red().bold().to_string()
                } else if new.minor > old.minor {
                    "minor".yellow().bold().to_string()
                } else {
                    "patch".green().bold().to_string()
                }
            }
            _ => "update".dimmed().to_string(),
        };

        println!(
            "  {:>8} {} {} {} → {}",
            update_label,
            style::package(&up.name),
            style::dim(&format!("({})", up.repo)),
            style::dim(&up.old_version),
            style::version(&up.new_version)
        );
    }

    if check_only {
        println!("\n{}", style::dim("Run 'omg update' to install"));
        return Ok(());
    }

    // Handle confirmation in both interactive and non-interactive modes
    if !yes {
        if console::user_attended() {
            // Interactive mode: show confirmation dialog
            if !Confirm::with_theme(&ui::prompt_theme())
                .with_prompt("\nProceed with system upgrade?")
                .default(true)
                .interact()?
            {
                ui::print_spacer();
                ui::print_warning("Upgrade cancelled.");
                ui::print_spacer();
                return Ok(());
            }
        } else {
            // Non-interactive mode: either auto-confirm (if --yes) or show helpful error
            anyhow::bail!(
                "This command requires an interactive terminal or the --yes flag.\n\
                 For automation/CI, use: omg update --yes\n\
                 Or run: sudo omg update"
            );
        }
    }

    println!("\n{} Executing system upgrade...", style::arrow("→"));
    service.update().await?;

    Ok(())
}
