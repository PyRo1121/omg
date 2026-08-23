//! Clean/orphan functionality for packages

#[cfg(any(feature = "arch", feature = "debian-pure"))]
use anyhow::Context;
use anyhow::Result;

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use crate::core::env::distro::is_debian_like;

#[cfg(feature = "arch")]
use crate::package_managers::{AurClient, clean_cache, list_orphans_direct, remove_orphans};

#[cfg(feature = "debian")]
use crate::package_managers::apt_remove_orphans;

#[cfg(feature = "debian-pure")]
use crate::package_managers::debian_db::{clean_package_cache, list_orphans_fast};

/// Clean up orphans and caches
#[expect(clippy::fn_params_excessive_bools)] // Parameters map directly to CLI boolean flags (orphans, cache, aur, all, dry_run)
#[allow(
    clippy::needless_return,
    reason = "additive backend feature branches return before compiled fallbacks"
)]
pub async fn clean(orphans: bool, cache: bool, aur: bool, all: bool, dry_run: bool) -> Result<()> {
    if dry_run {
        crate::cli::modern_ui::print_phase_header("🧹", "Clean Preview", "dry run");
    } else {
        crate::cli::modern_ui::print_phase_header("🧹", "Clean", "removing orphans and cache");
    }
    println!();

    // AUR cleanup needs an Arch-style package database. On Debian-like hosts
    // the Debian backends own routing below and cannot serve AUR, so fail
    // explicitly instead of silently ignoring the flag — including when the
    // Arch backend is compiled in for use on other hosts.
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if aur && is_debian_like() {
        anyhow::bail!("AUR cleanup is not available on Debian-like systems");
    }

    // Without any Arch backend at all there is no AUR to clean anywhere.
    #[cfg(all(
        any(feature = "debian", feature = "debian-pure"),
        not(feature = "arch")
    ))]
    if aur {
        anyhow::bail!("AUR cleanup is not available without the Arch backend");
    }

    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if is_debian_like() {
        #[cfg(feature = "debian-pure")]
        {
            return handle_debian_pure_clean(orphans, cache, all, dry_run).await;
        }

        #[cfg(all(feature = "debian", not(feature = "debian-pure")))]
        {
            if cache || aur {
                anyhow::bail!("Cache and AUR cleanup are not supported on the APT backend");
            }
            let do_orphans = orphans || all;
            if !do_orphans {
                println!(
                    "  {} To remove orphan packages: {}",
                    "·".dimmed(),
                    "omg clean --orphans".cyan()
                );
                println!();
                return Ok(());
            }
            if dry_run {
                // Dry run must never mutate: report candidates from the FFI
                // status snapshot instead of invoking apt_remove_orphans.
                let orphan_count =
                    tokio::task::spawn_blocking(crate::package_managers::apt_get_system_status)
                        .await
                        .context("APT status task failed")?
                        .map(|(_, _, orphan_count, _)| orphan_count)
                        .context("Failed to inspect orphan packages")?;
                if orphan_count == 0 {
                    println!("  {} No orphan packages found", "✓".green().bold());
                } else {
                    println!(
                        "  {} Would remove {orphan_count} orphan packages",
                        "→".cyan()
                    );
                    println!(
                        "    Run: {} without --dry-run to apply",
                        "omg clean --orphans".cyan()
                    );
                }
                println!();
                println!("  {} No changes made (dry run)", "ℹ".blue().dimmed());
                return Ok(());
            }
            tokio::task::spawn_blocking(crate::package_managers::apt_remove_orphans)
                .await
                .context("APT orphan removal task failed")??;
            return Ok(());
        }
    }

    // Compiled with only the Debian backends but running somewhere that is
    // not Debian-like: there is no Debian package database here to clean.
    // (With the Arch backend also compiled in, execution continues into the
    // Arch-capable block below instead.)
    #[cfg(all(feature = "debian-pure", not(feature = "arch")))]
    {
        anyhow::bail!(
            "Clean requires a Debian-like system (or an Arch-enabled build); \
             no supported package database was found"
        );
    }

    #[cfg(any(feature = "arch", not(feature = "debian-pure")))]
    {
        let do_orphans = orphans || all;
        let do_cache = cache || all;
        let do_aur = aur || all;

        if !do_orphans && !do_cache && !do_aur {
            // Default: show what can be cleaned
            #[cfg(feature = "arch")]
            {
                let orphan_list =
                    list_orphans_direct().context("Failed to list orphan packages")?;
                if !orphan_list.is_empty() {
                    use owo_colors::OwoColorize;
                    println!(
                        "  {} {} orphan packages can be removed",
                        "·".dimmed(),
                        orphan_list.len().to_string().yellow()
                    );
                    println!("    Run: {}", "omg clean --orphans".cyan());
                }
            }

            use owo_colors::OwoColorize;
            println!(
                "  {} To clear package cache: {}",
                "·".dimmed(),
                "omg clean --cache".cyan()
            );
            #[cfg(feature = "arch")]
            println!(
                "  {} To clear AUR builds: {}",
                "·".dimmed(),
                "omg clean --aur".cyan()
            );
            println!(
                "  {} To clean everything: {}",
                "·".dimmed(),
                "omg clean --all".cyan()
            );
            println!();
            return Ok(());
        }

        if do_orphans {
            #[cfg(feature = "arch")]
            {
                if dry_run {
                    let orphan_list =
                        list_orphans_direct().context("Failed to list orphan packages")?;
                    use owo_colors::OwoColorize;
                    println!(
                        "  {} Would remove {} orphan packages:",
                        "→".cyan(),
                        orphan_list.len()
                    );
                    for pkg in orphan_list.iter().take(10) {
                        println!("    {} {}", "·".dimmed(), pkg);
                    }
                    if orphan_list.len() > 10 {
                        println!(
                            "    {} and {} more...",
                            "·".dimmed(),
                            orphan_list.len() - 10
                        );
                    }
                } else {
                    remove_orphans().await?;
                }
            }
            #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
            {
                anyhow::bail!(
                    "Orphan removal is not available without an Arch or Debian package backend"
                );
            }
            #[cfg(all(
                feature = "debian",
                not(feature = "arch"),
                not(feature = "debian-pure")
            ))]
            {
                apt_remove_orphans()?;
            }
        }

        if do_cache {
            #[cfg(feature = "arch")]
            {
                if dry_run {
                    use owo_colors::OwoColorize;
                    println!(
                        "  {} Would clear package cache (keep 1 recent version)",
                        "→".cyan()
                    );
                } else {
                    crate::cli::modern_ui::print_info("Clearing package cache...");
                    // Warn (do not block): cleaning can delete exactly the
                    // cached versions that update/removal rollback plans from
                    // the last 30 days depend on.
                    match crate::core::history::HistoryManager::new()
                        .and_then(|history| history.rollback_referenced_versions(30))
                    {
                        Ok(referenced) if !referenced.is_empty() => {
                            use owo_colors::OwoColorize;
                            println!(
                                "  {} Cleaning will remove cached versions referenced by recent rollback plans:",
                                "⚠".yellow().bold()
                            );
                            for (name, version) in &referenced {
                                println!("    - {name} {version}");
                            }
                            println!("  After this, 'omg rollback' cannot restore those versions.");
                        }
                        Ok(_) => {}
                        Err(history_error) => {
                            tracing::debug!(
                                "Could not check history for rollback-referenced versions: {history_error}"
                            );
                        }
                    }
                    match clean_cache(1) {
                        // Keep 1 version by default
                        Ok((removed, freed)) => {
                            use owo_colors::OwoColorize;
                            println!(
                                "  {} Removed {} files, freed {:.2} MB",
                                "✓".green().bold(),
                                removed,
                                freed as f64 / 1024.0 / 1024.0
                            );
                        }
                        Err(e) => {
                            return Err(e).context("Failed to clear package cache");
                        }
                    }
                }
            }
            #[cfg(all(feature = "debian", not(feature = "arch")))]
            {
                anyhow::bail!("Package cache cleanup is not supported on the APT backend");
            }
            #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
            {
                anyhow::bail!(
                    "Package cache cleanup is not available without a package manager backend"
                );
            }
        }

        if do_aur {
            #[cfg(feature = "arch")]
            {
                if dry_run {
                    use owo_colors::OwoColorize;
                    println!("  {} Would clean all AUR build directories", "→".cyan());
                } else {
                    let aur_client = AurClient::new()?;
                    aur_client.clean_all()?;
                }
            }
            #[cfg(not(feature = "arch"))]
            {
                anyhow::bail!("AUR cleanup is not available without the Arch backend");
            }
        }

        if dry_run {
            println!();
            use owo_colors::OwoColorize;
            println!("  {} No changes made (dry run)", "ℹ".blue().dimmed());
            println!();
        } else {
            crate::cli::modern_ui::print_success("Cleanup complete!");
        }
        Ok(())
    }
}

