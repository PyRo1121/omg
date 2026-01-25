//! Search functionality for packages

use anyhow::Result;
use dialoguer::MultiSelect;
use std::io::Write;

use crate::cli::tea::run_search_elm;
use crate::cli::{style, ui};
use crate::core::client::DaemonClient;
use crate::core::env::distro::use_debian_backend;
use crate::daemon::protocol::{Request, ResponseResult};
use crate::package_managers::get_package_manager;

#[cfg(feature = "arch")]
use crate::package_managers::search_sync;

#[cfg(feature = "debian")]
use crate::package_managers::apt_search_sync;

use super::common::truncate;
use super::install::install;

#[cfg(feature = "arch")]
use crate::package_managers::{AurClient, search_detailed};

/// Holds the results from searching packages across different sources
struct SearchResults {
    official_packages: Vec<crate::package_managers::SyncPackage>,
    #[cfg(feature = "arch")]
    aur_packages_detailed: Option<Vec<crate::package_managers::AurPackageDetail>>,
    #[cfg(feature = "arch")]
    aur_packages_basic: Option<Vec<crate::core::Package>>,
    #[cfg(not(feature = "arch"))]
    _phantom: std::marker::PhantomData<()>,
}

/// Display search results with formatting and truncation
fn display_results(
    header: &str,
    packages: &[impl PackageDisplay],
    limit: usize,
    writer: &mut impl Write,
) -> Result<()> {
    writeln!(writer, "\n{}", style::header(header))?;
    for pkg in packages.iter().take(limit) {
        writeln!(writer, "{}", pkg.display_format())?;
    }

    if packages.len() > limit {
        let more = packages.len() - limit;
        writeln!(
            writer,
            "  {}",
            style::dim(&format!("(+{more} more packages...)"))
        )?;
    }

    Ok(())
}

/// Trait for types that can be displayed in search results
trait PackageDisplay {
    fn display_format(&self) -> String;
}

impl PackageDisplay for crate::package_managers::SyncPackage {
    #[allow(clippy::implicit_clone)]
    fn display_format(&self) -> String {
        let installed = if self.installed {
            style::dim(" [installed]")
        } else {
            String::new()
        };
        format!(
            "  {} {} ({}) - {}{}",
            style::package(&self.name),
            style::version(&self.version.to_string()),
            style::info(&self.repo),
            style::dim(&truncate(&self.description, 50)),
            installed
        )
    }
}

impl PackageDisplay for crate::core::Package {
    #[allow(clippy::implicit_clone)]
    fn display_format(&self) -> String {
        format!(
            "  {} {} ({}) - {}",
            style::package(&self.name),
            style::version(&self.version.to_string()),
            style::info(&self.source.to_string()),
            style::dim(&truncate(&self.description, 50))
        )
    }
}

/// Display AUR results with appropriate formatting
#[cfg(feature = "arch")]
fn display_aur_results(
    aur_packages_detailed: Option<&Vec<crate::package_managers::AurPackageDetail>>,
    aur_packages_basic: Option<&Vec<crate::core::Package>>,
    writer: &mut impl Write,
) -> Result<()> {
    if let Some(aur_packages) = aur_packages_detailed {
        if !aur_packages.is_empty() {
            writeln!(writer, "{}", style::header("AUR (Arch User Repository)"))?;
            for pkg in aur_packages.iter().take(10) {
                let out_of_date = if pkg.out_of_date.is_some() {
                    style::error(" [OUT OF DATE]")
                } else {
                    String::new()
                };
                writeln!(
                    writer,
                    "  {} {} - {} {} {}{}",
                    style::package(&pkg.name),
                    style::version(&pkg.version),
                    style::info(&format!("↑{}", pkg.num_votes)),
                    style::info(&format!("{:.1}%", pkg.popularity)),
                    style::dim(&truncate(
                        pkg.description.as_deref().unwrap_or_default(),
                        40
                    )),
                    out_of_date
                )?;
            }
            if aur_packages.len() > 10 {
                writeln!(
                    writer,
                    "  {}",
                    style::dim(&format!("(+{})", aur_packages.len() - 10))
                )?;
            }
            writeln!(writer)?;
        }
    } else if let Some(aur_packages) = aur_packages_basic
        && !aur_packages.is_empty()
    {
        writeln!(writer, "{}", style::header("AUR (Arch User Repository)"))?;
        for pkg in aur_packages.iter().take(10) {
            writeln!(
                writer,
                "  {} {} - {}",
                style::package(&pkg.name),
                style::version(&pkg.version.to_string()),
                style::dim(&truncate(&pkg.description, 55))
            )?;
        }
        if aur_packages.len() > 10 {
            writeln!(
                writer,
                "  {}",
                style::dim(&format!("(+{})", aur_packages.len() - 10))
            )?;
        }
        writeln!(writer)?;
    }
    Ok(())
}

