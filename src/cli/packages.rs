use anyhow::Result;
use colored::Colorize;

use crate::core::client::DaemonClient;
use crate::core::security::SecurityPolicy;
use crate::package_managers::{
    clean_cache,
    display_pkg_info,
    get_sync_pkg_info,
    get_update_list,
    list_explicit,
    list_orphans_direct,
    remove_orphans,
    search_detailed,
    // Direct ALPM functions (10-100x faster)
    search_sync,
    ArchPackageManager,
    AurClient,
    PackageManager,
};

/// Search for packages in official repos and AUR - LIGHTNING FAST
pub async fn search(query: &str, detailed: bool) -> Result<()> {
    let start = std::time::Instant::now();

    // FAST: Direct libalpm query instead of spawning pacman
    let official_packages = search_sync(query).unwrap_or_default();

    let sync_time = start.elapsed();

    // Display official packages first
    if !official_packages.is_empty() {
        println!(
            "{} {} results ({:.1}ms)\n",
            "OMG".cyan().bold(),
            official_packages.len(),
            sync_time.as_secs_f64() * 1000.0
        );

        println!("{}", "Official Repositories:".green().bold());
        for pkg in official_packages.iter().take(20) {
            let installed = if pkg.installed {
                " [installed]".dimmed()
            } else {
                "".into()
            };
            println!(
                "  {} {} ({}) - {}{}",
                pkg.name.white().bold(),
                pkg.version.dimmed(),
                pkg.repo.cyan(),
                truncate(&pkg.description, 50).dimmed(),
                installed
            );
        }
        if official_packages.len() > 20 {
            println!(
                "  {} more packages...",
                format!("(+{})", official_packages.len() - 20).dimmed()
            );
        }
        println!();
    }

    // Search AUR (network call - still async)
    if detailed {
        let aur_packages = search_detailed(query).await.unwrap_or_default();
        if !aur_packages.is_empty() {
            println!("{}", "AUR (Arch User Repository):".yellow().bold());
            for pkg in aur_packages.iter().take(10) {
                let out_of_date = if pkg.out_of_date.is_some() {
                    " [OUT OF DATE]".red()
                } else {
                    "".into()
                };
                println!(
                    "  {} {} - {} {} {}{}",
                    pkg.name.white().bold(),
                    pkg.version.dimmed(),
                    format!("↑{}", pkg.num_votes).blue(),
                    format!("{:.1}%", pkg.popularity).cyan(),
                    truncate(&pkg.description.clone().unwrap_or_default(), 40).dimmed(),
                    out_of_date
                );
            }
            if aur_packages.len() > 10 {
                println!(
                    "  {} more packages...",
                    format!("(+{})", aur_packages.len() - 10).dimmed()
                );
            }
            println!();
        }
    } else {
        let aur = AurClient::new();
        let aur_packages = aur.search(query).await.unwrap_or_default();
        if !aur_packages.is_empty() {
            println!("{}", "AUR (Arch User Repository):".yellow().bold());
            for pkg in aur_packages.iter().take(10) {
                println!(
                    "  {} {} - {}",
                    pkg.name.white().bold(),
                    pkg.version.dimmed(),
                    truncate(&pkg.description, 55).dimmed()
                );
            }
            if aur_packages.len() > 10 {
                println!(
                    "  {} more packages...",
                    format!("(+{})", aur_packages.len() - 10).dimmed()
                );
            }
            println!();
        }
    }

    println!("{} Use 'omg info <package>' for details", "→".dimmed());

    Ok(())
}

