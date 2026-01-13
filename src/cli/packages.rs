use anyhow::Result;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};

use crate::cli::style;
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
    sync_databases_parallel,
    OfficialPackageManager,
    AurClient,
    PackageManager,
};

/// Search for packages in official repos and AUR - LIGHTNING FAST
pub async fn search(query: &str, detailed: bool, interactive: bool) -> Result<()> {
    let start = std::time::Instant::now();

    let mut official_packages = Vec::new();
    let mut aur_packages_detailed = None;
    let mut aur_packages_basic = None;

    // 1. Try Daemon (Ultra Fast, Cached, Pooled)
    let mut daemon_used = false;
    if let Ok(mut client) = DaemonClient::connect().await {
        if let Ok(res) = client.search(query, Some(50)).await {
            daemon_used = true;
            let mut aur_basic = Vec::new();

            for pkg in res.packages {
                if pkg.source == "official" {
                    official_packages.push(crate::package_managers::SyncPackage {
                        name: pkg.name,
                        version: pkg.version,
                        description: pkg.description,
                        repo: "official".to_string(), 
                        download_size: 0,
                        installed: false, 
                    });
                } else {
                    aur_basic.push(crate::core::Package {
                        name: pkg.name,
                        version: pkg.version,
                        description: pkg.description,
                        source: crate::core::PackageSource::Aur,
                        installed: false,
                    });
                }
            }
            if !aur_basic.is_empty() {
                aur_packages_basic = Some(aur_basic);
            }
        }
    }

    if !daemon_used {
        // 2. Fallback: Direct libalpm query + Network
        official_packages = search_sync(query).unwrap_or_default();

        // Search AUR
        if detailed || interactive {
            let pb = style::spinner("Searching AUR...");
            let res = search_detailed(query).await.unwrap_or_default();
            pb.finish_and_clear();
            aur_packages_detailed = Some(res);
        } else if !interactive {
            let aur = AurClient::new();
            aur_packages_basic = Some(aur.search(query).await.unwrap_or_default());
        }
    }

    let sync_time = start.elapsed();

    if interactive {
        let mut items = Vec::new();
        let mut pkgs_to_install = Vec::new();

        // Add official
        for pkg in &official_packages {
            let status = if pkg.installed { "[installed]" } else { "" };
            items.push(format!(
                "{} {} {} ({}) - {}",
                style::package(&pkg.name),
                style::version(&pkg.version),
                status,
                style::info(&pkg.repo),
                style::dim(&truncate(&pkg.description, 40))
            ));
            pkgs_to_install.push(pkg.name.clone());
        }

        // Add AUR
        if let Some(aur) = &aur_packages_detailed {
            for pkg in aur {
                items.push(format!(
                    "{} {} ({}) - {}",
                    style::package(&pkg.name),
                    style::version(&pkg.version),
                    style::warning("AUR"),
                    style::dim(&truncate(&pkg.description.clone().unwrap_or_default(), 40))
                ));
                pkgs_to_install.push(pkg.name.clone());
            }
        } else if let Some(aur) = &aur_packages_basic {
            for pkg in aur {
                items.push(format!(
                    "{} {} ({}) - {}",
                    style::package(&pkg.name),
                    style::version(&pkg.version),
                    style::warning("AUR"),
                    style::dim(&truncate(&pkg.description, 40))
                ));
                pkgs_to_install.push(pkg.name.clone());
            }
        }

        if items.is_empty() {
            println!(
                "{}",
                style::error(&format!("No packages found for '{}'", query))
            );
            return Ok(());
        }

        println!("{}", style::arrow("Select packages to install:"));

        let selections = MultiSelect::with_theme(&ColorfulTheme::default())
            .items(&items)
            .interact()?;

        if selections.is_empty() {
            println!("{}", style::dim("No packages selected"));
            return Ok(());
        }

        let selected_names: Vec<String> = selections
            .into_iter()
            .map(|i| pkgs_to_install[i].clone())
            .collect();

        return install(&selected_names, false).await;
    }

    // Display official packages first
    if !official_packages.is_empty() {
        println!(
            "{} {} results ({:.1}ms)\n",
            style::header("OMG"),
            official_packages.len(),
            sync_time.as_secs_f64() * 1000.0
        );

        println!("{}", style::header("Official Repositories"));
        for pkg in official_packages.iter().take(20) {
            let installed = if pkg.installed {
                style::dim(" [installed]")
            } else {
                "".into()
            };
            println!(
                "  {} {} ({}) - {}{}",
                style::package(&pkg.name),
                style::version(&pkg.version),
                style::info(&pkg.repo),
                style::dim(&truncate(&pkg.description, 50)),
                installed
            );
        }
        if official_packages.len() > 20 {
            println!(
                "  {}",
                style::dim(&format!(
                    "(+{}) more packages...",
                    official_packages.len() - 20
                ))
            );
        }
        println!();
    }

    // Search AUR (cached result)
    if let Some(aur_packages) = aur_packages_detailed {
        if !aur_packages.is_empty() {
            println!("{}", style::header("AUR (Arch User Repository)"));
            for pkg in aur_packages.iter().take(10) {
                let out_of_date = if pkg.out_of_date.is_some() {
                    style::error(" [OUT OF DATE]")
                } else {
                    "".into()
                };
                println!(
                    "  {} {} - {} {} {}{}",
                    style::package(&pkg.name),
                    style::version(&pkg.version),
                    style::info(&format!("↑{}", pkg.num_votes)),
                    style::info(&format!("{:.1}%", pkg.popularity)),
                    style::dim(&truncate(&pkg.description.clone().unwrap_or_default(), 40)),
                    out_of_date
                );
            }
            if aur_packages.len() > 10 {
                println!(
                    "  {}",
                    style::dim(&format!("(+{}) more packages...", aur_packages.len() - 10))
                );
            }
            println!();
        }
    } else if let Some(aur_packages) = aur_packages_basic {
        if !aur_packages.is_empty() {
            println!("{}", style::header("AUR (Arch User Repository)"));
            for pkg in aur_packages.iter().take(10) {
                println!(
                    "  {} {} - {}",
                    style::package(&pkg.name),
                    style::version(&pkg.version),
                    style::dim(&truncate(&pkg.description, 55))
                );
            }
            if aur_packages.len() > 10 {
                println!(
                    "  {}",
                    style::dim(&format!("(+{}) more packages...", aur_packages.len() - 10))
                );
            }
            println!();
        }
    }

    println!(
        "{} {}",
        style::arrow("Use"),
        style::command("omg info <package> for details")
    );

    Ok(())
}