/// Handle interactive package selection and installation
#[allow(clippy::implicit_clone)]
async fn handle_interactive_selection(
    official_packages: &[crate::package_managers::SyncPackage],
    #[cfg(feature = "arch")] aur_packages_detailed: Option<
        &Vec<crate::package_managers::AurPackageDetail>,
    >,
    #[cfg(feature = "arch")] aur_packages_basic: Option<&Vec<crate::core::Package>>,
) -> Result<()> {
    let mut items = Vec::new();
    let mut pkgs_to_install = Vec::new();

    // Add official packages
    for pkg in official_packages {
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

    // Add AUR packages (Arch only)
    #[cfg(feature = "arch")]
    {
        if let Some(aur) = aur_packages_detailed {
            for pkg in aur {
                items.push(format!(
                    "{} {} ({}) - {}",
                    style::package(&pkg.name),
                    style::version(&pkg.version),
                    style::warning("AUR"),
                    style::dim(&truncate(
                        pkg.description.as_deref().unwrap_or_default(),
                        40
                    ))
                ));
                pkgs_to_install.push(pkg.name.clone());
            }
        } else if let Some(aur) = aur_packages_basic {
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

    if items.is_empty() {
        println!("{}", style::error("No packages found"));
        return Ok(());
    }

    println!("{}", style::arrow("Select packages to install:"));

    if !console::user_attended() {
        anyhow::bail!(
            "Interactive mode requires an interactive terminal.\n\
             For automation, use: omg install <package1> <package2> ...\n\
             Example: omg install firefox vim"
        );
    }

    let selections = MultiSelect::with_theme(&crate::cli::ui::prompt_theme())
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
}

/// Fetch packages from all sources (daemon, local, AUR)
async fn fetch_packages(
    query: &str,
    #[allow(unused_variables)] detailed: bool,
    #[allow(unused_variables)] interactive: bool,
) -> SearchResults {
    let mut official_packages: Vec<crate::package_managers::SyncPackage> = Vec::new();
    #[cfg(feature = "arch")]
    let mut aur_packages_detailed: Option<Vec<crate::package_managers::AurPackageDetail>> = None;
    #[cfg(feature = "arch")]
    let mut aur_packages_basic: Option<Vec<crate::core::Package>> = None;

    let mut daemon_used = false;
    if use_debian_backend() {
        // Debian Daemon Search
        if let Ok(mut client) = DaemonClient::connect().await
            && let Ok(res) = client.debian_search(query, Some(50)).await
        {
            daemon_used = true;
            for pkg in res {
                official_packages.push(crate::package_managers::SyncPackage {
                    name: pkg.name,
                    version: crate::package_managers::parse_version_or_zero(&pkg.version),
                    description: pkg.description,
                    repo: "apt".to_string(),
                    download_size: 0,
                    installed: false,
                });
            }
        }
    } else {
        // 1. Try Daemon (Ultra Fast, Cached, Pooled)
        if let Ok(mut client) = DaemonClient::connect().await
            && let Ok(res) = client.search(query, Some(50)).await
        {
            daemon_used = true;
            #[cfg(feature = "arch")]
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
                    #[cfg(feature = "arch")]
                    {
                        aur_basic.push(crate::core::Package {
                            name: pkg.name,
                            version: crate::package_managers::parse_version_or_zero(&pkg.version),
                            description: pkg.description,
                            source: crate::core::PackageSource::Aur,
                            installed: false,
                        });
                    }
                }
            }
            #[cfg(feature = "arch")]
            if !aur_basic.is_empty() {
                aur_packages_basic = Some(aur_basic);
            }
        }
    }

    if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            if !daemon_used {
                official_packages = apt_search_sync(query).unwrap_or_default();
            }
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

    SearchResults {
        official_packages,
        #[cfg(feature = "arch")]
        aur_packages_detailed,
        #[cfg(feature = "arch")]
        aur_packages_basic,
        #[cfg(not(feature = "arch"))]
        _phantom: std::marker::PhantomData,
    }
}

/// Search for packages in official repos and AUR (Synchronous fast-path)
pub fn search_sync_cli(query: &str, detailed: bool, interactive: bool) -> Result<bool> {
    // Daemon path works for both Arch and Debian - provides cached searches
    if detailed || interactive {
        // Fallback to async for these modes as they require spin-up or complex interaction
        return Ok(false);
    }

    let start = std::time::Instant::now();

    // 1. Try Daemon first (ULTRA FAST - <1ms)
    let daemon_res = if let Ok(mut client) = DaemonClient::connect_sync() {
        client
            .call_sync(&Request::Search {
                id: 0,
                query: query.to_string(),
                limit: Some(50),
            })
            .ok()
    } else {
        None
    };

    if let Some(ResponseResult::Search(res)) = daemon_res {
        let sync_time = start.elapsed();

        if res.packages.is_empty() {
            // Use Components for enhanced "no results" message
            use crate::cli::components::Components;
            use crate::cli::packages::execute_cmd;
            execute_cmd(Components::no_results(query));
            return Ok(false);
        }

        let mut stdout = std::io::BufWriter::new(std::io::stdout());

        writeln!(
            stdout,
            "{} {} results ({:.1}ms)\n",
            style::header("OMG"),
            res.packages.len(),
            sync_time.as_secs_f64() * 1000.0
        )?;

        // Convert daemon packages to Package type for display
        let packages: Vec<crate::core::Package> = res
            .packages
            .into_iter()
            .map(|pkg| crate::core::Package {
                name: pkg.name,
                version: crate::package_managers::parse_version_or_zero(&pkg.version),
                description: pkg.description,
                source: if pkg.source == "official" {
                    crate::core::PackageSource::Official
                } else {
                    crate::core::PackageSource::Aur
                },
                installed: false,
            })
            .collect();

        display_results("Official Repositories", &packages, 20, &mut stdout)?;

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

    // 2. Fallback to local search if daemon is not available (mostly for tests)
    let pm = get_package_manager();
    let pm_name = pm.name();

    if pm_name == "pacman" {
        #[cfg(feature = "arch")]
        {
            if let Ok(official_packages) = crate::package_managers::search_sync(query) {
                if official_packages.is_empty() {
                    return Ok(false);
                }

                let sync_time = start.elapsed();
                let mut stdout = std::io::BufWriter::new(std::io::stdout());

                writeln!(
                    stdout,
                    "{} {} results ({:.1}ms)\n",
                    style::header("OMG"),
                    official_packages.len(),
                    sync_time.as_secs_f64() * 1000.0
                )?;

                display_results("Official Repositories", &official_packages, 20, &mut stdout)?;
                stdout.flush()?;
                return Ok(true);
            }
        }
    } else if pm_name == "apt" {
        #[cfg(feature = "debian")]
        {
            if let Ok(official_packages) = crate::package_managers::apt_search_fast(query) {
                if official_packages.is_empty() {
                    return Ok(false);
                }

                let sync_time = start.elapsed();
                let mut stdout = std::io::BufWriter::new(std::io::stdout());

                writeln!(
                    stdout,
                    "{} {} results ({:.1}ms)\n",
                    style::header("OMG"),
                    official_packages.len(),
                    sync_time.as_secs_f64() * 1000.0
                )?;

                display_results("Official Repositories", &official_packages, 20, &mut stdout)?;
                stdout.flush()?;
                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Search for packages in official repos and AUR - LIGHTNING FAST
pub async fn search(query: &str, detailed: bool, interactive: bool) -> Result<()> {
    // SECURITY: Validate search query
    if query.len() > 100 {
        anyhow::bail!("Search query too long (max 100 characters)");
    }
    if query.chars().any(char::is_control) {
        anyhow::bail!("Search query contains invalid characters");
    }

    // Try modern Elm UI first (only for standard search, not interactive yet)
    if !interactive && !detailed {
        if let Err(e) = run_search_elm(query.to_string()) {
            eprintln!("Warning: Elm UI failed, falling back to basic mode: {e}");
            search_fallback(query, detailed, interactive).await
        } else {
            Ok(())
        }
    } else {
        // Interactive/Detailed modes still use the old path for now
        search_fallback(query, detailed, interactive).await
    }
}

async fn search_fallback(query: &str, detailed: bool, interactive: bool) -> Result<()> {
    // Try sync path first
    if search_sync_cli(query, detailed, interactive)? {
        return Ok(());
    }

    let start = std::time::Instant::now();

    // Fetch packages from all sources
    let results = fetch_packages(query, detailed, interactive).await;
    let sync_time = start.elapsed();

    // Handle interactive mode
    if interactive {
        #[cfg(feature = "arch")]
        return handle_interactive_selection(
            &results.official_packages,
            results.aur_packages_detailed.as_ref(),
            results.aur_packages_basic.as_ref(),
        )
        .await;

        #[cfg(not(feature = "arch"))]
        return handle_interactive_selection(&results.official_packages).await;
    }

    // Display official packages first
    if !results.official_packages.is_empty() {
        println!(
            "{} {} results ({:.1}ms)\n",
            style::header("OMG"),
            results.official_packages.len(),
            sync_time.as_secs_f64() * 1000.0
        );

        let mut stdout = std::io::stdout();
        display_results(
            "Official Repositories",
            &results.official_packages,
            20,
            &mut stdout,
        )?;
        println!();
    }

    // Display AUR results (Arch only)
    #[cfg(feature = "arch")]
    {
        let mut stdout = std::io::stdout();
        display_aur_results(
            results.aur_packages_detailed.as_ref(),
            results.aur_packages_basic.as_ref(),
            &mut stdout,
        )?;
    }

    ui::print_spacer();
    ui::print_tip("Use 'omg info <package>' for detailed information.");
    ui::print_spacer();

    Ok(())
}
