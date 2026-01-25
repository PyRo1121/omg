//! `omg blame` - Show when and why a package was installed

use anyhow::Result;
use owo_colors::OwoColorize;

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
    use crate::cli::components::Components;

    // First check if package is installed
    let (is_installed, version, install_reason) = get_package_info(package)?;

    if !is_installed {
        return Ok(Components::error_with_suggestion(
            format!("Package '{package}' is not installed"),
            "Try 'omg search' to find available packages",
        ));
    }

    let mut commands = vec![
        Components::header("Package History", package),
        Components::spacer(),
    ];

    // Package info
    if let Some(ver) = &version {
        commands.push(Components::kv_list(
            Some("Package Information"),
            vec![
                ("Name", package),
                ("Version", ver),
                ("Install Reason", &install_reason),
            ],
        ));
    } else {
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
        commands.push(Components::spacer());
        commands.push(Components::muted(
            "No transaction history found (Package may have been installed before OMG tracking began)",
        ));
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

        commands.push(Components::spacer());
        commands.push(Components::card(
            format!("Transaction History ({})", relevant.len()),
            txn_content,
        ));

        if relevant.len() > 10 {
            commands.push(Components::muted(format!(
                "... and {} more transactions",
                relevant.len() - 10
            )));
        }
    }

    // Show what requires this package
    commands.push(Components::spacer());
    commands.push(show_required_by(package)?);

    Ok(Cmd::batch(commands))
}

#[cfg(feature = "arch")]
fn get_package_info(package: &str) -> Result<(bool, Option<String>, String)> {
    use alpm::Alpm;

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();

    match localdb.pkg(package) {
        Ok(pkg) => {
            let reason = match pkg.reason() {
                alpm::PackageReason::Explicit => "explicit (user installed)".green().to_string(),
                alpm::PackageReason::Depend => "dependency".yellow().to_string(),
            };
            Ok((true, Some(pkg.version().to_string()), reason))
        }
        Err(_) => Ok((false, None, "not installed".to_string())),
    }
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn get_package_info(package: &str) -> Result<(bool, Option<String>, String)> {
    use std::process::Command;

    let output = Command::new("dpkg-query")
        .args(["-W", "-f=${Version}\t${Status}", "--", package])
        .output()?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<_> = stdout.split('\t').collect();
        if parts.len() >= 2 && parts[1].contains("installed") {
            // Check if auto-installed
            let auto_check = Command::new("apt-mark")
                .args(["showauto", "--", package])
                .output()?;
            let is_auto = String::from_utf8_lossy(&auto_check.stdout)
                .trim()
                .contains(package);

            let reason = if is_auto {
                "dependency (auto-installed)".yellow().to_string()
            } else {
                "explicit (user installed)".green().to_string()
            };

            return Ok((true, Some(parts[0].to_string()), reason));
        }
    }

    Ok((false, None, "not installed".to_string()))
}

#[cfg(not(any(feature = "arch", feature = "debian")))]
#[allow(clippy::unnecessary_wraps)]
fn get_package_info(_package: &str) -> Result<(bool, Option<String>, String)> {
    Ok((false, None, "unknown".to_string()))
}

#[cfg(feature = "arch")]
fn show_required_by(package: &str) -> Result<Cmd<()>> {
    use crate::cli::components::Components;
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
        Ok(Components::info("Nothing depends on this package"))
    } else {
        Ok(Components::card(
            format!("Required by ({} packages)", required_by.len()),
            required_by,
        ))
    }
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn show_required_by(package: &str) -> Result<Cmd<()>> {
    use crate::cli::components::Components;
    use std::process::Command;

    let output = Command::new("apt-cache")
        .args(["rdepends", "--installed", "--", package])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let deps: Vec<_> = stdout
        .lines()
        .skip(2) // Skip header
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().to_string())
        .collect();

    if deps.is_empty() {
        Ok(Components::info("Nothing depends on this package"))
    } else {
        Ok(Components::card(
            format!("Required by ({} packages)", deps.len()),
            deps,
        ))
    }
}

#[cfg(not(any(feature = "arch", feature = "debian")))]
#[allow(clippy::unnecessary_wraps)]
fn show_required_by(_package: &str) -> Result<Cmd<()>> {
    use crate::cli::components::Components;
    Ok(Components::info("Dependency information not available"))
}

fn format_timestamp(ts: i64) -> String {
    use jiff::Timestamp;

    if let Ok(dt) = Timestamp::from_second(ts) {
        // Format as ISO-like but more readable
        format!("{dt}")
            .chars()
            .take(16)
            .collect::<String>()
            .replace('T', " ")
    } else {
        "unknown".to_string()
    }
}
