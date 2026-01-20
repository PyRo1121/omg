//! Search functionality for packages

use anyhow::Result;
use dialoguer::{MultiSelect, theme::ColorfulTheme};

use crate::cli::style;
use crate::core::client::DaemonClient;
use crate::daemon::protocol::{Request, ResponseResult};

use super::common::{truncate, use_debian_backend};
use super::install::install;

#[cfg(feature = "arch")]
use crate::package_managers::{AurClient, search_detailed, search_sync};

#[cfg(feature = "debian")]
use crate::package_managers::apt_search_sync;

/// Search for packages in official repos and AUR (Synchronous fast-path)
pub fn search_sync_cli(query: &str, detailed: bool, interactive: bool) -> Result<bool> {
    // Daemon path works for both Arch and Debian - provides cached searches
    if detailed || interactive {
        // Fallback to async for these modes as they require spin-up or complex interaction
        return Ok(false);
    }

    let start = std::time::Instant::now();

    // 1. Try Daemon first (ULTRA FAST - <1ms)
    if let Ok(mut client) = DaemonClient::connect_sync()
        && let Ok(ResponseResult::Search(res)) = client.call_sync(&Request::Search {
            id: 0,
            query: query.to_string(),
            limit: Some(50),
        })
    {
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

        // Track usage
        crate::core::usage::track_search();

        return Ok(true);
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

    let mut official_packages: Vec<crate::package_managers::SyncPackage> = Vec::new();
    // AUR variables - typed differently per feature
    #[cfg(feature = "arch")]
    let mut aur_packages_detailed: Option<Vec<crate::package_managers::AurPackageDetail>> = None;
    #[cfg(feature = "arch")]
    let mut aur_packages_basic: Option<Vec<crate::core::Package>> = None;
    // Debian doesn't use AUR, but variables must exist for code structure
    #[cfg(not(feature = "arch"))]
    let aur_packages_detailed: Option<Vec<crate::core::Package>> = None;
    #[cfg(not(feature = "arch"))]
    let mut aur_packages_basic: Option<Vec<crate::core::Package>> = None;

    #[cfg(not(feature = "arch"))]
    let _ = &aur_packages_detailed; // Suppress unused warning

    let mut daemon_used = false;
    if !use_debian_backend() {
        // 1. Try Daemon (Ultra Fast, Cached, Pooled)
        if let Ok(mut client) = DaemonClient::connect().await
            && let Ok(res) = client.search(query, Some(50)).await
        {
            daemon_used = true;
            let mut aur_basic = Vec::new();

            for pkg in res.packages {
                if pkg.source == "official" {
                    official_packages.push(crate::package_managers::SyncPackage {
                        name: pkg.name,
                        version: crate::package_managers::parse_version_or_zero(&pkg.version),
                        description: pkg.description,
                        repo: "official".to_string(),
                        download_size: 0,
                        installed: false,
                    });
                } else {
                    aur_basic.push(crate::core::Package {
                        name: pkg.name,
                        version: crate::package_managers::parse_version_or_zero(&pkg.version),
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

    if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            official_packages = apt_search_sync(query).unwrap_or_default();
        }
    } else if !daemon_used {
        // 2. Fallback: Direct libalpm query + Network
        #[cfg(feature = "arch")]
        {
            official_packages = search_sync(query).unwrap_or_default();
        }

        // Search AUR (Arch only)
        #[cfg(feature = "arch")]
        {
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
                style::version(&pkg.version.to_string()),
                status,
                style::info(&pkg.repo),
                style::dim(&truncate(&pkg.description, 40))
            ));
            pkgs_to_install.push(pkg.name.clone());
        }

        // Add AUR (Arch only)
        #[cfg(feature = "arch")]
        {
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
                        style::version(&pkg.version.to_string()),
                        style::warning("AUR"),
                        style::dim(&truncate(&pkg.description, 40))
                    ));
                    pkgs_to_install.push(pkg.name.clone());
                }
            }
        }
        #[cfg(not(feature = "arch"))]
        {
            let _ = &aur_packages_detailed;
            let _ = &aur_packages_basic;
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
            .filter_map(|i| pkgs_to_install.get(i).cloned())
            .collect();

        install(&selected_names, false).await
    } else {
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
                    style::version(&pkg.version.to_string()),
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

        // Search AUR (cached result) - Arch only
        #[cfg(feature = "arch")]
        {
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
            } else if let Some(aur_packages) = aur_packages_basic
                && !aur_packages.is_empty()
            {
                println!("{}", style::header("AUR (Arch User Repository)"));
                for pkg in aur_packages.iter().take(10) {
                    println!(
                        "  {} {} - {}",
                        style::package(&pkg.name),
                        style::version(&pkg.version.to_string()),
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
        #[cfg(not(feature = "arch"))]
        {
            let _ = aur_packages_detailed;
            let _ = aur_packages_basic;
        }

        println!(
            "{} {}",
            style::arrow("Use"),
            style::command("omg info <package> for details")
        );

        Ok(())
    }
}
