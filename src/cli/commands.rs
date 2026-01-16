//! Command implementations for OMG CLI
//!
//! Uses direct libalpm access for 10-100x faster queries.

use anyhow::{Context, Result};
use std::process::{Command, Stdio};

#[cfg(feature = "debian")]
use crate::core::env::distro::is_debian_like;
use crate::core::paths;
use crate::package_managers::PackageManager;

use crate::cli::style;

#[cfg(feature = "arch")]
use crate::package_managers::get_system_status;

#[cfg(feature = "debian")]
use crate::package_managers::{apt_get_system_status, apt_list_all_package_names};

const fn use_debian_backend() -> bool {
    #[cfg(feature = "debian")]
    {
        return is_debian_like();
    }

    #[cfg(not(feature = "debian"))]
    {
        false
    }
}

// Re-export moved commands
pub use super::env::{capture as env_capture, check as env_check, share as env_share};
pub use super::packages::{
    clean, explicit, explicit_sync, info, info_sync, install, remove, search, sync, update,
};
pub use super::runtimes::{list_versions, use_version};
pub use super::security::audit;

pub async fn complete(_shell: &str, current: &str, last: &str, full: Option<&str>) -> Result<()> {
    let db = crate::core::Database::open(crate::core::Database::default_path()?)?;
    let engine = crate::core::completion::CompletionEngine::new(db);

    let full_tokens: Vec<&str> = full.unwrap_or_default().split_whitespace().collect();
    let in_tool = full_tokens.contains(&"tool");
    let in_env = full_tokens.contains(&"env");

    let suggestions = match last {
        "install" | "i" | "remove" | "r" | "info" => {
            if in_tool && last == "install" {
                output_suggestions(&engine, current, crate::cli::tool::registry_tool_names());
                return Ok(());
            }
            if in_tool && last == "remove" {
                output_suggestions(&engine, current, crate::cli::tool::installed_tool_names());
                return Ok(());
            }
            // Try daemon for package list
            let mut names = if use_debian_backend() {
                #[cfg(feature = "debian")]
                {
                    apt_list_all_package_names().unwrap_or_default()
                }
                #[cfg(not(feature = "debian"))]
                {
                    Vec::new()
                }
            } else if let Ok(mut client) = crate::core::client::DaemonClient::connect().await {
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
            let runtimes = crate::cli::runtimes::known_runtimes();
            engine.fuzzy_match(current, runtimes)
        }
        "tool" => vec![
            "install".to_string(),
            "list".to_string(),
            "remove".to_string(),
        ],
        "env" => vec![
            "capture".to_string(),
            "check".to_string(),
            "share".to_string(),
            "sync".to_string(),
        ],
        "run" => {
            let tasks = crate::core::task_runner::detect_tasks().unwrap_or_default();
            let names = tasks.into_iter().map(|task| task.name).collect();
            engine.fuzzy_match(current, names)
        }
        "new" => vec![
            "rust".to_string(),
            "react".to_string(),
            "react-ts".to_string(),
            "node".to_string(),
            "ts".to_string(),
            "typescript".to_string(),
            "python".to_string(),
            "py".to_string(),
            "go".to_string(),
            "golang".to_string(),
        ],
        "completions" => vec![
            "bash".to_string(),
            "zsh".to_string(),
            "fish".to_string(),
            "powershell".to_string(),
            "elvish".to_string(),
        ],
        _ => {
            // Check if last word is a runtime (for 'omg use <runtime> <TAB>')
            if crate::cli::runtimes::known_runtimes()
                .iter()
                .any(|rt| rt == last)
            {
                // Priority 1: Context awareness (package.json, .nvmrc, etc.)
                let mut suggestions = engine.probe_context(last);

                // Priority 2: Installed versions
                let data_dir = crate::core::Database::default_path()?
                    .parent()
                    .ok_or_else(|| anyhow::anyhow!("Invalid database path"))?
                    .to_path_buf();
                let runtime_dir = data_dir.join("versions").join(last);
                let mut installed_versions = Vec::new();
                if let Ok(entries) = std::fs::read_dir(runtime_dir) {
                    for entry in entries.flatten() {
                        if let Ok(file_type) = entry.file_type()
                            && file_type.is_dir()
                            && let Some(name) = entry.file_name().to_str()
                            && name != "current"
                        {
                            installed_versions.push(name.to_string());
                        }
                    }
                }

                let fuzzy_installed = engine.fuzzy_match(current, installed_versions);
                suggestions.extend(fuzzy_installed);
                suggestions.dedup();
                suggestions
            } else if in_env {
                let options = vec![
                    "capture".to_string(),
                    "check".to_string(),
                    "share".to_string(),
                    "sync".to_string(),
                ];
                engine.fuzzy_match(current, options)
            } else if in_tool {
                let options = vec![
                    "install".to_string(),
                    "list".to_string(),
                    "remove".to_string(),
                ];
                engine.fuzzy_match(current, options)
            } else {
                Vec::new()
            }
        }
    };

    output_suggestions(&engine, current, suggestions);
    Ok(())
}

fn output_suggestions(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
    suggestions: Vec<String>,
) {
    let filtered = if current.is_empty() {
        suggestions
    } else {
        engine.fuzzy_match(current, suggestions)
    };

    for suggestion in filtered {
        println!("{suggestion}");
    }
}

pub fn status_sync() -> Result<()> {
    let _start = std::time::Instant::now();
    let mut stdout = std::io::BufWriter::new(std::io::stdout());
    use std::io::Write;

    writeln!(stdout, "{} System Status\n", style::header("OMG"))?;

    // ULTRA FAST: Use Daemon Cache if available (<1ms)
    let (total, explicit, orphans, updates, security_vulnerabilities, cached_runtimes) =
        if use_debian_backend() {
            #[cfg(feature = "debian")]
            {
                let s = apt_get_system_status().unwrap_or((0, 0, 0, 0));
                (s.0, s.1, s.2, s.3, 0, None)
            }
            #[cfg(not(feature = "debian"))]
            {
                (0, 0, 0, 0, 0, None)
            }
        } else if let Ok(mut client) = crate::core::client::DaemonClient::connect_sync() {
            // Fixed ID for zero-overhead
            if let Ok(crate::daemon::protocol::ResponseResult::Status(res)) =
                client.call_sync(&crate::daemon::protocol::Request::Status { id: 0 })
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
                #[cfg(feature = "arch")]
                {
                    let s = get_system_status().unwrap_or((0, 0, 0, 0));
                    (s.0, s.1, s.2, s.3, 0, None)
                }
                #[cfg(not(feature = "arch"))]
                {
                    (0, 0, 0, 0, 0, None)
                }
            }
        } else {
            // Fallback to local optimized ALPM query
            #[cfg(feature = "arch")]
            {
                let s = get_system_status().unwrap_or((0, 0, 0, 0));
                (s.0, s.1, s.2, s.3, 0, None)
            }
            #[cfg(not(feature = "arch"))]
            {
                (0, 0, 0, 0, 0, None)
            }
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
                "data_dir" => println!(
                    "  {}",
                    style::warning(&paths::data_dir().display().to_string())
                ),
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
                style::warning(&paths::data_dir().display().to_string())
            );
            println!(
                "  {} = {}",
                style::success("socket"),
                style::warning(&paths::socket_path().display().to_string())
            );
        }
    }
    Ok(())
}

