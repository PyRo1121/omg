//! Update functionality for packages

use std::time::Instant;

use anyhow::Result;
use dialoguer::Confirm;

use crate::cli::{modern_ui, style, ui};
use crate::package_managers::{get_package_manager, types::UpdateInfo};

/// Fast update - sync + upgrade in a single privileged operation
/// This is the fastest way to update your system
#[cfg(feature = "arch")]
pub async fn update_fast() -> Result<()> {
    modern_ui::print_phase_header("⚡", "Fast System Update", "sync + upgrade");

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

    modern_ui::print_phase_header("🚀", "TURBO System Update", "cached, no sync");

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

    // Start timing
    let start_time = Instant::now();

    let pm = get_package_manager()?;

    // On Arch, sync requires root so we defer it until we actually upgrade.
    // For check/dry-run modes, we use the existing (possibly stale) sync databases.
    // For the actual upgrade, we combine sync + upgrade into a single privileged call.
    #[cfg(feature = "arch")]
    let skip_sync = check_only || dry_run || !crate::core::caps::can_write_pacman_db();
    #[cfg(feature = "arch")]
    let needs_deferred_sync = !check_only && !dry_run && !crate::core::caps::can_write_pacman_db();

    // Non-arch platforms: sync doesn't need root
    #[cfg(not(feature = "arch"))]
    let skip_sync = check_only || dry_run;
    #[cfg(not(feature = "arch"))]
    #[allow(unused_variables)]
    let needs_deferred_sync = false;

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
    } else if skip_sync {
        // Arch without root: skip sync here, will sync+upgrade together later
        modern_ui::print_phase_header("🔄", "Update", "Checking for updates");
    } else {
        modern_ui::print_phase_header("🔄", "Update", "Checking for updates");

        let pb = modern_ui::modern_spinner("Syncing", "package databases");
        let sync_start = std::time::Instant::now();
        pm.sync().await?;
        let sync_elapsed = sync_start.elapsed();

        modern_ui::finish_success(
            &pb,
            "Synced",
            &format!(
                "{} databases in {:.2}s",
                if cfg!(feature = "arch") { "3" } else { "1" },
                sync_elapsed.as_secs_f64()
            ),
        );
    }

    let mut all_updates: Vec<UpdateInfo> = Vec::with_capacity(32); // Typical update count

    let pb = modern_ui::modern_spinner("Checking", "official repositories");
    let check_start = std::time::Instant::now();
    // Try daemon IPC first (hot ALPM worker = ~5ms), fall back to direct ALPM (~150ms)
    let official_updates: Vec<UpdateInfo> = match try_daemon_list_updates().await {
        Some(updates) => updates,
        None => pm.list_updates().await?,
    };
    let check_elapsed = check_start.elapsed();

    if official_updates.is_empty() {
        modern_ui::finish_info(&pb, "No updates in official repositories");
    } else {
        modern_ui::finish_success(
            &pb,
            "Found",
            &format!(
                "{} update(s) in {:.2}s",
                official_updates.len(),
                check_elapsed.as_secs_f64()
            ),
        );
    }

    let official_count = official_updates.len();
    all_updates.extend(official_updates);

    #[cfg(feature = "arch")]
    let (_aur_count, aur_packages) = {
        use crate::core::env::distro::use_debian_backend;
        if use_debian_backend() {
            (0, Vec::new())
        } else {
            let aur_pb = modern_ui::modern_spinner("Checking", "AUR packages");
            let aur_start = std::time::Instant::now();
            let client = crate::package_managers::AurClient::new();
            match client.get_update_list().await {
                Ok(aur_updates) => {
                    let aur_elapsed = aur_start.elapsed();
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
                    if count == 0 {
                        modern_ui::finish_info(&aur_pb, "No updates in AUR");
                    } else {
                        modern_ui::finish_success(
                            &aur_pb,
                            "Found",
                            &format!(
                                "{} AUR update(s) in {:.2}s",
                                count,
                                aur_elapsed.as_secs_f64()
                            ),
                        );
                    }
                    (count, packages)
                }
                Err(e) => {
                    modern_ui::finish_clear(&aur_pb);
                    tracing::warn!("Failed to check AUR updates: {}", e);
                    (0, Vec::new())
                }
            }
        }
    };

    #[cfg(not(feature = "arch"))]
    let (_aur_count, aur_packages): (usize, Vec<String>) = (0, Vec::new());

    println!();

    if all_updates.is_empty() {
        modern_ui::print_up_to_date();
        return Ok(());
    }

    if dry_run {
        return update_dry_run(&all_updates);
    }

    modern_ui::print_update_summary(&all_updates);

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

    let mut installed_count = 0;
    #[cfg_attr(not(feature = "arch"), allow(unused_mut))]
    let mut failed_count = 0;

    if official_count > 0 {
        #[cfg(feature = "arch")]
        if needs_deferred_sync {
            // Combine sync + upgrade in a single privileged operation
            let pb = modern_ui::modern_spinner(
                "Syncing & upgrading",
                &format!("{official_count} official packages"),
            );
            crate::package_managers::arch::run_privileged_operation("fullupdate", &[], || async {
                Ok(())
            })
            .await?;
            modern_ui::finish_success(
                &pb,
                "Synced & upgraded",
                &format!("{official_count} packages"),
            );
            installed_count += official_count;
        } else {
            let pb = modern_ui::modern_spinner(
                "Upgrading",
                &format!("{official_count} official packages"),
            );
            pm.update().await?;
            modern_ui::finish_success(&pb, "Upgraded", &format!("{official_count} packages"));
            installed_count += official_count;
        }

        #[cfg(not(feature = "arch"))]
        {
            let pb = modern_ui::modern_spinner(
                "Upgrading",
                &format!("{official_count} official packages"),
            );
            pm.update().await?;
            modern_ui::finish_success(&pb, "Upgraded", &format!("{official_count} packages"));
            installed_count += official_count;
        }
    }

    #[cfg(feature = "arch")]
    if !aur_packages.is_empty() {
        println!();
        println!(
            "  {} Building {} AUR package{} in parallel...",
            "→".magenta(),
            aur_packages.len(),
            if aur_packages.len() == 1 { "" } else { "s" }
        );

        use crate::package_managers::aur::{BuildJob, ParallelBuilder};
        use std::sync::Arc;

        let client = Arc::new(crate::package_managers::AurClient::new());

        let jobs: Vec<BuildJob> = aur_packages
            .iter()
            .map(|pkg| BuildJob::new(pkg.clone(), Vec::new()))
            .collect();

        let max_concurrent = std::env::var("OMG_AUR_PARALLEL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        let builder = ParallelBuilder::new(client, max_concurrent);

        if let Err(e) = builder.build_packages(jobs).await {
            println!("  {} Failed to build AUR packages: {}", "✗".red(), e);
            failed_count += aur_packages.len();
        } else {
            installed_count += aur_packages.len();
        }
    }

    #[cfg(not(feature = "arch"))]
    let _ = &aur_packages;

    // Track update with timing
    let duration_ms = start_time.elapsed().as_millis() as u64;
    let success = failed_count == 0;
    let error_msg = if failed_count > 0 {
        Some(format!("{failed_count} packages failed to update"))
    } else {
        None
    };
    crate::core::usage::track_update_timed(
        installed_count,
        duration_ms,
        success,
        error_msg.as_deref(),
    );

    if failed_count > 0 {
        modern_ui::print_warning(&format!(
            "Upgraded {installed_count} packages, {failed_count} failed"
        ));
    } else if installed_count > 0 {
        modern_ui::print_success(&format!("Upgraded {installed_count} packages"));
    } else {
        modern_ui::print_up_to_date();
    }
    Ok(())
}

// Removed old box-drawing functions - using modern_ui now

#[expect(clippy::unnecessary_wraps)] // Result return required: API compat with feature-gated impls
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

/// Try to get update list from daemon IPC (fast path, ~5ms with hot ALPM worker)
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
