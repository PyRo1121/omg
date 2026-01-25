//! Status command - system-wide package status overview

use anyhow::Result;
use owo_colors::OwoColorize;
use std::io::Write;

use crate::cli::style;
use crate::cli::tea::run_status_elm;
use crate::core::client::DaemonClient;
use crate::daemon::protocol::{Request, ResponseResult};
use crate::package_managers::get_package_manager;

/// Show system package status overview
pub async fn status(fast: bool) -> Result<()> {
    // Try modern Elm UI first
    if let Err(e) = run_status_elm(fast) {
        eprintln!("Warning: Elm UI failed, falling back to basic mode: {e}");
        status_fallback(fast).await
    } else {
        Ok(())
    }
}

/// Fallback implementation using original approach
async fn status_fallback(fast: bool) -> Result<()> {
    let start = std::time::Instant::now();

    // 1. Try Daemon first (Hot Path)
    if let Ok(mut client) = DaemonClient::connect().await
        && let Ok(ResponseResult::Status(status)) = client.call(Request::Status { id: 0 }).await
    {
        display_status_report(
            status.total_packages,
            status.explicit_packages,
            status.orphan_packages,
            status.updates_available,
            start.elapsed(),
            fast,
        )?;
        return Ok(());
    }

    // 2. Fallback to direct path (Cold Path)
    let pm = get_package_manager();
    let (total, explicit, orphans, updates) = pm.get_status(fast).await?;
    display_status_report(total, explicit, orphans, updates, start.elapsed(), fast)
}

fn display_status_report(
    total: usize,
    explicit: usize,
    orphans: usize,
    updates: usize,
    duration: std::time::Duration,
    fast: bool,
) -> Result<()> {
    let mut stdout = std::io::BufWriter::new(std::io::stdout());

    writeln!(
        stdout,
        "  {} Status Overview ({:.1}ms)",
        "📋".bold(),
        duration.as_secs_f64() * 1000.0
    )?;
    writeln!(stdout, "  {}", "─".repeat(40).dimmed())?;

    writeln!(
        stdout,
        "  {:<20} {}",
        "Total Packages:".bold(),
        total.to_string().cyan()
    )?;
    writeln!(
        stdout,
        "  {:<20} {}",
        "Explicitly Installed:".bold(),
        explicit.to_string().green()
    )?;

    if fast {
        writeln!(
            stdout,
            "  {:<20} {}",
            "Orphans/Updates:".dimmed(),
            "skipped (fast mode)".dimmed()
        )?;
    } else {
        writeln!(
            stdout,
            "  {:<20} {}",
            "Orphan Packages:".bold(),
            if orphans > 0 {
                orphans.to_string().yellow().to_string()
            } else {
                "0".dimmed().to_string()
            }
        )?;
        writeln!(
            stdout,
            "  {:<20} {}",
            "Updates Available:".bold(),
            if updates > 0 {
                updates.to_string().bright_magenta().to_string()
            } else {
                "0".dimmed().to_string()
            }
        )?;
    }

    writeln!(stdout, "\n  {} Environment Status:", "🌍".bold())?;
    // Add runtime versions if available
    #[cfg(feature = "arch")]
    {
        // For now, just a placeholder for runtimes in status
    }

    writeln!(
        stdout,
        "\n  {} {}",
        style::arrow("Tip:"),
        style::dim("Use 'omg clean' to remove orphans and free up disk space.")
    )?;

    stdout.flush()?;
    Ok(())
}
