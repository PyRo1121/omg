use anyhow::{Context, Result};

use crate::cli::packages::common::try_daemon_list_updates;
use crate::cli::{modern_ui, style, ui};
use crate::package_managers::{get_package_manager, types::UpdateInfo};

/// Default AUR build concurrency when `OMG_AUR_PARALLEL` is unset.
const DEFAULT_AUR_BUILD_CONCURRENCY: usize = 2;

/// Hard bounds for `OMG_AUR_PARALLEL`: builds are process-heavy (makepkg
/// spawns compilers), so fan-out beyond a handful helps nobody and starves
/// the machine at the top end.
const MIN_AUR_BUILD_CONCURRENCY: usize = 1;
const MAX_AUR_BUILD_CONCURRENCY: usize = 8;

/// Parse the `OMG_AUR_PARALLEL` override, clamping to `[1, 8]` and warning on
/// invalid input instead of silently substituting the default.
fn aur_build_concurrency(raw: Option<&str>) -> usize {
    const fn clamp(value: usize) -> usize {
        if value < MIN_AUR_BUILD_CONCURRENCY {
            MIN_AUR_BUILD_CONCURRENCY
        } else if value > MAX_AUR_BUILD_CONCURRENCY {
            MAX_AUR_BUILD_CONCURRENCY
        } else {
            value
        }
    }

    let Some(raw) = raw else {
        return DEFAULT_AUR_BUILD_CONCURRENCY;
    };
    match raw.parse::<usize>() {
        Ok(parsed) if (MIN_AUR_BUILD_CONCURRENCY..=MAX_AUR_BUILD_CONCURRENCY).contains(&parsed) => {
            parsed
        }
        Ok(parsed) => {
            let clamped = clamp(parsed);
            tracing::warn!(
                "OMG_AUR_PARALLEL={parsed} is outside {MIN_AUR_BUILD_CONCURRENCY}..={MAX_AUR_BUILD_CONCURRENCY}; clamping to {clamped}"
            );
            clamped
        }
        Err(error) => {
            tracing::warn!(
                "OMG_AUR_PARALLEL={raw:?} is not a valid number ({error}); using {DEFAULT_AUR_BUILD_CONCURRENCY}"
            );
            DEFAULT_AUR_BUILD_CONCURRENCY
        }
    }
}

/// Number of repositories configured in pacman.conf, for honest sync
/// reporting; `0` means the configuration was unreadable and the count is
/// omitted rather than guessed.
fn configured_repo_count() -> usize {
    crate::core::pacman_conf::PacmanConfig::parse(crate::core::paths::pacman_conf_path())
        .map_or(0, |config| config.repos.len())
}

pub async fn update_fast() -> Result<()> {
    modern_ui::print_phase_header("⚡", "Fast System Update", "sync + upgrade");
    crate::package_managers::arch::run_privileged_operation(
        "fullupdate",
        &[],
        &[],
        run_full_sysupgrade,
    )
    .await
}

/// Refresh package databases before a root-side fast update.
async fn run_full_sysupgrade() -> anyhow::Result<()> {
    crate::package_managers::sync_databases_parallel().await?;
    run_sysupgrade().await
}

/// Execute a full system upgrade and record it in history.
///
/// Used by the privileged arms of `update --fast` / `--turbo`: previously
/// these closures were no-ops, so an already-privileged invocation claimed
/// success without upgrading anything.
async fn run_sysupgrade() -> anyhow::Result<()> {
    let updates = crate::package_managers::get_update_list()?;
    let changes = history_changes(&updates);
    let result = tokio::task::spawn_blocking(|| {
        crate::package_managers::execute_transaction(
            Vec::new(),
            crate::package_managers::TransactionKind::SystemUpgrade,
            None,
        )
    })
    .await
    .context("System upgrade task failed")?;

    let history = crate::core::history::HistoryManager::new()?;
    history.finish_operation(
        crate::core::history::TransactionType::Update,
        changes,
        result,
    )
}

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

    crate::package_managers::arch::run_privileged_operation("turboupdate", &[], &[], run_sysupgrade)
        .await
}

fn history_changes(updates: &[UpdateInfo]) -> Vec<crate::core::history::PackageChange> {
    updates
        .iter()
        .map(|update| crate::core::history::PackageChange {
            name: update.name.clone(),
            old_version: Some(update.old_version.clone()),
            new_version: Some(update.new_version.clone()),
            source: update.repo.clone(),
        })
        .collect()
}

