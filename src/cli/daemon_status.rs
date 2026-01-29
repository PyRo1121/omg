//! Daemon status display for OMG CLI
//!
//! Shows detailed daemon information including uptime, metrics, and health.

use anyhow::Result;

use crate::cli::style;
use crate::core::client::DaemonClient;
use crate::daemon::protocol::{Request, ResponseResult};

/// Display detailed daemon status
pub async fn run() -> Result<()> {
    println!("{}", style::header("OMG Daemon Status"));
    println!();

    let socket_path = crate::core::paths::socket_path();

    // Check if socket exists
    if !socket_path.exists() {
        println!("  {} Daemon socket not found", style::error("✗"));
        println!("    Path: {}", socket_path.display());
        println!();
        println!(
            "  {} Run 'omg daemon' to start the daemon",
            style::dim("Tip:")
        );
        return Ok(());
    }

    // Try to connect
    let client = match DaemonClient::connect().await {
        Ok(c) => c,
        Err(e) => {
            println!("  {} Failed to connect to daemon", style::error("✗"));
            println!("    Error: {e}");
            println!();

            // Check socket permissions
            #[cfg(unix)]
            {
                if let Ok(meta) = std::fs::metadata(&socket_path) {
                    use std::os::unix::fs::MetadataExt;
                    let socket_uid = meta.uid();
                    let current_uid = rustix::process::getuid().as_raw();

                    if socket_uid != current_uid {
                        println!(
                            "  {} Socket belongs to UID {} (you are UID {})",
                            style::warning("⚠"),
                            socket_uid,
                            current_uid
                        );
                        println!("    The daemon may have been started by a different user.");
                    }
                }
            }
            return Ok(());
        }
    };

    // Get daemon metrics
    let mut client = client;

    // Ping first to verify connection
    match client.ping().await {
        Ok(_) => {
            println!("  {} Daemon is running", style::success("✓"));
        }
        Err(e) => {
            println!("  {} Daemon not responding: {}", style::error("✗"), e);
            return Ok(());
        }
    }

    println!("    Socket: {}", socket_path.display());

    // Get metrics
    match client.call(Request::Metrics { id: 0 }).await {
        Ok(ResponseResult::Metrics(metrics)) => {
            println!();
            println!("  {}", style::header("Metrics"));
            println!(
                "    Requests total:     {}",
                style::info(&metrics.requests_total.to_string())
            );
            println!(
                "    Requests failed:    {}",
                if metrics.requests_failed > 0 {
                    style::warning(&metrics.requests_failed.to_string())
                } else {
                    style::success(&metrics.requests_failed.to_string())
                }
            );
            println!(
                "    Active connections: {}",
                style::info(&metrics.active_connections.to_string())
            );
            println!(
                "    Rate limit hits:    {}",
                if metrics.rate_limit_hits > 0 {
                    style::warning(&metrics.rate_limit_hits.to_string())
                } else {
                    style::dim(&metrics.rate_limit_hits.to_string())
                }
            );
            println!(
                "    Validation failures: {}",
                if metrics.validation_failures > 0 {
                    style::warning(&metrics.validation_failures.to_string())
                } else {
                    style::dim(&metrics.validation_failures.to_string())
                }
            );
            println!(
                "    Security audits:    {}",
                style::dim(&metrics.security_audit_requests.to_string())
            );

            // Format bytes
            let bytes_recv = format_bytes(metrics.bytes_received);
            let bytes_sent = format_bytes(metrics.bytes_sent);
            println!("    Bytes received:     {}", style::dim(&bytes_recv));
            println!("    Bytes sent:         {}", style::dim(&bytes_sent));
        }
        Ok(_) => {
            println!("  {} Unexpected response from daemon", style::warning("⚠"));
        }
        Err(e) => {
            println!("  {} Failed to get metrics: {}", style::warning("⚠"), e);
        }
    }

    // Get system status from daemon
    match client.call(Request::Status { id: 0 }).await {
        Ok(ResponseResult::Status(status)) => {
            println!();
            println!("  {}", style::header("Package Cache"));
            println!(
                "    Total packages:     {}",
                style::info(&status.total_packages.to_string())
            );
            println!(
                "    Explicit packages:  {}",
                style::info(&status.explicit_packages.to_string())
            );
            println!(
                "    Orphan packages:    {}",
                if status.orphan_packages > 0 {
                    style::warning(&status.orphan_packages.to_string())
                } else {
                    style::dim(&status.orphan_packages.to_string())
                }
            );
            println!(
                "    Updates available:  {}",
                if status.updates_available > 0 {
                    style::warning(&status.updates_available.to_string())
                } else {
                    style::success("0")
                }
            );
            println!(
                "    Vulnerabilities:    {}",
                if status.security_vulnerabilities > 0 {
                    style::error(&status.security_vulnerabilities.to_string())
                } else {
                    style::success("0")
                }
            );

            if !status.runtime_versions.is_empty() {
                println!();
                println!("  {}", style::header("Runtime Versions"));
                for (name, version) in &status.runtime_versions {
                    println!("    {}: {}", style::runtime(name), style::version(version));
                }
            }
        }
        Ok(_) => {}
        Err(_) => {}
    }

    println!();
    Ok(())
}

/// Format bytes as human-readable string
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
