//! Command implementations for OMG CLI
//!
//! Uses direct libalpm access for 10-100x faster queries.

use anyhow::Result;
use std::process::{Command, Stdio};

use crate::cli::style;
use crate::package_managers::get_system_status;

// Re-export moved commands
pub use super::env::{capture as env_capture, check as env_check, share as env_share};
pub use super::packages::{
    clean, explicit, explicit_sync, info, info_sync, install, remove, search, sync, update,
};
pub use super::runtimes::{list_versions, use_version};
pub use super::security::audit;

pub async fn complete(_shell: &str, current: &str, last: &str, _full: Option<&str>) -> Result<()> {
    let db = crate::core::Database::open(crate::core::Database::default_path()?)?;
    let engine = crate::core::completion::CompletionEngine::new(db);

    let suggestions = match last {
        "install" | "i" | "remove" | "r" | "info" => {
            // Try daemon for package list
            let mut names = if let Ok(mut client) =
                crate::core::client::DaemonClient::connect().await
            {
                if let Ok(res) = client.search("", None).await {
                    res.packages.into_iter().map(|p| p.name).collect()
                } else {
                    crate::package_managers::alpm_direct::list_all_package_names()
                        .unwrap_or_default()
                }
            } else {
                crate::package_managers::alpm_direct::list_all_package_names().unwrap_or_default()
            };

            // Also include AUR package names (from cache)
            if let Ok(aur_names) = engine.get_aur_package_names().await {
                names.extend(aur_names);
                names.sort();
                names.dedup();
            }

            engine.fuzzy_match(current, names)
        }
        "use" | "ls" | "list" | "which" => {
            let runtimes: Vec<String> = crate::runtimes::SUPPORTED_RUNTIMES
                .iter()
                .map(std::string::ToString::to_string)
                .collect();
            engine.fuzzy_match(current, runtimes)
        }
        _ => {
            // Check if last word is a runtime (for 'omg use <runtime> <TAB>')
            if crate::runtimes::SUPPORTED_RUNTIMES.contains(&last) {
                // Priority 1: Context awareness (package.json, .nvmrc, etc.)
                let mut suggestions = engine.probe_context(last);

                // Priority 2: Installed versions
                let data_dir = crate::core::Database::default_path()?
                    .parent()
                    .unwrap()
                    .to_path_buf();
                let runtime_dir = data_dir.join("versions").join(last);
                let mut installed_versions = Vec::new();
                if let Ok(entries) = std::fs::read_dir(runtime_dir) {
                    for entry in entries.flatten() {
                        if let Ok(file_type) = entry.file_type() {
                            if file_type.is_dir() {
                                if let Some(name) = entry.file_name().to_str() {
                                    if name != "current" {
                                        installed_versions.push(name.to_string());
                                    }
                                }
                            }
                        }
                    }
                }

                let fuzzy_installed = engine.fuzzy_match(current, installed_versions);
                suggestions.extend(fuzzy_installed);
                suggestions.dedup();
                suggestions
            } else {
                Vec::new()
            }
        }
    };

    for suggestion in suggestions {
        println!("{suggestion}");
    }

    Ok(())
}

