//! Update functionality for packages

use anyhow::Result;
use dialoguer::Confirm;

use crate::cli::{style, ui};
use crate::package_managers::{get_package_manager, types::UpdateInfo};

/// Fast update - sync + upgrade in a single privileged operation
/// This is the fastest way to update your system
#[cfg(feature = "arch")]
pub async fn update_fast() -> Result<()> {
    use owo_colors::OwoColorize;

    println!();
    println!("  {}", "╭─────────────────────────────────────────╮".cyan());
    println!(
        "  {} {} {}",
        "│".cyan(),
        "  ⚡ Fast System Update  ".bold().cyan(),
        "│".cyan()
    );
    println!("  {}", "╰─────────────────────────────────────────╯".cyan());
    println!();

    // Use the fullupdate command which does sync + sysupgrade in one go
    crate::package_managers::arch::run_privileged_operation("fullupdate", &[], || async {
        // This won't actually run - the fast path in main.rs handles it
        // But we need this for the non-elevated case
        Ok(())
    })
    .await
}

#[cfg(not(feature = "arch"))]
pub async fn update_fast() -> Result<()> {
    update(false, true, false).await
}

/// Turbo update - skip sync, use cached data, instant upgrade
/// Uses daemon's cached update list for zero-latency update detection
#[cfg(feature = "arch")]
pub async fn update_turbo() -> Result<()> {
    use owo_colors::OwoColorize;

    println!();
    println!(
        "  {}",
        "╭─────────────────────────────────────────╮".bright_magenta()
    );
    println!(
        "  {} {} {}",
        "│".bright_magenta(),
        "  🚀 TURBO System Update  ".bold().bright_magenta(),
        "│".bright_magenta()
    );
    println!(
        "  {}",
        "╰─────────────────────────────────────────╯".bright_magenta()
    );
    println!();

    println!(
        "  {} Checking for updates (cached, no sync)...",
        "⚡".bright_yellow()
    );
    let updates = crate::package_managers::get_update_list()?;

    if updates.is_empty() {
        println!();
        println!("  {} System is up to date!", "✓".green().bold());
        println!();
        return Ok(());
    }

    println!(
        "  {} Found {} update(s) - upgrading now...",
        "→".cyan(),
        updates.len().to_string().bright_yellow()
    );
    println!();

    crate::package_managers::arch::run_privileged_operation("turboupdate", &[], || async { Ok(()) })
        .await
}

#[cfg(not(feature = "arch"))]
pub async fn update_turbo() -> Result<()> {
    update(false, true, false).await
}

pub async fn update(check_only: bool, yes: bool, dry_run: bool) -> Result<()> {
    use owo_colors::OwoColorize;

    let pm = get_package_manager();

    print_update_header("Syncing Package Databases");
    pm.sync().await?;

    let mut all_updates: Vec<UpdateInfo> = Vec::with_capacity(32); // Typical update count

    let pb = style::spinner("Checking official repositories...");
    let official_updates: Vec<UpdateInfo> = pm.list_updates().await?;
    pb.finish_and_clear();

    let official_count = official_updates.len();
    all_updates.extend(official_updates);

    #[cfg(feature = "arch")]
    let (aur_count, aur_packages) = {
        use crate::core::env::distro::use_debian_backend;
        if use_debian_backend() {
            (0, Vec::new())
        } else {
            println!("  {} Checking AUR packages...", "→".cyan());
            let client = crate::package_managers::AurClient::new();
            match client.get_update_list().await {
                Ok(aur_updates) => {
                    let count = aur_updates.len();
                    let packages: Vec<String> = aur_updates
                        .iter()
                        .map(|(name, _, _)| name.clone())
                        .collect();
                    for (name, old_ver, new_ver) in aur_updates {
                        all_updates.push(UpdateInfo {
                            name,
                            old_version: old_ver.to_string(),
                            new_version: new_ver.to_string(),
                            repo: "AUR".to_string(),
                        });
                    }
                    (count, packages)
                }
                Err(e) => {
                    tracing::warn!("Failed to check AUR updates: {}", e);
                    (0, Vec::new())
                }
            }
        }
    };

    #[cfg(not(feature = "arch"))]
    let (aur_count, aur_packages): (usize, Vec<String>) = (0, Vec::new());

    println!();

    if all_updates.is_empty() {
        print_up_to_date();
        return Ok(());
    }

    if dry_run {
        return update_dry_run(&all_updates);
    }

    print_update_summary(&all_updates, official_count, aur_count);

    if check_only {
        println!();
        println!(
            "  {} Run {} to install updates",
            "→".dimmed(),
            "omg update".cyan().bold()
        );
        println!();
        return Ok(());
    }

    if !yes && console::user_attended() {
        println!();
        if !Confirm::with_theme(&ui::prompt_theme())
            .with_prompt("Proceed with upgrade?")
            .default(true)
            .interact()?
        {
            println!();
            println!("  {} Upgrade cancelled", "✗".yellow());
            println!();
            return Ok(());
        }
    } else if !yes {
        anyhow::bail!("Use --yes for non-interactive updates");
    }

    println!();
    print_update_header("Installing Updates");

    let mut installed_count = 0;

    if official_count > 0 {
        println!(
            "  {} Upgrading {} official package{}...",
            "→".cyan(),
            official_count,
            if official_count == 1 { "" } else { "s" }
        );
        pm.update().await?;
        installed_count += official_count;
    }

    #[cfg(feature = "arch")]
    if !aur_packages.is_empty() {
        println!();
        println!(
            "  {} Building {} AUR package{}...",
            "→".magenta(),
            aur_packages.len(),
            if aur_packages.len() == 1 { "" } else { "s" }
        );

        let client = crate::package_managers::AurClient::new();
        for pkg in &aur_packages {
            if let Err(e) = client.install(pkg).await {
                println!("  {} Failed to install {}: {}", "✗".red(), pkg, e);
            } else {
                installed_count += 1;
            }
        }
    }

    #[cfg(not(feature = "arch"))]
    let _ = &aur_packages;

    if installed_count > 0 {
        print_update_success(installed_count);
    } else {
        print_up_to_date();
    }
    Ok(())
}

