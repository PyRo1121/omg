//! Clean/orphan functionality for packages

use anyhow::{Context, Result};

use super::common::use_debian_backend;

#[cfg(feature = "arch")]
use crate::package_managers::{AurClient, clean_cache, list_orphans_direct, remove_orphans};

#[cfg(feature = "debian")]
use crate::package_managers::apt_remove_orphans;

#[cfg(feature = "debian-pure")]
use crate::package_managers::debian_db::{clean_package_cache, list_orphans_fast};

#[cfg(not(feature = "arch"))]
use crate::cli::style;

/// Clean up orphans and caches
#[expect(clippy::fn_params_excessive_bools)] // Parameters map directly to CLI boolean flags (orphans, cache, aur, all, dry_run)
pub async fn clean(orphans: bool, cache: bool, aur: bool, all: bool, dry_run: bool) -> Result<()> {
    if dry_run {
        crate::cli::modern_ui::print_phase_header("🧹", "Clean Preview", "dry run");
    } else {
        crate::cli::modern_ui::print_phase_header("🧹", "Clean", "removing orphans and cache");
    }
    println!();

    if use_debian_backend() {
        #[cfg(feature = "debian-pure")]
        {
            return handle_debian_pure_clean(orphans, cache, all, dry_run).await;
        }

        #[cfg(all(feature = "debian", not(feature = "debian-pure")))]
        {
            let do_orphans = orphans || all;
            if do_orphans {
                apt_remove_orphans()?;
            }
            if cache || aur {
                println!(
                    "{} Cache/AUR cleanup is not supported on APT yet",
                    style::warning("→")
                );
            }
            return Ok(());
        }
    }

    let do_orphans = orphans || all;
    let do_cache = cache || all;
    let do_aur = aur || all;

    if !do_orphans && !do_cache && !do_aur {
        // Default: show what can be cleaned
        #[cfg(feature = "arch")]
        {
            let orphan_list = list_orphans_direct().context("Failed to list orphan packages")?;
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
        #[cfg(not(feature = "arch"))]
        {
            println!(
                "{}",
                style::info("Orphan removal not available on this system")
            );
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
        #[cfg(feature = "debian")]
        {
            use owo_colors::OwoColorize;
            println!(
                "  {} Use 'apt clean' for cache cleanup on Debian",
                "ℹ".blue()
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
                let aur_client = AurClient::new();
                aur_client.clean_all()?;
            }
        }
        #[cfg(not(feature = "arch"))]
        {
            use owo_colors::OwoColorize;
            println!("  {} AUR cleanup not available on this system", "ℹ".blue());
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

/// Handle clean operations for debian-pure backend
#[cfg(feature = "debian-pure")]
#[allow(clippy::fn_params_excessive_bools)]
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
        let orphan_list = list_orphans_fast().unwrap_or_default();
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
        let orphan_list = list_orphans_fast().unwrap_or_default();

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

            match clean_package_cache() {
                Ok((removed, freed)) => {
                    println!(
                        "  {} Removed {} files, freed {:.2} MB",
                        "✓".green().bold(),
                        removed,
                        freed as f64 / 1024.0 / 1024.0
                    );
                }
                Err(e) => {
                    crate::cli::modern_ui::print_error(&format!("Failed to clear cache: {e}"));
                }
            }
        }
    }

    Ok(())
}