/// Handle clean operations for debian-pure backend
#[cfg(feature = "debian-pure")]
#[expect(clippy::fn_params_excessive_bools)]
async fn handle_debian_pure_clean(
    orphans: bool,
    cache: bool,
    all: bool,
    dry_run: bool,
) -> Result<()> {
    use owo_colors::OwoColorize;

    let do_orphans = orphans || all;
    let do_cache = cache || all;

    // Default behavior: show what can be cleaned
    if !do_orphans && !do_cache {
        let orphan_list = list_orphans_fast().context("Failed to list orphan packages")?;
        if !orphan_list.is_empty() {
            println!(
                "  {} {} orphan packages can be removed",
                "·".dimmed(),
                orphan_list.len().to_string().yellow()
            );
            println!("    Run: {}", "omg clean --orphans".cyan());
        }

        println!(
            "  {} To clear package cache: {}",
            "·".dimmed(),
            "omg clean --cache".cyan()
        );
        println!(
            "  {} To clean everything: {}",
            "·".dimmed(),
            "omg clean --all".cyan()
        );
        println!();
        return Ok(());
    }

    // Handle orphan removal
    if do_orphans {
        let orphan_list = list_orphans_fast().context("Failed to list orphan packages")?;

        if orphan_list.is_empty() {
            println!("  {} No orphan packages found", "✓".green().bold());
        } else if dry_run {
            println!(
                "  {} Would remove {} orphan packages:",
                "→".cyan(),
                orphan_list.len()
            );
            for pkg in orphan_list.iter().take(10) {
                println!("    {} {}", "·".dimmed(), pkg);
            }
            if orphan_list.len() > 10 {
                println!(
                    "    {} and {} more...",
                    "·".dimmed(),
                    orphan_list.len() - 10
                );
            }
        } else {
            crate::cli::modern_ui::print_info(&format!(
                "Removing {} orphan packages...",
                orphan_list.len()
            ));

            // Use the package manager's remove function
            let pm = crate::package_managers::get_package_manager()?;
            pm.remove(&orphan_list).await?;

            println!(
                "  {} Removed {} orphan packages",
                "✓".green().bold(),
                orphan_list.len()
            );
        }
    }

    // Handle cache cleanup
    if do_cache {
        if dry_run {
            println!("  {} Would clear APT package cache", "→".cyan());
        } else {
            crate::cli::modern_ui::print_info("Clearing package cache...");

            report_cache_clean(clean_package_cache())?;
        }
    }

    Ok(())
}