/// Changes THIS process must record for an update operation.
///
/// Single-ownership rule: when the official upgrade was delegated to the
/// elevated child (deferred sync), the child records the official changes and
/// this process records only the AUR portion it builds itself. Otherwise this
/// process performed everything and records all changes.
fn parent_recorded_changes(
    all_updates: &[UpdateInfo],
    aur_packages: &[String],
    delegated_official: bool,
) -> Vec<crate::core::history::PackageChange> {
    if !delegated_official {
        return history_changes(all_updates);
    }
    history_changes(all_updates)
        .into_iter()
        .filter(|change| aur_packages.contains(&change.name))
        .collect()
}

pub async fn update(check_only: bool, yes: bool, dry_run: bool) -> Result<()> {
    use owo_colors::OwoColorize;

    let pm = get_package_manager()?;

    let skip_sync = check_only || dry_run || !crate::core::caps::can_write_pacman_db();
    let needs_deferred_sync = !check_only && !dry_run && !crate::core::caps::can_write_pacman_db();

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
            let sync_elapsed = sync_start.elapsed();
            let repo_count = configured_repo_count();
            let detail = if repo_count > 0 {
                format!(
                    "{repo_count} repositories in {:.2}s",
                    sync_elapsed.as_secs_f64()
                )
            } else {
                format!("in {:.2}s", sync_elapsed.as_secs_f64())
            };
            modern_ui::finish_success(&pb, "Synced", &detail);
        }
    }

    let mut all_updates: Vec<UpdateInfo> = Vec::with_capacity(32);
    let pb = modern_ui::modern_spinner("Checking", "official repositories");
    let check_start = std::time::Instant::now();
    let official_updates = match try_daemon_list_updates().await {
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

    let aur_packages = {
        if crate::core::paths::test_mode() || crate::core::env::distro::is_debian_like() {
            Vec::new()
        } else {
            let aur_pb = modern_ui::modern_spinner("Checking", "AUR packages");
            let aur_start = std::time::Instant::now();
            let client = crate::package_managers::AurClient::new()?;
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
                    packages
                }
                Err(error) => {
                    modern_ui::finish_clear(&aur_pb);
                    if official_count == 0 {
                        return Err(error).context("Failed to check AUR updates");
                    }
                    modern_ui::print_warning(&format!(
                        "AUR update check failed; continuing with official updates: {error:#}"
                    ));
                    Vec::new()
                }
            }
        }
    };

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
        if !tokio::task::spawn_blocking(|| {
            dialoguer::Confirm::with_theme(&ui::prompt_theme())
                .with_prompt("Proceed with upgrade?")
                .default(true)
                .interact()
        })
        .await
        .map_err(|error| anyhow::anyhow!("Confirmation prompt task failed: {error}"))??
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

    let history = crate::core::history::HistoryManager::new()?;
    let changes = parent_recorded_changes(
        &all_updates,
        &aur_packages,
        needs_deferred_sync && official_count > 0,
    );
    let mut installed_count = 0;
    let mut failed_count = 0;

    let operation_result: Result<()> = async {
        if official_count > 0 {
            if needs_deferred_sync {
                let pb = modern_ui::modern_spinner(
                    "Syncing & upgrading",
                    &format!("{official_count} official packages"),
                );
                crate::package_managers::arch::run_privileged_operation(
                    "fullupdate",
                    &[],
                    &[],
                    || async { Ok(()) },
                )
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
        }

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

            let client = Arc::new(crate::package_managers::AurClient::new()?);
            // AUR updates are reported per installed output, but split packages
            // share one PackageBase checkout and must be built together. Resolve
            // the package-base graph only after confirmation so check-only and
            // cancelled updates do not pay an extra network request.
            let jobs: Vec<BuildJob> = client.build_jobs_for_updates(&aur_packages).await?;
            let max_concurrent =
                aur_build_concurrency(std::env::var("OMG_AUR_PARALLEL").ok().as_deref());
            let builder = ParallelBuilder::new(client, max_concurrent);

            let build_summary = builder.build_packages(jobs).await?;
            installed_count += build_summary.succeeded_output_count();
            failed_count += build_summary.failed_output_count();
            if let Some(error) = build_summary.first_error() {
                println!("  {} Failed to build AUR packages: {error:#}", "✗".red());
            }
        }

        if failed_count > 0 {
            modern_ui::print_warning(&format!(
                "Upgraded {installed_count} packages, {failed_count} failed"
            ));
            anyhow::bail!("{failed_count} package(s) failed to update");
        }
        if installed_count > 0 {
            modern_ui::print_success(&format!("Upgraded {installed_count} packages"));
        } else {
            modern_ui::print_up_to_date();
        }
        Ok(())
    }
    .await;

    crate::core::usage::track_update_result(installed_count, operation_result.is_ok());
    history.finish_operation(
        crate::core::history::TransactionType::Update,
        changes,
        operation_result,
    )
}

