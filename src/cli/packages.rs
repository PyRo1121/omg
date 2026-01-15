use anyhow::Result;
use colored::Colorize;
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect};
use futures::StreamExt;

use crate::cli::style;
use crate::config::Settings;
use crate::core::client::DaemonClient;
use crate::core::completion::CompletionEngine;
use crate::core::history::{HistoryManager, PackageChange, TransactionType};
use crate::core::security::SecurityPolicy;
use crate::core::Database;
use crate::daemon::protocol::{Request, ResponseResult};
use crate::package_managers::{
    check_updates_cached,
    clean_cache,
    display_pkg_info,
    get_sync_pkg_info,
    list_orphans_direct,
    remove_orphans,
    search_detailed,
    // Direct ALPM functions (10-100x faster)
    search_sync,
    sync_databases_parallel,
    AurClient,
    OfficialPackageManager,
    PackageManager,
};

/// Search for packages in official repos and AUR (Synchronous fast-path)
pub fn search_sync_cli(query: &str, detailed: bool, interactive: bool) -> Result<bool> {
    if detailed || interactive {
        // Fallback to async for these modes as they require spin-up or complex interaction
        return Ok(false);
    }

    let start = std::time::Instant::now();

    // 1. Try Daemon first (ULTRA FAST - <1ms)
    if let Ok(mut client) = DaemonClient::connect_sync() {
        if let Ok(ResponseResult::Search(res)) = client.call_sync(Request::Search {
            id: 0,
            query: query.to_string(),
            limit: Some(50),
        }) {
            let sync_time = start.elapsed();

            if res.packages.is_empty() {
                return Ok(false);
            }

            let mut stdout = std::io::BufWriter::new(std::io::stdout());
            use std::io::Write;

            writeln!(
                stdout,
                "{} {} results ({:.1}ms)\n",
                style::header("OMG"),
                res.packages.len(),
                sync_time.as_secs_f64() * 1000.0
            )?;

            writeln!(stdout, "{}", style::header("Official Repositories"))?;
            for pkg in res.packages.iter().take(20) {
                writeln!(
                    stdout,
                    "  {} {} ({}) - {}",
                    style::package(&pkg.name),
                    style::version(&pkg.version),
                    style::info(&pkg.source), // Source might be 'official' or 'aur' in search result
                    style::dim(&truncate(&pkg.description, 50))
                )?;
            }
            if res.packages.len() > 20 {
                let more = res.packages.len() - 20;
                write!(stdout, "  {}", style::dim("(+"))?;
                write!(stdout, "{more}")?;
                writeln!(stdout, "{})", style::dim(" more packages..."))?;
            }
            writeln!(
                stdout,
                "\n{} {}",
                style::arrow("Use"),
                style::command("omg info <package> for details")
            )?;
            stdout.flush()?;
            return Ok(true);
        }
    }

    Ok(false)
}

