//! `omg blame` - Show when and why a package was installed

use anyhow::Result;

use crate::cli::tea::Cmd;
use crate::core::history::{HistoryManager, TransactionType};

/// Show package installation history
pub fn run(package: &str) -> Result<()> {
    // SECURITY: Validate package name
    crate::core::security::validate_package_name(package)?;

    let cmd = build_blame_output(package)?;
    crate::cli::packages::execute_cmd(cmd);

    Ok(())
}

fn build_blame_output(package: &str) -> Result<Cmd<()>> {
    // First check if package is installed
    let (is_installed, version, install_reason) = get_package_info(package)?;

    if !is_installed {
        use crate::cli::components::Components;
        return Ok(Components::error_with_suggestion(
            format!("Package '{package}' is not installed"),
            "Try 'omg search' to find available packages",
        ));
    }

    let mut commands = vec![Cmd::header("Package History", package), Cmd::spacer()];

    // Package info
    if let Some(ver) = &version {
        use crate::cli::components::Components;
        commands.push(Components::kv_list(
            Some("Package Information"),
            vec![
                ("Name", package),
                ("Version", ver),
                ("Install Reason", &install_reason),
            ],
        ));
    } else {
        use crate::cli::components::Components;
        commands.push(Components::kv_list(
            Some("Package Information"),
            vec![("Name", package), ("Install Reason", &install_reason)],
        ));
    }

    // Search transaction history
    let history = HistoryManager::new()?;
    let transactions = history.load()?;

    let relevant: Vec<_> = transactions
        .iter()
        .filter(|t| t.changes.iter().any(|c| c.name == package))
        .collect();

    if relevant.is_empty() {
        use crate::cli::tea::{StyledTextConfig, TextStyle};
        commands.push(Cmd::spacer());
        commands.push(Cmd::styled_text(StyledTextConfig {
            text: "No transaction history found (Package may have been installed before OMG tracking began)".to_string(),
            style: TextStyle::Muted,
        }));
    } else {
        let txn_content: Vec<String> = relevant
            .iter()
            .rev()
            .take(10)
            .filter_map(|txn| {
                // Safe: we filtered for transactions containing this package above
                let change = txn.changes.iter().find(|c| c.name == package)?;

                let action = match txn.transaction_type {
                    TransactionType::Install => "installed",
                    TransactionType::Remove => "removed",
                    TransactionType::Update => "updated",
                    TransactionType::Sync => "synced",
                };

                let version_info = match (&change.old_version, &change.new_version) {
                    (None, Some(new)) => format!("→ {new}"),
                    (Some(old), Some(new)) => format!("{old} → {new}"),
                    (Some(old), None) => format!("{old} → (removed)"),
                    (None, None) => String::new(),
                };

                let time = format_timestamp(txn.timestamp.as_second());
                Some(format!(
                    "{} {} {} ({})",
                    time, action, version_info, change.source
                ))
            })
            .collect();

        commands.push(Cmd::spacer());
        commands.push(Cmd::card(
            format!("Transaction History ({})", relevant.len()),
            txn_content,
        ));

        if relevant.len() > 10 {
            use crate::cli::tea::{StyledTextConfig, TextStyle};
            commands.push(Cmd::styled_text(StyledTextConfig {
                text: format!("... and {} more transactions", relevant.len() - 10),
                style: TextStyle::Muted,
            }));
        }
    }

    // Show what requires this package
    commands.push(Cmd::spacer());
    commands.push(show_required_by(package)?);

    Ok(Cmd::batch(commands))
}