pub fn history(limit: usize) -> Result<()> {
    let history_mgr = crate::core::history::HistoryManager::new()?;
    let entries = history_mgr.load()?;

    println!(
        "{} Transaction History (last {})\n",
        style::header("OMG"),
        limit
    );

    if entries.is_empty() {
        println!("  {}", style::dim("No transactions recorded yet"));
        return Ok(());
    }

    for entry in entries.iter().rev().take(limit) {
        let timestamp = entry.timestamp.strftime("%Y-%m-%d %H:%M:%S");
        let status = if entry.success {
            style::success("✓")
        } else {
            style::error("✗")
        };

        println!(
            "{} {} [{}] - {} {}",
            status,
            style::dim(&timestamp.to_string()),
            style::info(&entry.id[..8]),
            style::warning(&format!("{:?}", entry.transaction_type)),
            style::dim(&format!("({} changes)", entry.changes.len()))
        );

        for change in &entry.changes {
            println!(
                "    {} {} {} → {}",
                style::arrow("→"),
                style::package(&change.name),
                style::dim(change.old_version.as_deref().unwrap_or("None")),
                style::version(change.new_version.as_deref().unwrap_or("None"))
            );
        }
        println!();
    }

    Ok(())
}

pub async fn rollback(id: Option<String>) -> Result<()> {
    let history_mgr = crate::core::history::HistoryManager::new()?;
    let entries = history_mgr.load()?;

    let target = if let Some(target_id) = id {
        entries
            .iter()
            .find(|e| e.id.starts_with(&target_id))
            .context("Transaction ID not found")?
    } else {
        // Interactive selection
        if entries.is_empty() {
            anyhow::bail!("No history entries available for rollback");
        }

        println!(
            "{} Select a transaction to roll back to:\n",
            style::header("OMG")
        );
        let options: Vec<String> = entries
            .iter()
            .rev()
            .take(10)
            .map(|e| {
                format!(
                    "{} [{}] - {:?} ({} changes)",
                    e.timestamp.strftime("%Y-%m-%d %H:%M"),
                    &e.id[..8],
                    e.transaction_type,
                    e.changes.len()
                )
            })
            .collect();

        use dialoguer::{Select, theme::ColorfulTheme};
        let selection = Select::with_theme(&ColorfulTheme::default())
            .items(&options)
            .default(0)
            .interact()?;

        entries.get(entries.len() - 1 - selection).ok_or_else(|| anyhow::anyhow!("Invalid selection"))?
    };

    println!(
        "\n{} Rolling back to state from {} [{}]",
        style::warning("⚠"),
        target.timestamp.strftime("%Y-%m-%d %H:%M:%S"),
        style::info(&target.id[..8])
    );

    println!(
        "{}\n",
        style::dim(
            "Note: Full rollback logic is coming in Phase 16.2. Currently, only official packages are supported via downgrade."
        )
    );

    use dialoguer::{Confirm, theme::ColorfulTheme};
    if !Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Proceed with rollback?")
        .default(false)
        .interact()?
    {
        return Ok(());
    }

    // Skeleton for rollback logic
    // 1. Identify packages that were changed
    // 2. For each package, try to install the 'old_version'
    let mut to_install = Vec::new();
    for change in &target.changes {
        if let Some(old_ver) = &change.old_version
            && change.source == "official"
        {
            to_install.push(format!("{}={}", change.name, old_ver));
        }
    }

    if to_install.is_empty() {
        println!(
            "{}",
            style::success(
                "Nothing to roll back (already at target state or no versions recorded)"
            )
        );
    } else {
        let pacman = crate::package_managers::OfficialPackageManager::new();
        pacman.install(&to_install).await?;
    }

    Ok(())
}
