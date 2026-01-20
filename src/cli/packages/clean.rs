//! Clean/orphan functionality for packages

use anyhow::Result;

use crate::cli::style;

use super::common::use_debian_backend;

#[cfg(feature = "arch")]
use crate::package_managers::{AurClient, clean_cache, list_orphans_direct, remove_orphans};

#[cfg(feature = "debian")]
use crate::package_managers::apt_remove_orphans;

/// Clean up orphans and caches
#[allow(clippy::fn_params_excessive_bools)]
pub async fn clean(orphans: bool, cache: bool, aur: bool, all: bool) -> Result<()> {
    println!("{} Cleaning up...\n", style::header("OMG"));

    if use_debian_backend() {
        #[cfg(feature = "debian")]
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
            let orphan_list = list_orphans_direct().unwrap_or_default();
            if !orphan_list.is_empty() {
                println!(
                    "{} {} orphan packages can be removed",
                    style::arrow("→"),
                    orphan_list.len()
                );
                println!("  Run: {}", style::command("omg clean --orphans"));
            }
        }

        println!(
            "{} To clear package cache: {}",
            style::arrow("→"),
            style::command("omg clean --cache")
        );
        #[cfg(feature = "arch")]
        println!(
            "{} To clear AUR builds: {}",
            style::arrow("→"),
            style::command("omg clean --aur")
        );
        println!(
            "{} To clean everything: {}",
            style::arrow("→"),
            style::command("omg clean --all")
        );
        return Ok(());
    }

    if do_orphans {
        #[cfg(feature = "arch")]
        {
            remove_orphans().await?;
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
        println!("{}", style::info("Clearing package cache..."));
        #[cfg(feature = "arch")]
        match clean_cache(1) {
            // Keep 1 version by default
            Ok((removed, freed)) => {
                println!(
                    "{} Removed {} files, freed {:.2} MB",
                    style::success("✓"),
                    removed,
                    freed as f64 / 1024.0 / 1024.0
                );
            }
            Err(e) => {
                println!("{}", style::error(&format!("Failed to clear cache: {e}")));
            }
        }
        #[cfg(feature = "debian")]
        println!(
            "{}",
            style::info("Use 'apt clean' for cache cleanup on Debian")
        );
    }

    if do_aur {
        #[cfg(feature = "arch")]
        {
            let aur_client = AurClient::new();
            aur_client.clean_all()?;
        }
        #[cfg(not(feature = "arch"))]
        println!(
            "{}",
            style::info("AUR cleanup not available on this system")
        );
    }

    println!("\n{}", style::success("Cleanup complete!"));
    Ok(())
}