/// Install packages (auto-detects AUR) with Graded Security
pub async fn install(packages: &[String], _yes: bool) -> Result<()> {
    if packages.is_empty() {
        println!("{}", style::error("No packages specified"));
        return Ok(());
    }

    let policy = SecurityPolicy::load_default().unwrap_or_default();

    println!(
        "{} Analyzing {} package(s) with {} model...\n",
        style::header("OMG"),
        packages.len(),
        style::success("Graded Security")
    );

    let aur = AurClient::new();
    let mut official = Vec::new();
    let mut aur_pkgs = Vec::new();
    let mut local_pkgs = Vec::new();
    let mut not_found = Vec::new();

    for pkg_name in packages {
        // Check if it's a local package file
        if std::path::Path::new(pkg_name).exists() && pkg_name.contains(".pkg.tar.") {
            local_pkgs.push(pkg_name.clone());
            continue;
        }

        let mut target_pkg_name = pkg_name.clone();

        // Try to find package info
        let mut sync_info = get_sync_pkg_info(&target_pkg_name).ok().flatten();
        let mut aur_info = if sync_info.is_none() {
            aur.info(&target_pkg_name).await.ok().flatten()
        } else {
            None
        };

        // If not found, try to resolve typo
        if sync_info.is_none() && aur_info.is_none() {
            // Only if interactive
            if console::user_attended() {
                // Search official
                let results = search_sync(&target_pkg_name).unwrap_or_default();
                if let Some(best_match) = results.first() {
                    if Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt(format!(
                            "Package '{}' not found. Did you mean '{}'?",
                            style::package(&target_pkg_name),
                            style::package(&best_match.name)
                        ))
                        .default(true)
                        .interact()?
                    {
                        target_pkg_name = best_match.name.clone();
                        sync_info = get_sync_pkg_info(&target_pkg_name).ok().flatten();
                    }
                } else {
                    // Try AUR search
                    if let Ok(results) = aur.search(&target_pkg_name).await {
                        if let Some(best_match) = results.first() {
                            if Confirm::with_theme(&ColorfulTheme::default())
                                .with_prompt(format!(
                                    "Package '{}' not found. Did you mean '{}' (AUR)?",
                                    style::package(&target_pkg_name),
                                    style::package(&best_match.name)
                                ))
                                .default(true)
                                .interact()?
                            {
                                target_pkg_name = best_match.name.clone();
                                aur_info = Some(best_match.clone());
                            }
                        }
                    }
                }
            }
        }

        let (grade, is_aur, is_official, license) = if let Some(info) = sync_info {
            let grade = policy
                .assign_grade(&info.name, &info.version, false, true)
                .await;
            let license = info.licenses.first().cloned();
            (grade, false, true, license)
        } else if let Some(info) = aur_info {
            let grade = policy
                .assign_grade(&info.name, &info.version, true, false)
                .await;
            (grade, true, false, None)
        } else {
            not_found.push(pkg_name.clone());
            continue;
        };

        // Check policy
        if let Err(e) = policy.check_package(&target_pkg_name, is_aur, license.as_deref(), grade) {
            println!("{}", style::error(&format!("Security Block: {}", e)));
            anyhow::bail!("Installation aborted due to security policy");
        }

        let grade_colored = match grade {
            crate::core::security::SecurityGrade::Locked => style::success(&grade.to_string()),
            crate::core::security::SecurityGrade::Verified => style::info(&grade.to_string()),
            crate::core::security::SecurityGrade::Community => style::warning(&grade.to_string()),
            crate::core::security::SecurityGrade::Risk => style::error(&grade.to_string()),
        };

        if is_official {
            official.push(target_pkg_name.clone());
            println!(
                "{} Official: {} [{}]",
                style::arrow("→"),
                style::package(&target_pkg_name),
                grade_colored
            );
        } else if is_aur {
            aur_pkgs.push(target_pkg_name.clone());
            println!(
                "{} AUR:      {} [{}]",
                style::warning("→"),
                style::package(&target_pkg_name),
                grade_colored
            );
        }
    }

    if !not_found.is_empty() {
        println!(
            "{}",
            style::error(&format!("Not found: {}", not_found.join(", ")))
        );
    }
    println!();

    // Install official packages
    if !official.is_empty() {
        let pacman = OfficialPackageManager::new();
        pacman.install(&official).await?;
    }

    // Install local packages
    if !local_pkgs.is_empty() {
        let pacman = OfficialPackageManager::new();
        pacman.install(&local_pkgs).await?;
    }

    // Install AUR packages
    for pkg in &aur_pkgs {
        aur.install(pkg).await?;
    }

    if official.is_empty() && aur_pkgs.is_empty() && local_pkgs.is_empty() {
        println!("{}", style::dim("No packages to install"));
    }

    Ok(())
}