fn print_update_header(title: &str) {
    use owo_colors::OwoColorize;

    println!();
    println!("  {}", "╭─────────────────────────────────────────╮".cyan());
    println!("  {} {:^39} {}", "│".cyan(), title.bold(), "│".cyan());
    println!("  {}", "╰─────────────────────────────────────────╯".cyan());
    println!();
}

fn print_up_to_date() {
    use owo_colors::OwoColorize;

    println!(
        "  {}",
        "╭─────────────────────────────────────────╮".green()
    );
    println!(
        "  {} {} {}",
        "│".green(),
        "  ✓ System is up to date!  ".bold().green(),
        "│".green()
    );
    println!(
        "  {}",
        "╰─────────────────────────────────────────╯".green()
    );
    println!();
}

fn print_update_summary(updates: &[UpdateInfo], official_count: usize, aur_count: usize) {
    use owo_colors::OwoColorize;

    let total = updates.len();

    println!(
        "  {}",
        "╭─────────────────────────────────────────╮".bright_yellow()
    );
    println!(
        "  {} {} {}",
        "│".bright_yellow(),
        format!(
            "  📦 {} update{} available  ",
            total,
            if total == 1 { "" } else { "s" }
        )
        .bold(),
        "│".bright_yellow()
    );
    println!(
        "  {}",
        "╰─────────────────────────────────────────╯".bright_yellow()
    );
    println!();

    if official_count > 0 {
        println!(
            "  {} {} from official repositories",
            "→".cyan(),
            official_count.to_string().bold()
        );
    }
    if aur_count > 0 {
        println!(
            "  {} {} from AUR",
            "→".magenta(),
            aur_count.to_string().bold()
        );
    }
    println!();

    let display_limit = 20;
    for update in updates.iter().take(display_limit) {
        let repo_badge = if update.repo == "AUR" {
            format!("{}", "(AUR)".magenta())
        } else {
            format!("{}", format!("({})", update.repo).dimmed())
        };

        println!(
            "    {} {} {} {} {} {}",
            "•".dimmed(),
            update.name.bold(),
            update.old_version.dimmed(),
            "→".cyan(),
            update.new_version.green(),
            repo_badge
        );
    }

    if updates.len() > display_limit {
        println!(
            "    {} {}",
            "…".dimmed(),
            format!("and {} more", updates.len() - display_limit).dimmed()
        );
    }
}

fn print_update_success(count: usize) {
    use owo_colors::OwoColorize;

    println!();
    println!(
        "  {}",
        "╭─────────────────────────────────────────╮".green()
    );
    println!(
        "  {} {} {}",
        "│".green(),
        "  ✓ Upgrade Complete!  ".bold().green(),
        "│".green()
    );
    println!(
        "  {}",
        "╰─────────────────────────────────────────╯".green()
    );
    println!();
    println!(
        "    {} {} package{} upgraded successfully",
        "✓".green().bold(),
        count.to_string().bold(),
        if count == 1 { "" } else { "s" }
    );
    println!();
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
