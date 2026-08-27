//! Info/display functionality for packages

use anyhow::{Context, Result};
use std::time::Duration;

use crate::cli::tea::run_info_elm;
use crate::cli::{style, ui};
#[cfg(unix)]
use crate::core::PackageSource;
#[cfg(unix)]
use crate::core::client::DaemonClient;
#[cfg(any(feature = "debian", feature = "debian-pure"))]
use crate::core::env::distro::is_debian_like;
use crate::package_managers::get_package_manager;

#[cfg(feature = "arch")]
use crate::package_managers::{AurClient, search_detailed};

const DAEMON_INFO_TIMEOUT: Duration = Duration::from_secs(3);
#[cfg(feature = "arch")]
const AUR_INFO_TIMEOUT: Duration = Duration::from_secs(8);

/// Show package information (Synchronous fast-path)
pub fn info_sync(package: &str) -> Result<bool> {
    // SECURITY: Validate package name
    if let Err(e) = crate::core::security::validate_package_name(package) {
        anyhow::bail!("Invalid package name: {e}");
    }

    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if is_debian_like() {
        if let Some(pkg) = crate::package_managers::debian_db::get_info_fast(package)? {
            ui::print_kv("Name", &style::package(&pkg.name));
            // debian_db stores parse_version_or_zero output, whose concrete
            // type follows the feature flags (AlpmVersion under arch,
            // DebVersion/String otherwise). Normalize to a display string.
            #[cfg(feature = "arch")]
            let version = pkg.version.to_string();
            #[cfg(not(feature = "arch"))]
            let version = pkg.version.clone();
            ui::print_kv("Version", &style::version(&version));
            ui::print_kv(
                "Description",
                &style::sanitize_terminal_text(&pkg.description),
            );
            ui::print_kv(
                "Status",
                if pkg.installed {
                    "installed"
                } else {
                    "not installed"
                },
            );
            ui::print_kv(
                "Source",
                &format!("Official repository ({})", style::info("apt")),
            );
            return Ok(true);
        }
        #[cfg(feature = "debian")]
        {
            if let Some(info) = crate::package_managers::apt_get_sync_pkg_info(package)? {
                display_package_info(&info);
                ui::print_kv(
                    "Source",
                    &format!("Official repository ({})", style::info("apt")),
                );
                return Ok(true);
            }
        }
        return Ok(false);
    }

    // 1. Try daemon first (ULTRA FAST - <1ms)
    #[cfg(unix)]
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

    let pm = get_package_manager()?;

    if pm.name() == "pacman" {
        #[cfg(feature = "arch")]
        {
            if let Some(info) = crate::package_managers::get_sync_pkg_info(package)
                .with_context(|| format!("Failed to look up {package} in official repositories"))?
            {
                crate::package_managers::display_pkg_info(&info);
                ui::print_kv(
                    "Source",
                    &format!("Official repository ({})", style::info(&info.repo)),
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
    let aur = AurClient::new()?;
    let Some(info) = aur.info(package).await? else {
        anyhow::bail!("Package '{package}' not found. Try: omg search {package}");
    };

    ui::print_header("OMG", "AUR Package Information");
    ui::print_spacer();

    ui::print_kv("Name", &style::package(&info.name));
    ui::print_kv("Version", &style::version(&info.version.to_string()));
    ui::print_kv(
        "Description",
        &style::sanitize_terminal_text(&info.description),
    );

    // Query detailed info for better UX
    if let Ok(detailed) = search_detailed(package).await
        && let Some(d) = detailed.into_iter().find(|p| p.name == info.name)
    {
        ui::print_kv(
            "URL",
            &style::url(&style::sanitize_terminal_text(
                d.url.as_deref().unwrap_or_default(),
            )),
        );
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

/// AUR information stub for builds without the Arch backend.
///
/// The async signature is preserved so callers compile uniformly across
/// feature combinations, but no AUR lookup can be performed here.
#[cfg(not(feature = "arch"))]
#[allow(
    clippy::unused_async,
    reason = "the non-Arch implementation preserves the asynchronous command interface"
)]
pub async fn info_aur(package: &str) -> Result<()> {
    anyhow::bail!("AUR information requires an Arch-enabled build; '{package}' was not resolved");
}

/// Helper to display detailed info from daemon
#[cfg(unix)]
fn display_detailed_info(info: &crate::daemon::protocol::DetailedPackageInfo) {
    ui::print_kv("Name", &style::package(&info.name));
    ui::print_kv("Version", &style::version(&info.version));
    ui::print_kv(
        "Description",
        &style::sanitize_terminal_text(&info.description),
    );

    let source_label = if PackageSource::from_label(&info.source) == Some(PackageSource::Official) {
        format!("Official repository ({})", style::info(&info.repo))
    } else {
        style::warning("AUR (Arch User Repository)")
    };
    ui::print_kv("Source", &source_label);
    ui::print_kv(
        "URL",
        &style::url(&style::sanitize_terminal_text(&info.url)),
    );
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
    info_with_json(package, false).await
}

pub async fn info_with_json(package: &str, json: bool) -> Result<()> {
    if json {
        return info_json(package).await;
    }

    if crate::core::paths::test_mode() || !console::user_attended() {
        return info_fallback(package).await;
    }

    if let Err(e) = run_info_elm(package.to_string()) {
        if e.kind() == std::io::ErrorKind::Other {
            return Err(e.into());
        }
        tracing::warn!("Elm UI failed, falling back to basic mode: {}", e);
        info_fallback(package).await
    } else {
        Ok(())
    }
}

async fn info_json(package: &str) -> Result<()> {
    if let Err(e) = crate::core::security::validate_package_name(package) {
        anyhow::bail!("Invalid package name: {e}");
    }

    #[cfg(unix)]
    if let Ok(Ok(info)) = tokio::time::timeout(DAEMON_INFO_TIMEOUT, async {
        let mut client = crate::core::client::DaemonClient::connect().await?;
        client.info(package).await
    })
    .await
    {
        let json_str = serde_json::to_string_pretty(&info)
            .context("Failed to serialize package info as JSON")?;
        println!("{json_str}");
        return Ok(());
    }

    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if is_debian_like() {
        let Some(pkg) =
            crate::package_managers::debian_db::get_info_fast(package).with_context(|| {
                format!("Failed to look up {package} in the Debian package database")
            })?
        else {
            anyhow::bail!("Package '{package}' not found");
        };
        let json_obj = serde_json::json!({
            "name": pkg.name,
            "version": pkg.version,
            "description": pkg.description,
            "installed": pkg.installed,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&json_obj)
                .context("Failed to serialize package info as JSON")?
        );
        return Ok(());
    }

    let pm = get_package_manager()?;
    if pm.name() == "pacman" {
        #[cfg(feature = "arch")]
        if let Some(info) = crate::package_managers::get_sync_pkg_info(package)
            .with_context(|| format!("Failed to look up {package} in official repositories"))?
        {
            let json_obj = serde_json::json!({
                "name": info.name,
                "version": info.version.to_string(),
                "description": info.description,
                "url": info.url,
                "size": info.size,
                "install_size": info.install_size,
                "download_size": info.download_size,
                "repo": info.repo,
                "depends": info.depends,
                "licenses": info.licenses,
                "installed": info.installed,
            });
            println!(
                "{}",
                serde_json::to_string_pretty(&json_obj)
                    .context("Failed to serialize package info as JSON")?
            );
            return Ok(());
        }
    }

    anyhow::bail!("Package '{package}' not found")
}

#[allow(
    clippy::unused_async,
    reason = "the Arch feature branch awaits while fallback builds do not"
)]
async fn info_fallback(package: &str) -> Result<()> {
    // Try sync path first
    if info_sync(package)? {
        return Ok(());
    }

    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if is_debian_like() {
        anyhow::bail!("Package '{package}' not found. Try: omg search {package}");
    }

    // 3. Try AUR directly as final fallback (Arch only)
    #[cfg(feature = "arch")]
    {
        ui::print_header("OMG", &format!("Package info for '{package}'"));
        ui::print_spacer();

        let pb = style::spinner("Searching AUR...");
        let details: Vec<crate::package_managers::AurPackageDetail> =
            match tokio::time::timeout(AUR_INFO_TIMEOUT, search_detailed(package)).await {
                Ok(Ok(results)) => results,
                Ok(Err(err)) => {
                    pb.finish_and_clear();
                    return Err(err).context(format!("Failed to look up {package} on the AUR"));
                }
                Err(_) => {
                    pb.finish_and_clear();
                    anyhow::bail!("Timed out looking up {package} on the AUR");
                }
            };
        pb.finish_and_clear();

        let Some(pkg) = details.into_iter().find(|p| p.name == package) else {
            anyhow::bail!("Package '{package}' not found. Try: omg search {package}");
        };

        ui::print_kv("Name", &style::package(&pkg.name));
        ui::print_kv("Version", &style::version(&pkg.version));
        ui::print_kv(
            "Description",
            &style::sanitize_terminal_text(pkg.description.as_deref().unwrap_or_default()),
        );
        ui::print_kv(
            "Maintainer",
            &style::sanitize_terminal_text(pkg.maintainer.as_deref().unwrap_or("orphan")),
        );
        ui::print_kv("Votes", &pkg.num_votes.to_string());
        ui::print_kv("Popularity", &format!("{:.2}%", pkg.popularity));
        if pkg.out_of_date.is_some() {
            ui::print_kv("Status", &style::error("OUT OF DATE"));
        }

        ui::print_spacer();
        ui::print_warning("Source: Arch User Repository (AUR)");
        ui::print_spacer();
        Ok(())
    }

    #[cfg(not(feature = "arch"))]
    anyhow::bail!("Package '{package}' not found. Try: omg search {package}");
}

/// Display package info (debian only)
#[cfg(feature = "debian")]
fn display_package_info(info: &crate::package_managers::types::PackageInfo) {
    ui::print_kv("Package", &style::package(&info.name));
    ui::print_kv("Version", &style::version(&info.version));
    ui::print_kv(
        "Status",
        if info.installed {
            "installed"
        } else {
            "not installed"
        },
    );
    ui::print_kv(
        "Description",
        &style::sanitize_terminal_text(&info.description),
    );
    if let Some(url) = &info.url {
        ui::print_kv("URL", &style::sanitize_terminal_text(url));
    }
    if let Some(size) = info.install_size {
        ui::print_kv("Install Size", &format!("{size} bytes"));
    }
    if !info.depends.is_empty() {
        ui::print_kv("Depends", &info.depends.join(", "));
    }
}
