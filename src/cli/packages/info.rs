//! Info/display functionality for packages

use anyhow::Result;

use crate::cli::tea::run_info_elm;
use crate::cli::{style, ui};
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

    // 1. Try daemon first (ULTRA FAST - <1ms)
    if let Ok(mut client) = DaemonClient::connect_sync()
        && let Ok(info) = client.info_sync(package)
    {
        ui::print_header("OMG", "Package Information");
        ui::print_spacer();

        display_detailed_info(&info);

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
        ui::print_error(format!(
            "Package '{package}' not found in official repos or AUR."
        ));
        return Ok(());
    };

    ui::print_header("OMG", "AUR Package Information");
    ui::print_spacer();

    ui::print_kv("Name", &style::package(&info.name));
    ui::print_kv("Version", &style::version(&info.version.to_string()));
    ui::print_kv("Description", &info.description);

    // Query detailed info for better UX
    if let Ok(detailed) = search_detailed(package).await
        && let Some(d) = detailed.into_iter().find(|p| p.name == info.name)
    {
        ui::print_kv("URL", &style::url(d.url.as_deref().unwrap_or_default()));
        ui::print_kv("Popularity", &format!("{:.2}", d.popularity));
        if let Some(license) = d.license
            && !license.is_empty()
        {
            ui::print_kv("License", &license.join(", "));
        }
    }

    ui::print_spacer();
    ui::print_warning("Source: Arch User Repository (AUR)");
    ui::print_spacer();
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

/// Helper to display detailed info from daemon
fn display_detailed_info(info: &crate::daemon::protocol::DetailedPackageInfo) {
    ui::print_kv("Name", &style::package(&info.name));
    ui::print_kv("Version", &style::version(&info.version));
    ui::print_kv("Description", &info.description);

    let source_label = if info.source == "official" {
        format!("Official repository ({})", style::info(&info.repo))
    } else {
        style::warning("AUR (Arch User Repository)")
    };
    ui::print_kv("Source", &source_label);
    ui::print_kv("URL", &style::url(&info.url));
    ui::print_kv("Size", &style::size(info.size));
    ui::print_kv("Download", &style::size(info.download_size));

    if !info.licenses.is_empty() {
        ui::print_kv("License", &info.licenses.join(", "));
    }
    if !info.depends.is_empty() {
        ui::print_kv("Depends", &info.depends.join(", "));
    }
}

pub async fn info(package: &str) -> Result<()> {
    // Try modern Elm UI first
    if let Err(e) = run_info_elm(package.to_string()) {
        eprintln!("Warning: Elm UI failed, falling back to basic mode: {e}");
        info_fallback(package).await
    } else {
        Ok(())
    }
}

#[allow(clippy::unused_async)] // Contains .await in arch feature block only
async fn info_fallback(package: &str) -> Result<()> {
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
        ui::print_header("OMG", &format!("Package info for '{package}'"));
        ui::print_spacer();

        let pb = style::spinner("Searching AUR...");
        let details: Vec<crate::package_managers::AurPackageDetail> =
            search_detailed(package).await.unwrap_or_default();
        pb.finish_and_clear();

        let Some(pkg) = details.into_iter().find(|p| p.name == package) else {
            ui::print_error(format!("Package '{package}' not found"));
            return Ok(());
        };

        ui::print_kv("Name", &style::package(&pkg.name));
        ui::print_kv("Version", &style::version(&pkg.version));
        ui::print_kv(
            "Description",
            pkg.description.as_deref().unwrap_or_default(),
        );
        ui::print_kv("Maintainer", pkg.maintainer.as_deref().unwrap_or("orphan"));
        ui::print_kv("Votes", &pkg.num_votes.to_string());
        ui::print_kv("Popularity", &format!("{:.2}%", pkg.popularity));
        if pkg.out_of_date.is_some() {
            ui::print_kv("Status", &style::error("OUT OF DATE"));
        }

        ui::print_spacer();
        ui::print_warning("Source: Arch User Repository (AUR)");
        ui::print_spacer();
    }
    Ok(())
}

/// Display package info (debian only)
#[cfg(feature = "debian")]
fn display_package_info(info: &crate::package_managers::types::PackageInfo) {
    ui::print_kv("Package", &style::package(&info.name));
    ui::print_kv("Version", &style::version(&info.version.clone()));
    ui::print_kv(
        "Status",
        if info.installed {
            "installed"
        } else {
            "not installed"
        },
    );
    ui::print_kv("Description", &info.description);
    if let Some(url) = &info.url {
        ui::print_kv("URL", url);
    }
    if let Some(size) = info.install_size {
        ui::print_kv("Install Size", &format!("{size} bytes"));
    }
    if !info.depends.is_empty() {
        ui::print_kv("Depends", &info.depends.join(", "));
    }
}