fn update_dry_run(updates: &[UpdateInfo]) -> Result<()> {
    ui::print_header("OMG", "Dry Run - Update Preview");
    ui::print_spacer();
    println!(
        "  {} The following packages would be updated:\n",
        style::info("→")
    );

    let mut total_download: u64 = 0;
    for update in updates.iter().take(50) {
        let download_size = match crate::package_managers::get_sync_pkg_info(&update.name) {
            Ok(Some(info)) => {
                total_download += info.download_size.unwrap_or(0);
                format!(
                    "{:.2} MB",
                    info.download_size.unwrap_or(0) as f64 / 1024.0 / 1024.0
                )
            }
            Ok(None) => "unknown".to_string(),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to look up {} in official repositories", update.name)
                });
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
    let estimate_label = if updates.len() > 50 {
        "Estimated download (first 50)"
    } else {
        "Estimated download"
    };
    println!(
        "  {} {estimate_label}: {:.2} MB",
        style::info("→"),
        total_download as f64 / 1024.0 / 1024.0
    );
    ui::print_dry_run_footer();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn update_info(name: &str, repo: &str) -> UpdateInfo {
        UpdateInfo {
            name: name.to_string(),
            old_version: "1.0".to_string(),
            new_version: "2.0".to_string(),
            repo: repo.to_string(),
        }
    }

    #[test]
    fn delegated_official_updates_are_recorded_by_the_child_not_twice() {
        // Regression: with deferred elevation the child (fullupdate arm)
        // records official changes; the parent must record only its own AUR
        // portion or every official package appears twice in history.
        let all = vec![
            update_info("linux", "core"),
            update_info("firefox", "extra"),
            update_info("paru", "aur"),
        ];
        let aur = vec!["paru".to_string()];

        let recorded = parent_recorded_changes(&all, &aur, true);

        assert_eq!(recorded.len(), 1, "only the AUR change is parent-recorded");
        assert_eq!(recorded[0].name, "paru");
    }

    #[test]
    fn non_delegated_updates_record_every_change_once() {
        let all = vec![update_info("linux", "core"), update_info("paru", "aur")];
        let aur = vec!["paru".to_string()];

        let recorded = parent_recorded_changes(&all, &aur, false);

        assert_eq!(recorded.len(), 2);
    }

    #[test]
    fn aur_parallel_unset_uses_default() {
        assert_eq!(aur_build_concurrency(None), 2);
    }

    #[test]
    fn aur_parallel_in_range_is_honored() {
        assert_eq!(aur_build_concurrency(Some("4")), 4);
        assert_eq!(aur_build_concurrency(Some("1")), 1);
        assert_eq!(aur_build_concurrency(Some("8")), 8);
    }

    #[test]
    fn aur_parallel_out_of_range_is_clamped() {
        assert_eq!(aur_build_concurrency(Some("0")), 1);
        assert_eq!(aur_build_concurrency(Some("1000000")), 8);
    }

    #[test]
    fn aur_parallel_invalid_falls_back_to_default() {
        assert_eq!(aur_build_concurrency(Some("abc")), 2);
        assert_eq!(aur_build_concurrency(Some("")), 2);
        assert_eq!(aur_build_concurrency(Some("-2")), 2);
    }

    #[test]
    fn update_history_preserves_versions_and_sources() {
        let changes = history_changes(&[UpdateInfo {
            name: "example".to_string(),
            old_version: "1.0".to_string(),
            new_version: "2.0".to_string(),
            repo: "core".to_string(),
        }]);

        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].name, "example");
        assert_eq!(changes[0].old_version.as_deref(), Some("1.0"));
        assert_eq!(changes[0].new_version.as_deref(), Some("2.0"));
        assert_eq!(changes[0].source, "core");
    }
}
