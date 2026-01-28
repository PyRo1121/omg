//! Update functionality for packages

use anyhow::Result;
use dialoguer::Confirm;

use crate::cli::{style, ui};
use crate::package_managers::{get_package_manager, types::UpdateInfo};

pub async fn update(check_only: bool, yes: bool, dry_run: bool) -> Result<()> {
    let pm = get_package_manager();

    println!("\n{}", style::header("Syncing Databases"));
    pm.sync().await?;

    #[cfg(feature = "arch")]
    {
        use crate::core::env::distro::use_debian_backend;
        if !use_debian_backend() {
            println!("{}", style::header("Syncing AUR Metadata"));
            let client = crate::package_managers::AurClient::new();
            let _ = client.get_update_list().await;
        }
    }

    let pb = style::spinner("Checking for updates...");
    let updates: Vec<UpdateInfo> = pm.list_updates().await?;
    pb.finish_and_clear();

    if updates.is_empty() {
        ui::print_success("System is up to date!");
        return Ok(());
    }

    if dry_run {
        return update_dry_run(&updates);
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

#[allow(clippy::unnecessary_wraps)] // Result return required: API compat with feature-gated impls
fn update_dry_run(updates: &[UpdateInfo]) -> Result<()> {
    ui::print_header("OMG", "Dry Run - Update Preview");
    ui::print_spacer();

    println!(
        "  {} The following packages would be updated:\n",
        style::info("→")
    );

    #[allow(unused_mut)] // Mutated only inside feature-gated block
    let mut total_download: u64 = 0;

    for update in updates.iter().take(50) {
        let download_size = {
            #[cfg(feature = "arch")]
            {
                if let Ok(Some(info)) = crate::package_managers::get_sync_pkg_info(&update.name) {
                    total_download += info.download_size.unwrap_or(0);
                    format!(
                        "{:.2} MB",
                        info.download_size.unwrap_or(0) as f64 / 1024.0 / 1024.0
                    )
                } else {
                    "unknown".to_string()
                }
            }
            #[cfg(not(feature = "arch"))]
            {
                "unknown".to_string()
            }
        };

        println!(
            "    {} {} {} {} {} ({})",
            style::success("↑"),
            style::package(&update.name),
            style::dim(&update.old_version),
            style::arrow("->"),
            style::version(&update.new_version),
            style::dim(&download_size)
        );
    }

    if updates.len() > 50 {
        println!(
            "    {}",
            style::dim(&format!("(+{} more updates)", updates.len() - 50))
        );
    }

    ui::print_spacer();
    println!("  {} Total updates: {}", style::info("→"), updates.len());
    println!(
        "  {} Estimated download: {:.2} MB",
        style::info("→"),
        total_download as f64 / 1024.0 / 1024.0
    );
    ui::print_dry_run_footer();

    Ok(())
}