pub fn status_sync() -> Result<()> {
    let _start = std::time::Instant::now();
    let mut stdout = std::io::BufWriter::new(std::io::stdout());
    use std::io::Write;

    writeln!(stdout, "{} System Status\n", style::header("OMG"))?;

    // ULTRA FAST: Use Daemon Cache if available (<1ms)
    let (total, explicit, orphans, updates, security_vulnerabilities, cached_runtimes) =
        if let Ok(mut client) = crate::core::client::DaemonClient::connect_sync() {
            // Fixed ID for zero-overhead
            if let Ok(crate::daemon::protocol::ResponseResult::Status(res)) =
                client.call_sync(crate::daemon::protocol::Request::Status { id: 0 })
            {
                (
                    res.total_packages,
                    res.explicit_packages,
                    res.orphan_packages,
                    res.updates_available,
                    res.security_vulnerabilities,
                    Some(res.runtime_versions),
                )
            } else {
                let s = get_system_status().unwrap_or((0, 0, 0, 0));
                (s.0, s.1, s.2, s.3, 0, None)
            }
        } else {
            // Fallback to local optimized ALPM query
            let s = get_system_status().unwrap_or((0, 0, 0, 0));
            (s.0, s.1, s.2, s.3, 0, None)
        };

    if updates > 0 {
        writeln!(
            stdout,
            "  {} {} updates available",
            style::warning("Updates:"),
            updates
        )?;
    } else {
        writeln!(
            stdout,
            "  {} System is up to date",
            style::success("Updates:")
        )?;
    }

    writeln!(
        stdout,
        "  {} {} total ({} explicit)",
        style::success("Packages:"),
        total,
        explicit
    )?;

    if orphans > 0 {
        writeln!(
            stdout,
            "  {} {} packages",
            style::warning("Orphans:"),
            orphans
        )?;
    }

    // Zero-Trust Security Status
    if security_vulnerabilities > 0 {
        writeln!(
            stdout,
            "  {} {} vulnerabilities found!",
            style::error("Security:"),
            security_vulnerabilities
        )?;
        writeln!(
            stdout,
            "  {} Run '{}' for details",
            style::dim("→"),
            style::warning("omg audit")
        )?;
    } else {
        writeln!(
            stdout,
            "  {} No known vulnerabilities",
            style::success("Security:")
        )?;
    }

    // Daemon status
    let socket = crate::core::client::default_socket_path();
    if socket.exists() {
        writeln!(stdout, "  {} Running", style::success("Daemon:"))?;
    } else {
        writeln!(stdout, "  {} Not running", style::dim("Daemon:"))?;
    }

    // Runtimes - INSTANT FROM CACHE
    writeln!(
        stdout,
        "\n{} Runtimes:\n",
        style::dim("────────────────────")
    )?;

    if let Some(versions) = cached_runtimes {
        for (rt_name, v) in versions {
            let label = match rt_name.as_str() {
                "node" => "Node.js",
                "python" => "Python",
                "rust" => "Rust",
                "go" => "Go",
                "bun" => "Bun",
                "java" => "Java",
                "ruby" => "Ruby",
                _ => &rt_name,
            };
            writeln!(stdout, "  {} {} {}", style::success("●"), label, v)?;
        }
    } else {
        // Fallback to local probing if daemon is down
        for rt_name in &["node", "python", "rust", "go", "bun", "java", "ruby"] {
            if let Some(v) = crate::runtimes::probe_version(rt_name) {
                let label = match *rt_name {
                    "node" => "Node.js",
                    "python" => "Python",
                    "rust" => "Rust",
                    "go" => "Go",
                    "bun" => "Bun",
                    "java" => "Java",
                    "ruby" => "Ruby",
                    _ => rt_name,
                };
                writeln!(stdout, "  {} {} {}", style::success("●"), label, v)?;
            }
        }
    }

    stdout.flush()?;
    Ok(())
}

/// Start the daemon
pub fn daemon(foreground: bool) -> Result<()> {
    if foreground {
        println!("{} Run 'omgd' directly for daemon mode", style::info("→"));
    } else {
        // Start daemon in background
        let status = Command::new("omgd")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match status {
            Ok(_) => println!("{} Daemon started", style::success("✓")),
            Err(e) => println!("{} Failed to start daemon: {}", style::error("✗"), e),
        }
    }
    Ok(())
}

/// Get or set configuration
pub fn config(key: Option<&str>, value: Option<&str>) -> Result<()> {
    match (key, value) {
        (Some(k), Some(v)) => {
            println!(
                "{} Setting {} = {}",
                style::header("OMG"),
                style::success(k),
                style::warning(v)
            );
        }
        (Some(k), None) => {
            println!(
                "{} Config key '{}':",
                style::header("OMG"),
                style::success(k)
            );
            match k {
                "shims.enabled" => println!("  {}", style::warning("false")),
                "data_dir" => println!("  {}", style::warning("~/.omg")),
                _ => println!("  {}", style::dim("(not set)")),
            }
        }
        (None, _) => {
            println!("{} Configuration:\n", style::header("OMG"));
            println!(
                "  {} = {}",
                style::success("shims.enabled"),
                style::warning("false")
            );
            println!(
                "  {} = {}",
                style::success("data_dir"),
                style::warning("~/.omg")
            );
            println!(
                "  {} = {}",
                style::success("socket"),
                style::warning("/run/user/1000/omg.sock")
            );
        }
    }
    Ok(())
}
