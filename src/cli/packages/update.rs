//! Update functionality for packages

use anyhow::Result;
use dialoguer::{Confirm, theme::ColorfulTheme};
use owo_colors::OwoColorize;
use std::sync::Arc;

use crate::cli::style;
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;

/// Update all packages
pub async fn update(check_only: bool, yes: bool) -> Result<()> {
    let pm = Arc::from(get_package_manager());
    let service = PackageService::new(pm);

    let pb = style::spinner("Checking for updates...");
    let updates = service.list_updates().await?;
    pb.finish_and_clear();

    if updates.is_empty() {
        println!("{} System is up to date!", style::success("✓"));
        return Ok(());
    }

    println!(
        "{} Found {} update(s):",
        style::header("OMG"),
        style::info(&updates.len().to_string())
    );

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

    if !yes
        && console::user_attended()
        && !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("\nProceed with system upgrade?")
            .default(true)
            .interact()?
    {
        println!("{}", style::dim("Upgrade cancelled."));
        return Ok(());
    }

    println!("\n{} Executing system upgrade...", style::arrow("→"));
    service.update().await?;

    Ok(())
}