/// Install packages (auto-detects AUR) with Graded Security
pub async fn install(packages: &[String], _yes: bool) -> Result<()> {
    if packages.is_empty() {
        println!("{} No packages specified", "✗".red());
        return Ok(());
    }

    let policy = SecurityPolicy::load_default().unwrap_or_default();

    println!(
        "{} Analyzing {} package(s) with {} model...\n",
        "OMG".cyan().bold(),
        packages.len(),
        "Graded Security".green()
    );

    let aur = AurClient::new();
    let mut official = Vec::new();
    let mut aur_pkgs = Vec::new();
    let mut not_found = Vec::new();

    for pkg_name in packages {
        let (grade, is_aur, is_official, license) =
            if let Some(info) = get_sync_pkg_info(pkg_name).ok().flatten() {
                let grade = policy
                    .assign_grade(&info.name, &info.version, false, true)
                    .await;
                let license = info.licenses.first().cloned();
                (grade, false, true, license)
            } else if let Ok(Some(info)) = aur.info(pkg_name).await {
                let grade = policy
                    .assign_grade(&info.name, &info.version, true, false)
                    .await;
                (grade, true, false, None)
            } else {
                not_found.push(pkg_name.clone());
                continue;
            };

        // Check policy
        if let Err(e) = policy.check_package(pkg_name, is_aur, license.as_deref(), grade) {
            println!("{} Security Block: {}", "✗".red().bold(), e);
            anyhow::bail!("Installation aborted due to security policy");
        }

        let grade_colored = match grade {
            crate::core::security::SecurityGrade::Locked => grade.to_string().green().bold(),
            crate::core::security::SecurityGrade::Verified => grade.to_string().cyan(),
            crate::core::security::SecurityGrade::Community => grade.to_string().yellow(),
            crate::core::security::SecurityGrade::Risk => grade.to_string().red().blink(),
        };

        if is_official {
            official.push(pkg_name.clone());
            println!(
                "{} Official: {} [{}]",
                "→".blue(),
                pkg_name.white().bold(),
                grade_colored
            );
        } else if is_aur {
            aur_pkgs.push(pkg_name.clone());
            println!(
                "{} AUR:      {} [{}]",
                "→".yellow(),
                pkg_name.white().bold(),
                grade_colored
            );
        }
    }

    if !not_found.is_empty() {
        println!("{} Not found: {}", "✗".red(), not_found.join(", "));
    }
    println!();

    // Install official packages
    if !official.is_empty() {
        let pacman = ArchPackageManager::new();
        pacman.install(&official).await?;
    }

    // Install AUR packages
    for pkg in &aur_pkgs {
        aur.install(pkg).await?;
    }

    if official.is_empty() && aur_pkgs.is_empty() {
        println!("{} No packages to install", "→".dimmed());
    }

    Ok(())
}

/// Remove packages
pub async fn remove(packages: &[String], recursive: bool) -> Result<()> {
    if packages.is_empty() {
        println!("{} No packages specified", "✗".red());
        return Ok(());
    }

    let pacman = ArchPackageManager::new();

    if recursive {
        // Remove with dependencies: -Rs
        println!(
            "{} Removing with unused dependencies...\n",
            "OMG".cyan().bold()
        );
    }

    pacman.remove(packages).await?;

    Ok(())
}

/// Update all packages
pub async fn update(check_only: bool) -> Result<()> {
    if check_only {
        println!("{} Checking for updates...\n", "OMG".cyan().bold());

        // LIGHTNING FAST: Direct ALPM update check (no subprocess)
        let update_list = get_update_list().unwrap_or_default();

        if update_list.is_empty() {
            println!("{} System is up to date!", "✓".green());
        } else {
            println!("{} {} updates available:\n", "→".blue(), update_list.len());
            for (name, old_ver, new_ver) in &update_list {
                // Determine update type (Major/Minor/Patch)
                let update_label = match (
                    semver::Version::parse(old_ver.trim_start_matches(|c: char| !c.is_numeric())),
                    semver::Version::parse(new_ver.trim_start_matches(|c: char| !c.is_numeric())),
                ) {
                    (Ok(old), Ok(new)) => {
                        if new.major > old.major {
                            "MAJOR".red().bold()
                        } else if new.minor > old.minor {
                            "minor".yellow().bold()
                        } else {
                            "patch".green()
                        }
                    }
                    _ => "update".dimmed(), // Fallback for non-semver
                };

                println!(
                    "  {:>8} {} {} → {}",
                    update_label,
                    name.white().bold(),
                    old_ver.dimmed(),
                    new_ver.green()
                );
            }
            println!("\n{} Run 'omg update' to install", "→".dimmed());
        }
    } else {
        let pacman = ArchPackageManager::new();
        pacman.update().await?;
    }

    Ok(())
}

