//! Info/display functionality for packages

use anyhow::Result;

use crate::cli::style;
use crate::core::client::DaemonClient;
use crate::core::env::distro::use_debian_backend;
use crate::package_managers::get_package_manager;

#[cfg(feature = "arch")]
use crate::package_managers::{AurClient, search_detailed};

/// Show package information (Synchronous fast-path)
/// Alias for CLI fast path
pub fn info_sync_cli(package: &str) -> Result<bool> {
    info_sync(package)
}

/// Show package information (Synchronous fast-path)
pub fn info_sync(package: &str) -> Result<bool> {
    // SECURITY: Validate package name
    if let Err(e) = crate::core::security::validate_package_name(package) {
        anyhow::bail!("Invalid package name: {e}");
    }

    let pm = get_package_manager();
    let pm_name = pm.name();

    let start = std::time::Instant::now();

    // 1. Try daemon first (ULTRA FAST - <1ms)
    if let Ok(mut client) = DaemonClient::connect_sync()
        && let Ok(info) = client.info_sync(package)
    {
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

        // Track usage
        crate::core::usage::track_info();

        return Ok(true);
    }

    // 2. Fallback to local package manager (Sync-like via block_on if needed, but here we just use the backend functions)
    if pm_name == "apt" {
        #[cfg(feature = "debian")]
        {
            if let Some(info) = crate::package_managers::apt_get_sync_pkg_info(package)
                .ok()
                .flatten()
            {
                display_package_info(&info);
                println!(
                    "\n  {} Official repository ({})",
                    style::success("Source:"),
                    style::info("apt")
                );
                return Ok(true);
            }
        }
    } else if pm_name == "pacman" {
        #[cfg(feature = "arch")]
        {
            if let Some(info) = crate::package_managers::get_sync_pkg_info(package)
                .ok()
                .flatten()
            {
                crate::package_managers::display_pkg_info(&info);
                println!(
                    "\n  {} Official repository ({})",
                    style::success("Source:"),
                    style::info(&info.repo)
                );

                // Track usage
                crate::core::usage::track_info();

                return Ok(true);
            }
        }
    }

    Ok(false)
}

/// Show AUR package information (Async fallback) - Arch only
#[cfg(feature = "arch")]
pub async fn info_aur(package: &str) -> Result<()> {
    let aur = AurClient::new();
    let Some(info) = aur.info(package).await? else {
        println!(
            "{} Package '{}' not found in official repos or AUR.",
            style::error("Error:"),
            style::package(package)
        );
        return Ok(());
    };

    // Display beautified info
    let ver = info.version.to_string();
    let source_str = info.source.to_string();
    println!(
        "  {} {} ({})",
        style::package(&info.name),
        style::version(&ver),
        style::info(&source_str)
    );
    println!("  {} {}", style::dim("Description:"), info.description);

    // Query detailed info for better UX
    if let Ok(detailed) = search_detailed(package).await
        && let Some(d) = detailed.into_iter().find(|p| p.name == info.name)
    {
        println!(
            "  {} {}",
            style::dim("URL:"),
            style::url(d.url.as_deref().unwrap_or_default())
        );
        println!("  {} {:.2} MB", style::dim("Popularity:"), d.popularity);
        if let Some(license) = d.license
            && !license.is_empty()
        {
            println!("  {} {}", style::dim("License:"), license.join(", "));
        }
    }

    println!(
        "\n  {} {}",
        style::success("Source:"),
        style::warning("Arch User Repository (AUR)")
    );
    Ok(())
}

/// Show AUR package information - no-op fallback for non-Arch systems
#[cfg(not(feature = "arch"))]
pub async fn info_aur(package: &str) -> Result<()> {
    println!(
        "{} AUR is not available on this system.",
        style::error("Error:")
    );
    let _ = package;
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

    if use_debian_backend() {
        println!(
            "{} Package '{}' not found in apt repositories.",
            style::error("Error:"),
            style::package(package)
        );
        return Ok(());
    }

    // 3. Try AUR directly as final fallback (Arch only)
    #[cfg(feature = "arch")]
    {
        println!(
            "{} Package info for '{}':\n",
            style::header("OMG"),
            style::package(package)
        );
        let pb = style::spinner("Searching AUR...");
        let details: Vec<crate::package_managers::AurPackageDetail> =
            search_detailed(package).await.unwrap_or_default();
        pb.finish_and_clear();

        let Some(pkg) = details.into_iter().find(|p| p.name == package) else {
            println!(
                "{}",
                style::error(&format!("Package '{package}' not found"))
            );
            return Ok(());
        };

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
            pkg.description.as_deref().unwrap_or_default()
        );
        println!(
            "  {} {}",
            style::warning("Maintainer:"),
            pkg.maintainer.as_deref().unwrap_or("orphan")
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
    }
    Ok(())
}

/// Display package info (debian only)
#[cfg(feature = "debian")]
fn display_package_info(info: &crate::package_managers::types::PackageInfo) {
    println!(
        "{} {}",
        style::header("Package:"),
        style::package(&info.name)
    );
    println!(
        "  {} {}",
        style::dim("Version:"),
        style::version(&info.version.clone())
    );
    println!(
        "  {} {}",
        style::dim("Status:"),
        if info.installed {
            style::success("installed")
        } else {
            style::error("not installed")
        }
    );
    println!("  {} {}", style::dim("Description:"), info.description);
    if let Some(url) = &info.url {
        println!("  {} {}", style::dim("URL:"), url);
    }
    if let Some(size) = info.install_size {
        println!("  {} {} bytes", style::dim("Install Size:"), size);
    }
    if !info.depends.is_empty() {
        println!("  {} {}", style::dim("Depends:"), info.depends.join(", "));
    }
}