/// Search for packages in official repos and AUR - LIGHTNING FAST
pub async fn search(query: &str, detailed: bool, interactive: bool) -> Result<()> {
    // Try sync path first
    if search_sync_cli(query, detailed, interactive)? {
        return Ok(());
    }

    // ... rest of async search
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
                style::error(&format!("No packages found for '{query}'"))
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
                String::new()
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
                    String::new()
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
pub async fn install(packages: &[String], yes: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
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
    let mut changes = Vec::new();

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
            // Only if interactive and not auto-confirming
            if console::user_attended() && !yes {
                // 1. Try Fuzzy Matching (Best for typos like 'frfx' -> 'firefox')
                let suggestion = fuzzy_suggest(&target_pkg_name);

                if let Some(best_match) = suggestion {
                    if Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt(format!(
                            "Package '{}' not found. Did you mean '{}'?",
                            style::package(&target_pkg_name),
                            style::package(&best_match)
                        ))
                        .default(true)
                        .interact()?
                    {
                        target_pkg_name = best_match;
                        // Retry finding info
                        sync_info = get_sync_pkg_info(&target_pkg_name).ok().flatten();
                        aur_info = if sync_info.is_none() {
                            aur.info(&target_pkg_name).await.ok().flatten()
                        } else {
                            None
                        };
                    }
                } else {
                    // 2. Try Substring Search (Fallback)
                    // Search official
                    let results = search_sync(&target_pkg_name).unwrap_or_default();
                    if let Some(best_match) = results.first() {
                        if Confirm::with_theme(&ColorfulTheme::default())
                            .with_prompt(format!(
                                "Package '{}' not found. Did you mean '{}' ?",
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
        }

        let (grade, is_aur, is_official, license, change) = if let Some(info) = sync_info {
            let grade = policy
                .assign_grade(&info.name, &info.version, false, true)
                .await;
            let license = info.licenses.first().cloned();
            let change = PackageChange {
                name: info.name.clone(),
                old_version: None, // Simplified for now
                new_version: Some(info.version.clone()),
                source: "official".to_string(),
            };
            (grade, false, true, license, change)
        } else if let Some(info) = aur_info {
            let grade = policy
                .assign_grade(&info.name, &info.version, true, false)
                .await;
            let change = PackageChange {
                name: info.name.clone(),
                old_version: None,
                new_version: Some(info.version.clone()),
                source: "aur".to_string(),
            };
            (grade, true, false, None, change)
        } else {
            not_found.push(pkg_name.clone());
            continue;
        };

        // Check policy
        if let Err(e) = policy.check_package(&target_pkg_name, is_aur, license.as_deref(), grade) {
            println!("{}", style::error(&format!("Security Block: {e}")));
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
        changes.push(change);
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
        if let Err(e) = pacman.install(&official).await {
            if let Ok(history) = HistoryManager::new() {
                let _ = history.add_transaction(TransactionType::Install, changes, false);
            }
            return Err(e);
        }
    }

    // Install local packages
    if !local_pkgs.is_empty() {
        let pacman = OfficialPackageManager::new();
        if let Err(e) = pacman.install(&local_pkgs).await {
            if let Ok(history) = HistoryManager::new() {
                let _ = history.add_transaction(TransactionType::Install, changes, false);
            }
            return Err(e);
        }
    }

    // Install AUR packages
    for pkg in &aur_pkgs {
        if let Err(e) = aur.install(pkg).await {
            if let Ok(history) = HistoryManager::new() {
                let _ = history.add_transaction(TransactionType::Install, changes, false);
            }
            return Err(e);
        }
    }

    if official.is_empty() && aur_pkgs.is_empty() && local_pkgs.is_empty() {
        println!("{}", style::dim("No packages to install"));
        return Ok(());
    }

    // Log transaction
    if let Ok(history) = HistoryManager::new() {
        let _ = history.add_transaction(TransactionType::Install, changes, true);
    }

    Ok(())
}

/// Remove packages
pub async fn remove(packages: &[String], recursive: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    let mut changes = Vec::new();
    for pkg in packages {
        if let Ok(Some(info)) = crate::package_managers::get_local_package(pkg) {
            changes.push(PackageChange {
                name: pkg.clone(),
                old_version: Some(info.version),
                new_version: None,
                source: "official".to_string(), // Defaulting to official for now
            });
        }
    }

    let pacman = OfficialPackageManager::new();

    if recursive {
        println!("{}", style::info("Removing with unused dependencies..."));
    }

    let result = pacman.remove(packages).await;
    let success = result.is_ok();

    // Log transaction
    if !changes.is_empty() {
        if let Ok(history) = HistoryManager::new() {
            let _ = history.add_transaction(TransactionType::Remove, changes, success);
        }
    }

    result
}

/// Update all packages
pub async fn update(check_only: bool) -> Result<()> {
    let aur = AurClient::new();
    let pacman = OfficialPackageManager::new();

    // STEP 1: Sync databases first to get latest info
    if !crate::core::is_root() {
        println!(
            "{} Synchronizing databases (elevation might be required)...",
            style::arrow("→")
        );
    }
    pacman.sync_databases().await?;

    let pb = style::spinner("Checking for updates...");

    // STEP 2: Get both official and AUR updates
    let official_updates_raw = check_updates_cached().unwrap_or_default();
    let aur_updates = aur.get_update_list().await.unwrap_or_default();

    pb.finish_and_clear();

    if official_updates_raw.is_empty() && aur_updates.is_empty() {
        println!("{}", style::success("System is up to date!"));
        return Ok(());
    }

    // STEP 3: Display combined updates
    println!(
        "{} Found {} official and {} AUR update(s):",
        style::header("OMG"),
        style::info(&official_updates_raw.len().to_string()),
        style::warning(&aur_updates.len().to_string())
    );

    let display_list = |updates: &[(String, String, String)], source: &str| {
        for (name, old_ver, new_ver) in updates {
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
                "  {:>8} {} {} {} → {}",
                update_label,
                style::package(name),
                style::dim(&format!("({source})")),
                style::dim(old_ver),
                style::version(new_ver)
            );
        }
    };

    // Convert raw official updates to common format for display
    let official_display: Vec<(String, String, String)> = official_updates_raw
        .iter()
        .map(|(n, o, n2, _, _, _)| (n.clone(), o.clone(), n2.clone()))
        .collect();

    display_list(&official_display, "repo");
    display_list(&aur_updates, "aur");

    let mut changes = Vec::new();
    for (name, old_ver, new_ver, _, _, _) in &official_updates_raw {
        changes.push(PackageChange {
            name: name.clone(),
            old_version: Some(old_ver.clone()),
            new_version: Some(new_ver.clone()),
            source: "official".to_string(),
        });
    }
    for (name, old_ver, new_ver) in &aur_updates {
        changes.push(PackageChange {
            name: name.clone(),
            old_version: Some(old_ver.clone()),
            new_version: Some(new_ver.clone()),
            source: "aur".to_string(),
        });
    }

    if check_only {
        println!("\n{}", style::dim("Run 'omg update' to install"));
        return Ok(());
    }

    // STEP 4: Interactive confirmation
    if console::user_attended() {
        if !Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("\nProceed with system upgrade?")
            .default(true)
            .interact()?
        {
            println!("{}", style::dim("Upgrade cancelled."));
            return Ok(());
        }
    } else {
        println!("{}", style::dim("Proceeding without prompt (no TTY detected)."));
    }

    // STEP 5: Execute upgrades
    // Always do official first
    if !official_updates_raw.is_empty() {
        println!("\n{} Downloading official packages...", style::arrow("→"));

        // Prepare download jobs
        let jobs: Vec<crate::package_managers::DownloadJob> = official_updates_raw
            .iter()
            .map(|(name, _, new_ver, repo, filename, size)| {
                crate::package_managers::DownloadJob::new(name, new_ver, repo, filename, *size)
            })
            .collect();

        // Download in parallel (8 threads)
        let pkg_paths = crate::package_managers::download_packages_parallel(jobs, 8).await?;

        println!("\n{} Installing official packages...", style::arrow("→"));

        // Convert PathBufs to Strings for the install method
        let pkg_paths_str: Vec<String> = pkg_paths
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        if !pkg_paths_str.is_empty() {
            pacman.install(&pkg_paths_str).await?;
        }
    }

    if !aur_updates.is_empty() {
        use indicatif::{ProgressBar, ProgressStyle};

        let settings = Settings::load().unwrap_or_default();
        let concurrency = settings.aur.build_concurrency.max(1);
        let total = aur_updates.len();
        println!(
            "\n{} Building {} AUR package{}...\n",
            style::arrow("→"),
            total,
            if total == 1 { "" } else { "s" }
        );
        println!(
            "{} Using build concurrency: {}",
            style::dim("→"),
            style::info(&concurrency.to_string())
        );

        let progress = ProgressBar::new(total as u64);
        progress.set_style(
            ProgressStyle::default_bar()
                .template("  [{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap(),
        );

        let mut built_packages: Vec<(String, String, std::path::PathBuf)> = Vec::new();
        let mut failed_builds: Vec<(String, String)> = Vec::new();

        let mut stream = futures::stream::iter(aur_updates.into_iter())
            .map(|(name, _old_ver, new_ver)| {
                let aur = aur.clone();
                async move {
                    let result = aur.build_only(&name).await;
                    (name, new_ver, result)
                }
            })
            .buffer_unordered(concurrency);

        while let Some((name, new_ver, result)) = stream.next().await {
            progress.inc(1);
            match result {
                Ok(pkg_path) => {
                    progress.set_message(format!("Built {name}"));
                    built_packages.push((name, new_ver, pkg_path));
                }
                Err(e) => {
                    progress.set_message(format!("Failed {name}"));
                    failed_builds.push((name, e.to_string()));
                }
            }
        }

        progress.finish_and_clear();

        // Phase 2: Install all built packages in a single transaction
        if !built_packages.is_empty() {
            let spinner = ProgressBar::new_spinner();
            #[allow(clippy::literal_string_with_formatting_args)]
            let install_template =
                format!("\n  {{spinner}} Installing {} packages...", built_packages.len());
            spinner.set_style(
                ProgressStyle::default_spinner()
                    .template(&install_template)
                    .unwrap(),
            );
            spinner.enable_steady_tick(std::time::Duration::from_millis(80));

            let pkg_paths: Vec<_> = built_packages.iter().map(|(_, _, p)| p.clone()).collect();

            let exe = std::env::current_exe().unwrap_or_else(|_| std::path::PathBuf::from("omg"));
            let mut cmd = tokio::process::Command::new("sudo");
            cmd.arg("--").arg(&exe).arg("install");
            for path in &pkg_paths {
                cmd.arg(path);
            }
            // Suppress install output for clean UI
            cmd.stdout(std::process::Stdio::null());
            cmd.stderr(std::process::Stdio::null());

            match cmd.status().await {
                Ok(status) if status.success() => {
                    spinner.finish_and_clear();
                    println!(
                        "\n  {} Installed {} packages",
                        style::success("✓"),
                        built_packages.len()
                    );
                }
                Ok(_) | Err(_) => {
                    spinner.finish_and_clear();
                    println!(
                        "\n  {} Batch install failed, trying individually...",
                        style::warning("⚠")
                    );
                    // Fallback to individual installs (quiet)
                    for (name, _ver, path) in &built_packages {
                        let mut cmd = tokio::process::Command::new("sudo");
                        cmd.arg("--").arg(&exe).arg("install").arg(path);
                        cmd.stdout(std::process::Stdio::null());
                        cmd.stderr(std::process::Stdio::null());
                        if let Ok(status) = cmd.status().await {
                            if status.success() {
                                // Silent success in fallback
                            } else {
                                println!(
                                    "    {} Failed: {}",
                                    style::error("✗"),
                                    style::package(name)
                                );
                            }
                        }
                    }
                }
            }
        }

        let (success_count, fail_count) = (built_packages.len(), failed_builds.len());
        println!(
            "\n{} AUR: {} built, {} failed",
            if fail_count == 0 {
                style::success("✓")
            } else {
                style::warning("⚠")
            },
            success_count,
            fail_count
        );
        if !failed_builds.is_empty() {
            println!("  {} Failed builds:", style::error("✗"));
            for (name, err) in &failed_builds {
                println!("    {} - {}", style::package(name), err);
            }
        }
    }

    // Log transaction
    if let Ok(history) = HistoryManager::new() {
        let _ = history.add_transaction(TransactionType::Update, changes, true);
        // Assuming overall progress if reached here
    }

    Ok(())
}

/// Show package information
/// Show package information (Synchronous fast-path)
pub fn info_sync(package: &str) -> Result<bool> {
    let start = std::time::Instant::now();

    // 1. Try daemon first (ULTRA FAST - <1ms)
    if let Ok(mut client) = DaemonClient::connect_sync() {
        if let Ok(info) = client.info_sync(package) {
            let mut stdout = std::io::BufWriter::new(std::io::stdout());
            use std::io::Write;

            writeln!(
                stdout,
                "{} {} ({:.1}ms)\n",
                style::header("OMG"),
                style::dim("Daemon result (Sync Bridge)"),
                start.elapsed().as_secs_f64() * 1000.0
            )?;

            display_detailed_info_buffered(&mut stdout, &info)?;
            stdout.flush()?;
            return Ok(true);
        }
    }

    // 2. Fallback to local ALPM (Sync, fast)
    if let Some(info) = get_sync_pkg_info(package).ok().flatten() {
        display_pkg_info(&info);
        println!(
            "\n  {} Official repository ({})",
            style::success("Source:"),
            style::info(&info.repo)
        );
        return Ok(true);
    }

    Ok(false)
}

/// Show AUR package information (Async fallback)
pub async fn info_aur(package: &str) -> Result<()> {
    let aur = AurClient::new();
    if let Some(info) = aur.info(package).await? {
        // Display beautified info
        println!(
            "{} {} {}",
            style::package(&info.name),
            style::version(&info.version),
            style::warning("(AUR)")
        );
        println!("  {} {}", style::dim("Description:"), info.description);

        // Query detailed info for better UX
        if let Ok(detailed) = search_detailed(package).await {
            if let Some(d) = detailed.into_iter().find(|p| p.name == info.name) {
                println!(
                    "  {} {}",
                    style::dim("URL:"),
                    style::url(&d.url.unwrap_or_default())
                );
                println!("  {} {:.2} MB", style::dim("Popularity:"), d.popularity);
                if let Some(license) = d.license {
                    if !license.is_empty() {
                        println!("  {} {}", style::dim("License:"), license.join(", "));
                    }
                }
            }
        }

        println!(
            "\n  {} {}",
            style::success("Source:"),
            style::warning("Arch User Repository (AUR)")
        );
        return Ok(());
    }

    println!(
        "{} Package '{}' not found in official repos or AUR.",
        style::error("Error:"),
        style::package(package)
    );
    Ok(())
}

/// Helper to display detailed info from daemon (Buffered)
fn display_detailed_info_buffered<W: std::io::Write>(
    out: &mut W,
    info: &crate::daemon::protocol::DetailedPackageInfo,
) -> Result<()> {
    writeln!(
        out,
        "{} {}",
        style::package(&info.name),
        style::version(&info.version)
    )?;
    writeln!(out, "  {} {}", style::dim("Description:"), info.description)?;
    let source_label = if info.source == "official" {
        format!("Official repository ({})", style::info(&info.repo))
    } else {
        style::warning("AUR (Arch User Repository)")
    };
    writeln!(out, "  {} {}", style::dim("Source:"), source_label)?;
    writeln!(out, "  {} {}", style::dim("URL:"), style::url(&info.url))?;
    writeln!(
        out,
        "  {} {:.2} MB",
        style::dim("Size:"),
        info.size as f64 / 1024.0 / 1024.0
    )?;
    writeln!(
        out,
        "  {} {:.2} MB",
        style::dim("Download:"),
        info.download_size as f64 / 1024.0 / 1024.0
    )?;
    if !info.licenses.is_empty() {
        write!(out, "  {} ", style::dim("License:"))?;
        for (i, license) in info.licenses.iter().enumerate() {
            if i > 0 {
                write!(out, ", ")?;
            }
            write!(out, "{license}")?;
        }
        writeln!(out)?;
    }
    if !info.depends.is_empty() {
        write!(out, "  {} ", style::dim("Depends:"))?;
        for (i, dep) in info.depends.iter().enumerate() {
            if i > 0 {
                write!(out, ", ")?;
            }
            write!(out, "{dep}")?;
        }
        writeln!(out)?;
    }
    if !info.depends.is_empty() {
        writeln!(
            out,
            "  {} {}",
            style::dim("Depends:"),
            info.depends.join(", ")
        )?;
    }
    Ok(())
}

pub async fn info(package: &str) -> Result<()> {
    // Try sync path first
    if info_sync(package)? {
        return Ok(());
    }

    // 3. Try AUR directly as final fallback
    println!(
        "{} Package info for '{}':\n",
        style::header("OMG"),
        style::package(package)
    );
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
        style::error(&format!("Package '{package}' not found"))
    );
    Ok(())
}

/// Clean up orphans and caches
#[allow(clippy::fn_params_excessive_bools)]
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
                println!("{}", style::error(&format!("Failed to clear cache: {e}")));
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

/// List explicitly installed packages (Synchronous)
pub fn explicit_sync(count: bool) -> Result<()> {
    // Try daemon first
    let packages = if let Ok(mut client) = DaemonClient::connect_sync() {
        if let Ok(ResponseResult::Explicit(res)) = client.call_sync(Request::Explicit { id: 0 }) {
            res.packages
        } else {
            // Sequential fallback to local ALPM
            crate::package_managers::list_explicit_fast().unwrap_or_default()
        }
    } else {
        crate::package_managers::list_explicit_fast().unwrap_or_default()
    };

    use std::io::Write;
    let mut stdout = std::io::BufWriter::new(std::io::stdout());

    if count {
        writeln!(
            stdout,
            "{} {} packages",
            style::success("Total:"),
            packages.len()
        )?;
        stdout.flush()?;
        return Ok(());
    }

    writeln!(
        stdout,
        "{} Explicitly installed packages:\n",
        style::header("OMG")
    )?;

    for pkg in &packages {
        writeln!(stdout, "  {}", style::package(pkg))?;
    }

    writeln!(
        stdout,
        "\n{} {} packages",
        style::success("Total:"),
        packages.len()
    )?;
    stdout.flush()?;
    Ok(())
}

/// List explicitly installed packages (Async fallback)
pub async fn explicit(count: bool) -> Result<()> {
    // Just call sync version for now as it's already fast and safe
    explicit_sync(count)
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

/// Fuzzy match candidate for "Did you mean?"
fn fuzzy_suggest(query: &str) -> Option<String> {
    // 1. Get all names (Fast from local ALPM)
    let names = crate::package_managers::alpm_direct::list_all_package_names().ok()?;

    // 2. Open DB for engine (Dummy open just to satisfy constructor)
    let db_path = Database::default_path().ok()?;
    let db = Database::open(&db_path).ok()?;
    let engine = CompletionEngine::new(db);

    // 3. Fuzzy Match
    let matches = engine.fuzzy_match(query, names);

    matches.first().cloned()
}
