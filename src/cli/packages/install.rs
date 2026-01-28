//! Install functionality for packages

use anyhow::Result;
use dialoguer::Select;

use crate::cli::{style, ui};
use crate::core::client::DaemonClient;
use crate::package_managers::get_package_manager;

use futures::future::BoxFuture;

#[cfg(feature = "arch")]
use crate::package_managers::AurClient;

fn extract_missing_package(msg: &str, packages: &[String]) -> Option<String> {
    // Match pattern: "Package {name} not found in any repository" from alpm_ops.rs
    if msg.contains("not found in any repository") || msg.contains("Package not found:") {
        for pkg in packages {
            if msg.contains(pkg.as_str()) {
                return Some(pkg.clone());
            }
        }
    }

    packages.iter().find(|p| msg.contains(p.as_str())).cloned()
}

pub async fn install(packages: &[String], yes: bool, dry_run: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    if dry_run {
        return install_dry_run(packages);
    }

    let pm = get_package_manager();

    ui::print_header("OMG", &format!("Installing {} package(s)", packages.len()));
    ui::print_spacer();

    if let Err(e) = pm.install(packages).await {
        let msg = e.to_string();

        if let Some(pkg_name) = extract_missing_package(&msg, packages) {
            return handle_missing_package(pkg_name, e, yes).await;
        }
        return Err(e);
    }

    crate::core::usage::track_install(packages);
    Ok(())
}

#[allow(clippy::unnecessary_wraps)] // Result return required: API compat with feature-gated impls
fn install_dry_run(packages: &[String]) -> Result<()> {
    ui::print_header("OMG", "Dry Run - Install Preview");
    ui::print_spacer();

    println!(
        "  {} The following packages would be installed:\n",
        style::info("→")
    );

    #[allow(unused_mut)] // Mutated only inside feature-gated block
    let mut total_size: u64 = 0;

    for pkg_name in packages {
        #[cfg(feature = "arch")]
        {
            if let Ok(Some(info)) = crate::package_managers::get_sync_pkg_info(pkg_name) {
                let size_mb = info.download_size.unwrap_or(0) as f64 / 1024.0 / 1024.0;
                total_size += info.download_size.unwrap_or(0);
                println!(
                    "    {} {} {} ({:.2} MB)",
                    style::success("✓"),
                    style::package(&info.name),
                    style::version(&info.version.to_string()),
                    size_mb
                );
                if !info.depends.is_empty() {
                    println!(
                        "      {} Dependencies: {}",
                        style::dim("└"),
                        style::dim(&info.depends.join(", "))
                    );
                }
            } else {
                println!(
                    "    {} {} (not found in repositories, may be AUR)",
                    style::warning("?"),
                    style::package(pkg_name)
                );
            }
        }
        #[cfg(not(feature = "arch"))]
        {
            println!("    {} {}", style::dim("·"), style::package(pkg_name));
        }
    }

    ui::print_spacer();
    println!(
        "  {} Total download size: {:.2} MB",
        style::info("→"),
        total_size as f64 / 1024.0 / 1024.0
    );
    ui::print_dry_run_footer();

    Ok(())
}

fn handle_missing_package(
    pkg_name: String,
    original_error: anyhow::Error,
    yes: bool,
) -> BoxFuture<'static, Result<()>> {
    Box::pin(async move {
        // Try AUR search first (if feature enabled)
        #[cfg(feature = "arch")]
        {
            if let Ok(aur_pkg) = try_aur_package(&pkg_name).await {
                return handle_aur_package(&pkg_name, aur_pkg, yes).await;
            }
        }

        // Fall back to suggestions from official repos
        let suggestions = try_get_suggestions(&pkg_name).await;

        if suggestions.is_empty() {
            return Err(original_error);
        }

        println!();
        ui::print_error(format!("Package '{pkg_name}' not found"));
        println!("Did you mean one of these?");
        println!();

        // Skip interactive prompt when --yes is true
        if !yes && console::user_attended() {
            let selection = Select::with_theme(&ui::prompt_theme())
                .with_prompt("Select a replacement (or Esc to abort)")
                .default(0)
                .items(&suggestions)
                .interact_opt()?;

            if let Some(index) = selection {
                let new_pkg = suggestions[index].clone();
                println!(
                    "{} Replacing '{}' with '{}'\n",
                    style::success("✓"),
                    pkg_name,
                    new_pkg
                );

                return install(&[new_pkg], yes, false).await;
            }
        } else {
            println!("  Suggested alternatives:");
            for (i, suggestion) in suggestions.iter().enumerate().take(5) {
                println!("    {}. {}", i + 1, suggestion);
            }
        }

        Err(original_error)
    })
}

async fn try_get_suggestions(query: &str) -> Vec<String> {
    if let Ok(mut client) = DaemonClient::connect().await
        && let Ok(suggestions) = client.suggest(query, Some(5)).await
    {
        return suggestions;
    }
    Vec::new()
}

#[cfg(feature = "arch")]
async fn try_aur_package(pkg_name: &str) -> Result<crate::core::Package> {
    let aur = AurClient::new();

    let results = aur.search(pkg_name).await?;

    results
        .into_iter()
        .find(|p| p.name == pkg_name)
        .ok_or_else(|| anyhow::anyhow!("Package not found in AUR"))
}

#[cfg(feature = "arch")]
async fn handle_aur_package(
    pkg_name: &str,
    aur_pkg: crate::core::Package,
    yes: bool,
) -> Result<()> {
    println!();
    ui::print_warning(format!(
        "Package '{pkg_name}' not found in official repositories"
    ));
    println!(
        "  {} Found in AUR: {} ({})",
        style::info("→"),
        style::package(&aur_pkg.name),
        style::version(&aur_pkg.version.to_string())
    );

    if !aur_pkg.description.is_empty() {
        println!("  {} {}", style::dim("│"), style::dim(&aur_pkg.description));
    }

    println!();
    ui::print_warning("AUR packages are user-submitted and not vetted by Arch Linux");
    println!(
        "  {} Review the PKGBUILD before installing",
        style::dim("ℹ")
    );

    let should_install = if yes {
        true
    } else if console::user_attended() {
        use dialoguer::Confirm;
        Confirm::with_theme(&ui::prompt_theme())
            .with_prompt(format!("Install {pkg_name} from AUR?"))
            .default(false)
            .interact()?
    } else {
        false
    };

    if !should_install {
        anyhow::bail!("Installation cancelled by user");
    }

    println!();
    ui::print_header("AUR", &format!("Building {pkg_name}"));
    ui::print_spacer();

    let aur_client = AurClient::new();
    aur_client.install(pkg_name).await?;

    crate::core::usage::track_install(&[pkg_name.to_string()]);
    Ok(())
}
