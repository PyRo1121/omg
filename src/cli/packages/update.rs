//! Update functionality for packages

use anyhow::Result;
#[cfg(feature = "arch")]
use dialoguer::{Confirm, theme::ColorfulTheme};
#[cfg(feature = "arch")]
use futures::StreamExt;
#[cfg(feature = "arch")]
use owo_colors::OwoColorize;

use crate::cli::style;
#[cfg(feature = "arch")]
use crate::core::history::PackageChange;

use crate::package_managers::get_package_manager;

#[cfg(feature = "arch")]
use super::common::log_transaction;

#[cfg(feature = "arch")]
use crate::package_managers::{AurClient, check_updates_cached};

/// Update all packages
pub async fn update(check_only: bool) -> Result<()> {
    let pm = get_package_manager();

    if pm.name() == "apt" {
        if check_only {
            #[cfg(feature = "debian")]
            {
                let updates = crate::package_managers::apt_list_updates().unwrap_or_default();
                if updates.is_empty() {
                    println!("{} System is up to date!", style::success("✓"));
                } else {
                    println!(
                        "{} Found {} update(s):",
                        style::header("OMG"),
                        style::info(&updates.len().to_string())
                    );
                    for (name, old_ver, new_ver) in updates {
                        println!(
                            "  {} {} {} → {}",
                            style::package(&name),
                            style::dim(&old_ver),
                            style::arrow("→"),
                            style::version(&new_ver)
                        );
                    }
                }
            }
            return Ok(());
        }
        return pm.update().await;
    }

    #[cfg(not(feature = "arch"))]
    {
        anyhow::bail!("Update not implemented for this backend - use debian backend");
    }

    #[cfg(feature = "arch")]
    update_arch(check_only, &*pm).await
}