#[cfg(feature = "arch")]
fn get_package_info(package: &str) -> Result<(bool, Option<String>, String)> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return get_package_info_debian(package);
    }

    use crate::cli::style;
    use alpm::Alpm;

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();

    match localdb.pkg(package) {
        Ok(pkg) => {
            let reason = match pkg.reason() {
                alpm::PackageReason::Explicit => style::version("explicit (user installed)"),
                alpm::PackageReason::Depend => style::path("dependency"),
            };
            Ok((true, Some(pkg.version().to_string()), reason))
        }
        Err(_) => Ok((false, None, "not installed".to_string())),
    }
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn get_package_info_debian(package: &str) -> Result<(bool, Option<String>, String)> {
    use crate::cli::style;
    use crate::package_managers::debian_db;

    match debian_db::get_package_version(package)? {
        Some(version) => {
            let is_auto = debian_db::is_package_auto_installed(package)?;

            let reason = if is_auto {
                style::path("dependency (auto-installed)")
            } else {
                style::version("explicit (user installed)")
            };

            Ok((true, Some(version), reason))
        }
        None => Ok((false, None, "not installed".to_string())),
    }
}

#[cfg(all(
    any(feature = "debian", feature = "debian-pure"),
    not(feature = "arch")
))]
fn get_package_info(package: &str) -> Result<(bool, Option<String>, String)> {
    get_package_info_debian(package)
}

#[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
fn get_package_info(_package: &str) -> Result<(bool, Option<String>, String)> {
    anyhow::bail!("Package information is not available without an Arch or Debian package backend");
}

#[cfg(feature = "arch")]
fn show_required_by(package: &str) -> Result<Cmd<()>> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return show_required_by_debian(package);
    }

    use alpm::Alpm;

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();
    let mut required_by = Vec::new();

    for pkg in localdb.pkgs() {
        for dep in pkg.depends() {
            if dep.name() == package {
                required_by.push(pkg.name().to_string());
                break;
            }
        }
    }

    if required_by.is_empty() {
        Ok(Cmd::info("Nothing depends on this package"))
    } else {
        Ok(Cmd::card(
            format!("Required by ({} packages)", required_by.len()),
            required_by,
        ))
    }
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn show_required_by_debian(package: &str) -> Result<Cmd<()>> {
    use crate::package_managers::debian_db;

    let (_, reverse_deps) = debian_db::get_package_dependencies(package)?;

    let deps: Vec<_> = reverse_deps.into_iter().filter(|d| !d.is_empty()).collect();

    if deps.is_empty() {
        Ok(Cmd::info("Nothing depends on this package"))
    } else {
        Ok(Cmd::card(
            format!("Required by ({} packages)", deps.len()),
            deps,
        ))
    }
}

#[cfg(all(
    any(feature = "debian", feature = "debian-pure"),
    not(feature = "arch")
))]
fn show_required_by(package: &str) -> Result<Cmd<()>> {
    show_required_by_debian(package)
}

#[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
fn show_required_by(_package: &str) -> Result<Cmd<()>> {
    anyhow::bail!(
        "Dependency information is not available without an Arch or Debian package backend"
    );
}

fn format_timestamp(ts: i64) -> String {
    use jiff::Timestamp;

    Timestamp::from_second(ts).map_or_else(
        |_| "unknown".to_string(),
        |dt| {
            // Format as ISO-like but more readable
            format!("{dt}")
                .chars()
                .take(16)
                .collect::<String>()
                .replace('T', " ")
        },
    )
}

#[cfg(all(
    test,
    not(any(feature = "arch", feature = "debian", feature = "debian-pure"))
))]
mod tests {
    use super::*;

    #[test]
    fn blame_without_backend_does_not_pretend_the_package_is_missing() {
        let error = build_blame_output("bash")
            .expect_err("blame with no backend must not look like not-installed");
        assert!(
            error
                .to_string()
                .contains("not available without an Arch or Debian package backend")
        );
    }

    #[test]
    fn required_by_without_backend_is_an_error() {
        let error = show_required_by("bash")
            .expect_err("dependency lookup with no backend must not look like success");
        assert!(
            error
                .to_string()
                .contains("not available without an Arch or Debian package backend")
        );
    }
}