/// Remove packages
pub async fn remove(packages: &[String], recursive: bool) -> Result<()> {
    if packages.is_empty() {
        println!("{}", style::error("No packages specified"));
        return Ok(());
    }

    let pacman = OfficialPackageManager::new();

    if recursive {
        println!("{}", style::info("Removing with unused dependencies..."));
    }

    pacman.remove(packages).await?;

    Ok(())
}

/// Update all packages
pub async fn update(check_only: bool) -> Result<()> {
    if check_only {
        let pb = style::spinner("Checking for updates...");
        let update_list = get_update_list().unwrap_or_default();
        pb.finish_and_clear();

        if update_list.is_empty() {
            println!("{}", style::success("System is up to date!"));
        } else {
            println!(
                "{} {} updates available:",
                style::arrow("→"),
                update_list.len()
            );
            for (name, old_ver, new_ver) in &update_list {
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
                    _ => "update".dimmed(),
                };

                println!(
                    "  {:>8} {} {} → {}",
                    update_label,
                    style::package(name),
                    style::dim(old_ver),
                    style::version(new_ver)
                );
            }
            println!("\n{}", style::dim("Run 'omg update' to install"));
        }
    } else {
        let pacman = OfficialPackageManager::new();
        pacman.update().await?;
    }

    Ok(())
}