#[cfg(any(test, feature = "debian-pure"))]
fn report_cache_clean(result: Result<(usize, u64)>) -> Result<()> {
    match result {
        Ok((removed, freed)) => {
            use owo_colors::OwoColorize;
            println!(
                "  {} Removed {} files, freed {:.2} MB",
                "✓".green().bold(),
                removed,
                freed as f64 / 1024.0 / 1024.0
            );
            Ok(())
        }
        Err(e) => {
            crate::cli::modern_ui::print_error(&format!("Failed to clear cache: {e}"));
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_clean_failure_returns_err() {
        let result = report_cache_clean(Err(anyhow::anyhow!("permission denied")));
        assert!(
            result.is_err(),
            "failed cache cleanup must be a CLI error so the process exits non-zero"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("permission denied"),
            "original cache cleanup error must be preserved"
        );
    }

    #[test]
    fn cache_clean_success_returns_ok() {
        assert!(report_cache_clean(Ok((3, 4096))).is_ok());
    }

    #[tokio::test]
    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    async fn clean_orphans_without_backend_fails() {
        let error = clean(true, false, false, false, false)
            .await
            .expect_err("orphan removal with no backend must not look like success");
        assert!(
            error
                .to_string()
                .contains("not available without an Arch or Debian package backend")
        );
    }

    #[tokio::test]
    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    async fn clean_cache_without_backend_fails() {
        let error = clean(false, true, false, false, false)
            .await
            .expect_err("cache cleanup with no backend must not look like success");
        assert!(
            error
                .to_string()
                .contains("not available without a package manager backend")
        );
    }

    #[tokio::test]
    #[cfg(not(feature = "arch"))]
    async fn clean_aur_without_arch_fails() {
        let error = clean(false, false, true, false, false)
            .await
            .expect_err("AUR cleanup without the Arch backend must not look like success");
        assert!(
            error
                .to_string()
                .contains("AUR cleanup is not available without the Arch backend")
        );
    }
}
