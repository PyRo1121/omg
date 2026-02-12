use std::time::Instant;

use anyhow::Result;
use dialoguer::Confirm;

use crate::cli::{modern_ui, style, ui};
use crate::package_managers::{get_package_manager, types::UpdateInfo};

pub async fn update_fast() -> Result<()> {
    update(false, true, false).await
}

pub async fn update_turbo() -> Result<()> {
    update(false, true, false).await
}

pub async fn update(check_only: bool, yes: bool, dry_run: bool) -> Result<()> {
    use owo_colors::OwoColorize;

    let start_time = Instant::now();
    let pm = get_package_manager()?;
    let skip_sync = check_only || dry_run;

    if check_only || dry_run {
        modern_ui::print_phase_header(
            "🔄",
            "Update",
            if check_only {
                "Checking for updates (no sync)"
            } else {
                "Dry run - checking for updates"
            },
        );
    } else {
        modern_ui::print_phase_header("🔄", "Update", "Checking for updates");
        if !skip_sync {
            let pb = modern_ui::modern_spinner("Syncing", "package databases");
            let sync_start = std::time::Instant::now();
            pm.sync().await?;
            modern_ui::finish_success(
                &pb,
                "Synced",
                &format!("1 databases in {:.2}s", sync_start.elapsed().as_secs_f64()),
            );
        }
    }

    let pb = modern_ui::modern_spinner("Checking", "official repositories");
    let check_start = std::time::Instant::now();
    let official_updates = match try_daemon_list_updates().await {
        Some(updates) => updates,
        None => pm.list_updates().await?,
    };
    if official_updates.is_empty() {
        modern_ui::finish_info(&pb, "No updates in official repositories");
    } else {
        modern_ui::finish_success(
            &pb,
            "Found",
            &format!(
                "{} update(s) in {:.2}s",
                official_updates.len(),
                check_start.elapsed().as_secs_f64()
            ),
        );
    }

    println!();
    if official_updates.is_empty() {
        modern_ui::print_up_to_date();
        return Ok(());
    }

    if dry_run {
        return update_dry_run(&official_updates);
    }

    modern_ui::print_update_summary(&official_updates);

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
    modern_ui::print_section("Installing updates");

    let count = official_updates.len();
    let pb = modern_ui::modern_spinner("Upgrading", &format!("{count} official packages"));
    pm.update().await?;
    modern_ui::finish_success(&pb, "Upgraded", &format!("{count} packages"));

    let duration_ms = start_time.elapsed().as_millis() as u64;
    crate::core::usage::track_update_timed(count, duration_ms, true, None);
    modern_ui::print_success(&format!("Upgraded {count} packages"));
    Ok(())
}

fn update_dry_run(updates: &[UpdateInfo]) -> Result<()> {
    let names: Vec<String> = updates.iter().map(|update| update.name.clone()).collect();
    crate::core::security::validate_package_names(&names)?;

    ui::print_header("OMG", "Dry Run - Update Preview");
    ui::print_spacer();
    println!(
        "  {} The following packages would be updated:\n",
        style::info("→")
    );

    for update in updates.iter().take(50) {
        println!(
            "    {} {} {} {} {} ({})",
            style::success("↑"),
            style::package(&update.name),
            style::dim(&update.old_version),
            style::arrow("->"),
            style::version(&update.new_version),
            style::dim("unknown")
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
    println!("  {} Estimated download: unknown", style::info("→"));
    ui::print_dry_run_footer();
    Ok(())
}

#[cfg(unix)]
async fn try_daemon_list_updates() -> Option<Vec<UpdateInfo>> {
    let mut client = crate::core::client::DaemonClient::connect().await.ok()?;
    let entries = client.list_updates().await.ok()?;
    Some(
        entries
            .into_iter()
            .map(|e| UpdateInfo {
                name: e.name,
                old_version: e.old_version,
                new_version: e.new_version,
                repo: e.repo,
            })
            .collect(),
    )
}

#[cfg(not(unix))]
#[allow(clippy::unused_async)]
async fn try_daemon_list_updates() -> Option<Vec<UpdateInfo>> {
    None
}