/// Show package information
pub async fn info(package: &str) -> Result<()> {
    let start = std::time::Instant::now();
    println!(
        "{} Package info for '{}':\n",
        "OMG".cyan().bold(),
        package.yellow()
    );

    // 1. Try daemon first (ULTRA FAST - <10ms)
    if let Ok(mut client) = DaemonClient::connect().await {
        if let Ok(info) = client.info(package).await {
            println!(
                "{} {} ({:.1}ms)\n",
                "OMG".cyan().bold(),
                "Daemon result".dimmed(),
                start.elapsed().as_secs_f64() * 1000.0
            );

            println!("{} {}", info.name.white().bold(), info.version.green());
            println!("  {} {}", "Description:".dimmed(), info.description);
            let source_label = if info.source == "official" {
                format!("Official repository ({})", info.repo.cyan()).green()
            } else {
                "AUR (Arch User Repository)".yellow()
            };
            println!("  {} {}", "Source:".dimmed(), source_label);
            println!("  {} {}", "URL:".dimmed(), info.url);
            println!(
                "  {} {:.2} MB",
                "Size:".dimmed(),
                info.size as f64 / 1024.0 / 1024.0
            );
            println!(
                "  {} {:.2} MB",
                "Download:".dimmed(),
                info.download_size as f64 / 1024.0 / 1024.0
            );
            if !info.licenses.is_empty() {
                println!("  {} {}", "License:".dimmed(), info.licenses.join(", "));
            }
            if !info.depends.is_empty() {
                println!("  {} {}", "Depends:".dimmed(), info.depends.join(", "));
            }
            return Ok(());
        }
    }

    // 2. Fallback to local ALPM if daemon unreachable (slow)
    if let Some(info) = get_sync_pkg_info(package).ok().flatten() {
        display_pkg_info(&info);
        println!(
            "\n  {} Official repository ({})",
            "Source:".green(),
            info.repo.cyan()
        );
        return Ok(());
    }

    // 3. Try AUR directly as final fallback
    let details = search_detailed(package).await.ok();
    if let Some(pkgs) = details {
        if let Some(pkg) = pkgs.into_iter().find(|p| p.name == package) {
            println!("  {} {}", "Name:".yellow(), pkg.name.white().bold());
            println!("  {} {}", "Version:".yellow(), pkg.version);
            println!(
                "  {} {}",
                "Description:".yellow(),
                pkg.description.unwrap_or_default()
            );
            println!(
                "  {} {}",
                "Maintainer:".yellow(),
                pkg.maintainer.unwrap_or("orphan".to_string())
            );
            println!("  {} {}", "Votes:".yellow(), pkg.num_votes);
            println!("  {} {:.2}%", "Popularity:".yellow(), pkg.popularity);
            if pkg.out_of_date.is_some() {
                println!("  {} {}", "Status:".red(), "OUT OF DATE".red().bold());
            }
            println!("\n  {} AUR (Arch User Repository)", "Source:".yellow());
            return Ok(());
        }
    }

    println!("{} Package '{}' not found", "✗".red(), package);
    Ok(())
}

/// Clean up orphans and caches
pub async fn clean(orphans: bool, cache: bool, aur: bool, all: bool) -> Result<()> {
    println!("{} Cleaning up...\n", "OMG".cyan().bold());

    let do_orphans = orphans || all;
    let do_cache = cache || all;
    let do_aur = aur || all;

    if !do_orphans && !do_cache && !do_aur {
        // Default: show what can be cleaned
        let orphan_list = list_orphans_direct().unwrap_or_default();
        if !orphan_list.is_empty() {
            println!(
                "{} {} orphan packages can be removed",
                "→".blue(),
                orphan_list.len()
            );
            println!("  Run: {}", "omg clean --orphans".dimmed());
        }

        println!(
            "{} To clear package cache: {}",
            "→".blue(),
            "omg clean --cache".dimmed()
        );
        println!(
            "{} To clear AUR builds: {}",
            "→".blue(),
            "omg clean --aur".dimmed()
        );
        println!(
            "{} To clean everything: {}",
            "→".blue(),
            "omg clean --all".dimmed()
        );
        return Ok(());
    }

    if do_orphans {
        remove_orphans().await?;
    }

    if do_cache {
        println!("{} Clearing package cache...", "→".blue());
        match clean_cache(1) {
            // Keep 1 version by default
            Ok((removed, freed)) => {
                println!(
                    "{} Removed {} files, freed {:.2} MB",
                    "✓".green(),
                    removed,
                    freed as f64 / 1024.0 / 1024.0
                );
            }
            Err(e) => {
                println!("{} Failed to clear cache: {}", "✗".red(), e);
            }
        }
    }

    if do_aur {
        let aur_client = AurClient::new();
        aur_client.clean_all()?;
    }

    println!("\n{} Cleanup complete!", "✓".green());
    Ok(())
}

/// List explicitly installed packages
pub async fn explicit() -> Result<()> {
    println!("{} Explicitly installed packages:\n", "OMG".cyan().bold());

    let packages = list_explicit().await?;

    for pkg in &packages {
        println!("  {}", pkg);
    }

    println!("\n{} {} packages", "Total:".green(), packages.len());
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        // Find a valid char boundary
        let mut end = max.saturating_sub(3);
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &s[..end])
    }
}
