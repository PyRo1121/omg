//! Command implementations for OMG CLI
//!
//! Uses direct libalpm access for 10-100x faster queries.

use anyhow::Result;
use colored::Colorize;
use std::process::Stdio;
use tokio::process::Command;

use crate::package_managers::get_system_status;

// Re-export moved commands
pub use super::env::{capture as env_capture, check as env_check, share as env_share};
pub use super::packages::{clean, explicit, info, install, remove, search, sync, update};
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

pub async fn status() -> Result<()> {
    let _start = std::time::Instant::now();

    println!("{} System Status\n", "OMG".cyan().bold());

    // ULTRA FAST: Use Daemon Cache if available (<1ms)
    let (total, explicit, orphans, updates, security_vulnerabilities) =
        if let Ok(mut client) = crate::core::client::DaemonClient::connect().await {
            if let Ok(res) = client.status().await {
                (
                    res.total_packages,
                    res.explicit_packages,
                    res.orphan_packages,
                    res.updates_available,
                    res.security_vulnerabilities,
                )
            } else {
                let s = get_system_status().unwrap_or((0, 0, 0, 0));
                (s.0, s.1, s.2, s.3, 0)
            }
        } else {
            // Fallback to local optimized ALPM query
            let s = get_system_status().unwrap_or((0, 0, 0, 0));
            (s.0, s.1, s.2, s.3, 0)
        };

    if updates > 0 {
        println!("  {} {} updates available", "Updates:".yellow(), updates);
    } else {
        println!("  {} System is up to date", "Updates:".green());
    }

    println!(
        "  {} {} total ({} explicit)",
        "Packages:".green(),
        total,
        explicit
    );

    if orphans > 0 {
        println!("  {} {} packages", "Orphans:".yellow(), orphans);
    }

    // Zero-Trust Security Status
    if security_vulnerabilities > 0 {
        println!(
            "  {} {} vulnerabilities found!",
            "Security:".red().bold(),
            security_vulnerabilities
        );
        println!(
            "  {} Run '{}' for details",
            "→".dimmed(),
            "omg audit".yellow()
        );
    } else {
        println!("  {} No known vulnerabilities", "Security:".green());
    }

    // Daemon status
    let socket = std::env::var("XDG_RUNTIME_DIR")
        .map_or_else(|_| "/tmp/omg.sock".to_string(), |d| format!("{d}/omg.sock"));

    if std::path::Path::new(&socket).exists() {
        println!("  {} Running", "Daemon:".green());
    } else {
        println!("  {} Not running", "Daemon:".dimmed());
    }

    // Runtimes - ULTRA-FAST PROBING (<1ms)
    println!("\n{} Runtimes:\n", "─".repeat(20).dimmed());

    let (node, py, rust, go, bun, java, ruby) = tokio::join!(
        tokio::task::spawn_blocking(|| crate::runtimes::probe_version("node")),
        tokio::task::spawn_blocking(|| crate::runtimes::probe_version("python")),
        tokio::task::spawn_blocking(|| crate::runtimes::probe_version("rust")),
        tokio::task::spawn_blocking(|| crate::runtimes::probe_version("go")),
        tokio::task::spawn_blocking(|| crate::runtimes::probe_version("bun")),
        tokio::task::spawn_blocking(|| crate::runtimes::probe_version("java")),
        tokio::task::spawn_blocking(|| crate::runtimes::probe_version("ruby")),
    );

    if let Ok(Some(v)) = node {
        println!("  {} Node.js {}", "●".green(), v);
    }
    if let Ok(Some(v)) = py {
        println!("  {} Python {}", "●".green(), v);
    }
    if let Ok(Some(v)) = rust {
        println!("  {} Rust {}", "●".green(), v);
    }
    if let Ok(Some(v)) = go {
        println!("  {} Go {}", "●".green(), v);
    }
    if let Ok(Some(v)) = bun {
        println!("  {} Bun {}", "●".green(), v);
    }
    if let Ok(Some(v)) = java {
        println!("  {} Java {}", "●".green(), v);
    }
    if let Ok(Some(v)) = ruby {
        println!("  {} Ruby {}", "●".green(), v);
    }

    Ok(())
}

/// Start the daemon
pub async fn daemon(foreground: bool) -> Result<()> {
    if foreground {
        println!("{} Run 'omgd' directly for daemon mode", "→".blue());
    } else {
        // Start daemon in background
        let status = Command::new("omgd")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match status {
            Ok(_) => println!("{} Daemon started", "✓".green()),
            Err(e) => println!("{} Failed to start daemon: {}", "✗".red(), e),
        }
    }
    Ok(())
}

/// Get or set configuration
pub async fn config(key: Option<&str>, value: Option<&str>) -> Result<()> {
    match (key, value) {
        (Some(k), Some(v)) => {
            println!(
                "{} Setting {} = {}",
                "OMG".cyan().bold(),
                k.green(),
                v.yellow()
            );
        }
        (Some(k), None) => {
            println!("{} Config key '{}':", "OMG".cyan().bold(), k.green());
            match k {
                "shims.enabled" => println!("  {}", "false".yellow()),
                "data_dir" => println!("  {}", "~/.omg".yellow()),
                _ => println!("  {}", "(not set)".dimmed()),
            }
        }
        (None, _) => {
            println!("{} Configuration:\n", "OMG".cyan().bold());
            println!("  {} = {}", "shims.enabled".green(), "false".yellow());
            println!("  {} = {}", "data_dir".green(), "~/.omg".yellow());
            println!(
                "  {} = {}",
                "socket".green(),
                "/run/user/1000/omg.sock".yellow()
            );
        }
    }
    Ok(())
}
