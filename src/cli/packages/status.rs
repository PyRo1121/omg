//! Status command - system-wide package status overview

use anyhow::Result;
use owo_colors::OwoColorize;
use serde::Serialize;
use std::io::Write;

use crate::cli::style;
use crate::cli::tea::run_status_elm;
use crate::core::client::DaemonClient;
use crate::daemon::protocol::{Request, ResponseResult};
use crate::package_managers::get_package_manager;

#[derive(Serialize)]
struct StatusJson {
    total_packages: usize,
    explicit_packages: usize,
    orphan_packages: usize,
    updates_available: usize,
    query_time_ms: f64,
}

pub async fn status(fast: bool) -> Result<()> {
    status_with_json(fast, false).await
}

pub async fn status_with_json(fast: bool, json: bool) -> Result<()> {
    if json {
        return status_json(fast).await;
    }

    if let Err(e) = run_status_elm(fast) {
        tracing::warn!("Elm UI failed, falling back to basic mode: {}", e);
        status_fallback(fast).await
    } else {
        Ok(())
    }
}

async fn status_json(fast: bool) -> Result<()> {
    let start = std::time::Instant::now();

    let (total, explicit, orphans, updates) = if let Ok(mut client) = DaemonClient::connect().await
        && let Ok(ResponseResult::Status(status)) = client.call(Request::Status { id: 0 }).await
    {
        (
            status.total_packages,
            status.explicit_packages,
            status.orphan_packages,
            status.updates_available,
        )
    } else {
        let pm = get_package_manager();
        pm.get_status(fast).await?
    };

    let status = StatusJson {
        total_packages: total,
        explicit_packages: explicit,
        orphan_packages: orphans,
        updates_available: updates,
        query_time_ms: start.elapsed().as_secs_f64() * 1000.0,
    };

    let json_str = serde_json::to_string_pretty(&status)
        .unwrap_or_else(|_| "{}".to_string());
    println!("{json_str}");

    Ok(())
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
        style::maybe_color("📋", |t| t.bold().to_string()),
        duration.as_secs_f64() * 1000.0
    )?;
    writeln!(stdout, "  {}", style::dim(&"─".repeat(40)))?;

    writeln!(
        stdout,
        "  {:<20} {}",
        style::maybe_color("Total Packages:", |t| t.bold().to_string()),
        style::maybe_color(&total.to_string(), |t| t.cyan().to_string())
    )?;
    writeln!(
        stdout,
        "  {:<20} {}",
        style::maybe_color("Explicitly Installed:", |t| t.bold().to_string()),
        style::version(&explicit.to_string())
    )?;

    if fast {
        writeln!(
            stdout,
            "  {:<20} {}",
            style::dim("Orphans/Updates:"),
            style::dim("skipped (fast mode)")
        )?;
    } else {
        writeln!(
            stdout,
            "  {:<20} {}",
            style::maybe_color("Orphan Packages:", |t| t.bold().to_string()),
            if orphans > 0 {
                style::maybe_color(&orphans.to_string(), |t| t.yellow().to_string())
            } else {
                style::dim("0")
            }
        )?;
        writeln!(
            stdout,
            "  {:<20} {}",
            style::maybe_color("Updates Available:", |t| t.bold().to_string()),
            if updates > 0 {
                style::maybe_color(&updates.to_string(), |t| {
                    t.bright_magenta().to_string()
                })
            } else {
                style::dim("0")
            }
        )?;
    }

    writeln!(
        stdout,
        "\n  {} Environment Status:",
        style::maybe_color("🌍", |t| t.bold().to_string())
    )?;
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
