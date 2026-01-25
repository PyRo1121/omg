//! Update functionality for packages
//!
//! Handles checking for updates and performing system upgrades.
//! Supports both direct libalpm operations and daemon-mediated updates.

use anyhow::Result;
use dialoguer::Confirm;

use crate::cli::{style, ui};
use crate::package_managers::{get_package_manager, types::UpdateInfo};

/// Update all packages
pub async fn update(check_only: bool, yes: bool) -> Result<()> {
    let pm = get_package_manager();
    let pb = style::spinner("Checking for updates...");
    let updates: Vec<UpdateInfo> = pm.list_updates().await?;
    pb.finish_and_clear();

    if updates.is_empty() {
        ui::print_success("System is up to date!");
        return Ok(());
    }

    ui::print_header("OMG", &format!("Found {} update(s)", updates.len()));

    let mut stdout = std::io::BufWriter::new(std::io::stdout());
    use std::io::Write;

    for update in updates.iter().take(50) {
        writeln!(
            stdout,
            "  {} {} {} {}",
            style::package(&update.name),
            style::dim(&update.old_version),
            style::arrow("->"),
            style::version(&update.new_version)
        )?;
    }

    if updates.len() > 50 {
        writeln!(
            stdout,
            "  {}",
            style::dim(&format!("(+{} more updates)", updates.len() - 50))
        )?;
    }
    stdout.flush()?;

    if check_only {
        println!("\n{}", style::dim("Run 'omg update' to install"));
        return Ok(());
    }

    if !yes && console::user_attended() {
        if !Confirm::with_theme(&ui::prompt_theme())
            .with_prompt("\nProceed with system upgrade?")
            .default(true)
            .interact()?
        {
            ui::print_warning("Upgrade cancelled.");
            return Ok(());
        }
    } else if !yes {
        anyhow::bail!("Use --yes for non-interactive updates");
    }

    println!("\n{}", style::header("Starting Upgrade"));
    pm.update().await?;

    ui::print_success("System upgraded successfully!");
    Ok(())
}