/// Show package information
pub async fn info(package: &str) -> Result<()> {
    let start = std::time::Instant::now();
    println!(
        "{} Package info for '{}':\n",
        style::header("OMG"),
        style::package(package)
    );

    // 1. Try daemon first (ULTRA FAST - <10ms)
    if let Ok(mut client) = DaemonClient::connect().await {
        if let Ok(info) = client.info(package).await {
            println!(
                "{} {} ({:.1}ms)\n",
                style::header("OMG"),
                style::dim("Daemon result"),
                start.elapsed().as_secs_f64() * 1000.0
            );

            println!(
                "{} {}",
                style::package(&info.name),
                style::version(&info.version)
            );
            println!("  {} {}", style::dim("Description:"), info.description);
            let source_label = if info.source == "official" {
                format!("Official repository ({})", style::info(&info.repo))
            } else {
                style::warning("AUR (Arch User Repository)")
            };
            println!("  {} {}", style::dim("Source:"), source_label);
            println!("  {} {}", style::dim("URL:"), style::url(&info.url));
            println!(
                "  {} {:.2} MB",
                style::dim("Size:"),
                info.size as f64 / 1024.0 / 1024.0
            );
            println!(
                "  {} {:.2} MB",
                style::dim("Download:"),
                info.download_size as f64 / 1024.0 / 1024.0
            );
            if !info.licenses.is_empty() {
                println!("  {} {}", style::dim("License:"), info.licenses.join(", "));
            }
            if !info.depends.is_empty() {
                println!("  {} {}", style::dim("Depends:"), info.depends.join(", "));
            }
            return Ok(());
        }
    }

    // 2. Fallback to local ALPM if daemon unreachable (slow)
    if let Some(info) = get_sync_pkg_info(package).ok().flatten() {
        display_pkg_info(&info);
        println!(
            "\n  {} Official repository ({})",
            style::success("Source:"),
            style::info(&info.repo)
        );
        return Ok(());
    }

    // 3. Try AUR directly as final fallback
    let pb = style::spinner("Searching AUR...");
    let details = search_detailed(package).await.ok();
    pb.finish_and_clear();

    if let Some(pkgs) = details {
        if let Some(pkg) = pkgs.into_iter().find(|p| p.name == package) {
            println!(
                "  {} {}",
                style::warning("Name:"),
                style::package(&pkg.name)
            );
            println!(
                "  {} {}",
                style::warning("Version:"),
                style::version(&pkg.version)
            );
            println!(
                "  {} {}",
                style::warning("Description:"),
                pkg.description.unwrap_or_default()
            );
            println!(
                "  {} {}",
                style::warning("Maintainer:"),
                pkg.maintainer.unwrap_or("orphan".to_string())
            );
            println!("  {} {}", style::warning("Votes:"), pkg.num_votes);
            println!("  {} {:.2}%", style::warning("Popularity:"), pkg.popularity);
            if pkg.out_of_date.is_some() {
                println!(
                    "  {} {}",
                    style::error("Status:"),
                    style::error("OUT OF DATE")
                );
            }
            println!("\n  {}", style::warning("AUR (Arch User Repository)"));
            return Ok(());
        }
    }

    println!(
        "{}",
        style::error(&format!("Package '{}' not found", package))
    );
    Ok(())
}

/// Clean up orphans and caches
pub async fn clean(orphans: bool, cache: bool, aur: bool, all: bool) -> Result<()> {
    println!("{} Cleaning up...\n", style::header("OMG"));

    let do_orphans = orphans || all;
    let do_cache = cache || all;
    let do_aur = aur || all;

    if !do_orphans && !do_cache && !do_aur {
        // Default: show what can be cleaned
        let orphan_list = list_orphans_direct().unwrap_or_default();
        if !orphan_list.is_empty() {
            println!(
                "{} {} orphan packages can be removed",
                style::arrow("→"),
                orphan_list.len()
            );
            println!("  Run: {}", style::command("omg clean --orphans"));
        }

        println!(
            "{} To clear package cache: {}",
            style::arrow("→"),
            style::command("omg clean --cache")
        );
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
        remove_orphans().await?;
    }

    if do_cache {
        println!("{}", style::info("Clearing package cache..."));
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
                println!("{}", style::error(&format!("Failed to clear cache: {}", e)));
            }
        }
    }

    if do_aur {
        let aur_client = AurClient::new();
        aur_client.clean_all()?;
    }

    println!("\n{}", style::success("Cleanup complete!"));
    Ok(())
}

/// List explicitly installed packages
pub async fn explicit() -> Result<()> {
    println!("{} Explicitly installed packages:\n", style::header("OMG"));

    // Try daemon first
    let packages = if let Ok(mut client) = DaemonClient::connect().await {
        if let Ok(pkgs) = client.list_explicit().await {
            pkgs
        } else {
            list_explicit().await.unwrap_or_default()
        }
    } else {
        list_explicit().await.unwrap_or_default()
    };

    for pkg in &packages {
        println!("  {}", style::package(pkg));
    }

    println!("\n{} {} packages", style::success("Total:"), packages.len());
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

/// Sync package databases from mirrors (parallel, fast)
pub async fn sync() -> Result<()> {
    sync_databases_parallel().await
}