#[cfg(feature = "arch")]
async fn update_arch(check_only: bool, pm: &dyn crate::package_managers::PackageManager) -> Result<()> {
    use crate::core::history::TransactionType;
    let aur = AurClient::new();

    // STEP 1: Sync databases first to get latest info
    if !crate::core::is_root() {
            println!(
                "{} Synchronizing databases (elevation might be required)...",
                style::arrow("→")
            );
        }

        // Specialized sync for Arch (parallel)
        crate::package_managers::sync_databases_parallel().await?;

        let pb = style::spinner("Checking for updates...");

        // STEP 2: Get both official and AUR updates IN PARALLEL
        let (official_updates_raw, aur_updates) = tokio::join!(
            async { check_updates_cached().unwrap_or_default() },
            async { aur.get_update_list().await.unwrap_or_default() }
        );

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
                            "MAJOR".red().bold().to_string()
                        } else if new.minor > old.minor {
                            "minor".yellow().bold().to_string()
                        } else {
                            "patch".green().bold().to_string()
                        }
                    }
                    _ => "update".dimmed().to_string(),
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
            .map(|(n, o, n2, _, _, _)| (n.clone(), o.to_string(), n2.to_string()))
            .collect();

        let aur_display: Vec<(String, String, String)> = aur_updates
            .iter()
            .map(|(n, o, n2)| (n.clone(), o.to_string(), n2.to_string()))
            .collect();

        display_list(&official_display, "repo");
        display_list(&aur_display, "aur");

        let mut changes = Vec::new();
        for (name, old_ver, new_ver, _, _, _) in &official_updates_raw {
            changes.push(PackageChange {
                name: name.clone(),
                old_version: Some(old_ver.to_string()),
                new_version: Some(new_ver.to_string()),
                source: "official".to_string(),
            });
        }
        for (name, old_ver, new_ver) in &aur_updates {
            changes.push(PackageChange {
                name: name.clone(),
                old_version: Some(old_ver.to_string()),
                new_version: Some(new_ver.to_string()),
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
            println!(
                "{}",
                style::dim("Proceeding without prompt (no TTY detected).")
            );
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
                pm.install(&pkg_paths_str).await?;
            }
        }

        if !aur_updates.is_empty() {
            let total = aur_updates.len();
            println!(
                "\n{} Building {} AUR package{}...\n",
                style::arrow("→"),
                total,
                if total == 1 { "" } else { "s" }
            );

            // PHASE 1: Clone/update ALL packages in parallel (FAST!)
            println!("{} Fetching PKGBUILDs...", style::dim("→"));
            let aur_names: Vec<String> = aur_updates.iter().map(|(n, _, _)| n.clone()).collect();

            // Parallel git operations - use all cores for network I/O
            let git_concurrency = std::thread::available_parallelism()
                .map(std::num::NonZero::get)
                .unwrap_or(8)
                .max(16); // At least 16 for network I/O bound tasks
            let clone_tasks: Vec<_> = aur_names
                .iter()
                .map(|name| {
                    let aur = aur.clone();
                    let name = name.clone();
                    async move {
                        let pkg_dir = crate::core::paths::cache_dir().join("aur").join(&name);
                        let pkgbuild_path = pkg_dir.join("PKGBUILD");

                        if pkg_dir.exists() && pkgbuild_path.exists() {
                            let _ = aur.git_pull_public(&pkg_dir).await;
                        } else {
                            if pkg_dir.exists() {
                                std::fs::remove_dir_all(&pkg_dir).ok();
                            }
                            let _ = aur.git_clone_public(&name).await;
                        }
                        (name, pkgbuild_path)
                    }
                })
                .collect();

            // Use buffered stream for controlled concurrency
            let mut clone_stream =
                futures::stream::iter(clone_tasks).buffer_unordered(git_concurrency);
            let mut clone_results = Vec::new();
            while let Some(result) = clone_stream.next().await {
                clone_results.push(result);
            }
            println!(
                "{} Fetched {} PKGBUILDs",
                style::success("✓"),
                clone_results.len()
            );

            // PHASE 2: Parse ALL PKGBUILDs in parallel using rayon
            println!("{} Resolving dependencies...", style::dim("→"));
            let parse_results: Vec<_> = clone_results
                .iter()
                .filter_map(|(_name, pkgbuild_path)| {
                    crate::package_managers::pkgbuild::PkgBuild::parse(pkgbuild_path).ok()
                })
                .collect();

            let mut all_makedeps: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            let mut all_deps: std::collections::HashSet<String> = std::collections::HashSet::new();

            for pkgbuild in &parse_results {
                for dep in &pkgbuild.depends {
                    let dep_name = dep.split(['>', '<', '=']).next().unwrap_or(dep);
                    all_deps.insert(dep_name.to_string());
                }
                for dep in &pkgbuild.makedepends {
                    let dep_name = dep.split(['>', '<', '=']).next().unwrap_or(dep);
                    all_makedeps.insert(dep_name.to_string());
                }
                for dep in &pkgbuild.checkdepends {
                    let dep_name = dep.split(['>', '<', '=']).next().unwrap_or(dep);
                    all_makedeps.insert(dep_name.to_string());
                }
            }

            // Filter out already installed packages
            let installed: std::collections::HashSet<String> =
                crate::package_managers::list_installed_fast()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|p| p.name)
                    .collect();

            let makedeps_to_install: Vec<String> = all_makedeps
                .into_iter()
                .filter(|d| !installed.contains(d) && !aur_names.contains(d))
                .collect();

            let deps_to_install: Vec<String> = all_deps
                .into_iter()
                .filter(|d| {
                    !installed.contains(d)
                        && !aur_names.contains(d)
                        && !makedeps_to_install.contains(d)
                })
                .collect();

            // PHASE 3: Install ALL dependencies in ONE transaction (FAST!)
            let total_deps = makedeps_to_install.len() + deps_to_install.len();
            if total_deps > 0 {
                println!(
                    "{} Installing {} dependencies...",
                    style::arrow("→"),
                    total_deps
                );

                let mut all_to_install = makedeps_to_install.clone();
                all_to_install.extend(deps_to_install);

                let status = tokio::process::Command::new("sudo")
                    .arg("pacman")
                    .arg("-S")
                    .arg("--needed")
                    .arg("--noconfirm")
                    .arg("--asdeps")
                    .args(&all_to_install)
                    .status()
                    .await;

                if let Ok(s) = status
                    && s.success()
                {
                    println!("{} Dependencies installed", style::success("✓"));
                }
            }

            // PHASE 4: Build packages in parallel (deps already installed)
            // Use all available cores - modern CPUs like i9-14900K have 32 threads
            let mut built_packages: Vec<(String, String, std::path::PathBuf)> = Vec::new();
            let mut failed_builds: Vec<(String, String)> = Vec::new();
            let concurrency = aur.build_concurrency().clamp(1, 32);

            let mut stream = futures::stream::iter(aur_updates.into_iter().enumerate())
                .map(|(i, (name, _old_ver, new_ver))| {
                    let aur = aur.clone();
                    async move {
                        println!(
                            "{} [{}/{}] Building {}...",
                            style::arrow("→"),
                            i + 1,
                            total,
                            style::package(&name)
                        );
                        let res = aur.build_only(&name).await;
                        (name, new_ver, res)
                    }
                })
                .buffer_unordered(concurrency);

            while let Some((name, new_ver, res)) = stream.next().await {
                match res {
                    Ok(pkg_path) => {
                        println!("  {} Built {}", style::success("✓"), style::package(&name));
                        built_packages.push((name, new_ver.to_string(), pkg_path));
                    }
                    Err(e) => {
                        println!("  {} Failed: {}", style::error("✗"), style::package(&name));
                        failed_builds.push((name, e.to_string()));
                    }
                }
            }

            // Phase 2: Install all built packages in a single transaction
            if !built_packages.is_empty() {
                println!(
                    "\n{} Installing {} built packages...",
                    style::arrow("→"),
                    built_packages.len()
                );

                let pkg_paths: Vec<_> = built_packages.iter().map(|(_, _, p)| p.clone()).collect();

                // Build args for run_self_sudo
                let mut args = vec!["install".to_string()];
                for path in &pkg_paths {
                    args.push(path.display().to_string());
                }
                let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

                match crate::core::privilege::run_self_sudo(&args_refs).await {
                    Ok(_) => {
                        println!(
                            "{} Installed {} packages",
                            style::success("✓"),
                            built_packages.len()
                        );
                    }
                    Err(_) => {
                        println!(
                            "{} Batch install failed, trying individually...",
                            style::warning("⚠")
                        );
                        // Fallback to individual installs
                        for (name, _ver, path) in &built_packages {
                            let path_str = path.display().to_string();
                            let individual_args = ["install", &path_str];
                            match crate::core::privilege::run_self_sudo(&individual_args).await {
                                Ok(_) => {
                                    println!("  {} Installed {}", style::success("✓"), name);
                                }
                                Err(_) => {
                                    println!(
                                        "  {} Failed: {}",
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
    log_transaction(TransactionType::Update, changes, true);

    Ok(())
}
