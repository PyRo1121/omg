//! Command implementations for OMG CLI
//!
//! Uses direct libalpm access for 10-100x faster queries.

use anyhow::{Context, Result};
use std::process::{Command, Stdio};

use crate::core::paths;
#[cfg(feature = "arch")]
use crate::package_managers::PackageManager;
#[cfg(feature = "debian")]
use crate::package_managers::{apt_get_system_status, apt_list_all_package_names};

use crate::cli::{style, ui};
use crate::core::env::distro::use_debian_backend;
use dialoguer::{Confirm, Select};
use owo_colors::OwoColorize;

#[cfg(feature = "arch")]
use crate::package_managers::get_system_status;

// Re-export moved commands
pub use super::env::{capture as env_capture, check as env_check, share as env_share};
pub use super::packages::{
    clean, explicit, explicit_sync, info, info_sync, install, remove, search, sync, update,
};
pub use super::runtimes::{list_versions, use_version};

// Const slices for completion - avoids allocation on every call
const TOOL_COMMANDS: &[&str] = &["install", "list", "remove"];
const ENV_COMMANDS: &[&str] = &["capture", "check", "share", "sync"];
const NEW_TEMPLATES: &[&str] = &[
    "rust",
    "react",
    "react-ts",
    "node",
    "ts",
    "typescript",
    "python",
    "py",
    "go",
    "golang",
];
const SHELL_COMPLETIONS: &[&str] = &["bash", "zsh", "fish", "powershell", "elvish"];

/// Convert runtime internal name to human-readable display label.
///
/// Maps short names like "node" to proper branding like "Node.js".
#[inline]
pub fn runtime_display_name(name: &str) -> &str {
    match name {
        "node" => "Node.js",
        "python" => "Python",
        "rust" => "Rust",
        "go" => "Go",
        "bun" => "Bun",
        "java" => "Java",
        "ruby" => "Ruby",
        _ => name,
    }
}

pub async fn complete(_shell: &str, current: &str, last: &str, full: Option<&str>) -> Result<()> {
    let db = crate::core::Database::open(crate::core::Database::default_path()?)?;
    let engine = crate::core::completion::CompletionEngine::new(db);

    let full_tokens: Vec<&str> = full.unwrap_or_default().split_whitespace().collect();
    let in_tool = full_tokens.contains(&"tool");
    let in_env = full_tokens.contains(&"env");

    // Fast path: empty current means show top suggestions only (limit 50 for speed)
    let limit = if current.is_empty() { 50 } else { 200 };

    let suggestions = match last {
        "install" | "i" | "info" => {
            let mut results = complete_package_names(&engine, current, last, in_tool).await?;
            results.truncate(limit);
            results
        }
        "remove" | "r" => {
            // Remove should only show INSTALLED packages
            let mut results = complete_installed_packages(&engine, current)?;
            results.truncate(limit);
            results
        }
        "use" | "ls" | "list" | "which" => complete_runtime_names(&engine, current),
        "tool" => complete_tool_commands(&engine, current),
        "env" => complete_env_commands(&engine, current),
        "run" => complete_task_names(&engine, current)?,
        "new" => complete_templates(&engine, current),
        "completions" => complete_shells(&engine, current),
        _ => {
            let mut results = complete_fallback(&engine, current, last, in_tool, in_env)?;
            results.truncate(limit);
            results
        }
    };

    output_suggestions(&engine, current, suggestions);
    Ok(())
}

/// Complete package names for install/remove/info commands
async fn complete_package_names(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
    last: &str,
    in_tool: bool,
) -> Result<Vec<String>> {
    // Handle tool subcommands
    if in_tool && last == "install" {
        return Ok(engine.fuzzy_match(current, crate::cli::tool::registry_tool_names()));
    }
    if in_tool && last == "remove" {
        return Ok(engine.fuzzy_match(
            current,
            crate::cli::tool::installed_tool_names()
                .context("Failed to list installed tools for completion")?,
        ));
    }

    // Get package names from daemon or fallback to direct ALPM.
    // Official lookup failures fail closed; AUR names remain optional enrichment.
    #[allow(unused_mut)] // Mutated only inside feature-gated block
    let mut names = get_package_names_with_fallback().await?;

    // Include AUR packages on Arch
    #[cfg(feature = "arch")]
    if let Ok(aur_names) = engine.get_aur_package_names().await {
        names.extend(aur_names);
        names.sort();
        names.dedup();
    }

    Ok(engine.fuzzy_match(current, names))
}

/// Complete installed package names for remove command
fn complete_installed_packages(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
) -> Result<Vec<String>> {
    let names = get_installed_package_names()?;
    Ok(engine.fuzzy_match(current, names))
}

/// Get installed package names for remove completion
#[cfg_attr(
    not(feature = "arch"),
    expect(
        clippy::unnecessary_wraps,
        reason = "feature-gated backends can return package lookup errors"
    )
)]
fn get_installed_package_names() -> Result<Vec<String>> {
    #[cfg(feature = "arch")]
    {
        return crate::package_managers::list_installed_fast()
            .map(|installed| installed.into_iter().map(|pkg| pkg.name).collect())
            .context("Failed to list installed packages for completion");
    }

    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    {
        return crate::package_managers::debian_db::list_installed_fast()
            .map(|installed| installed.into_iter().map(|pkg| pkg.name).collect())
            .context("Failed to list installed packages for completion");
    }

    #[allow(unreachable_code)]
    Ok(Vec::new())
}

/// Get package names from daemon with fallback to direct access
async fn get_package_names_with_fallback() -> Result<Vec<String>> {
    if use_debian_backend() {
        #[cfg(feature = "debian")]
        return apt_list_all_package_names()
            .context("Failed to list official package names for completion");
        #[cfg(not(feature = "debian"))]
        return Ok(Vec::new());
    }

    // Try daemon first. A missing or down daemon is expected and falls back.
    #[cfg(unix)]
    if let Ok(mut client) = crate::core::client::DaemonClient::connect().await {
        match client.search("", None).await {
            Ok(res) => return Ok(res.packages.into_iter().map(|pkg| pkg.name).collect()),
            Err(error) => {
                tracing::debug!("Daemon package-name search failed, using local lookup: {error}");
            }
        }
    }

    // Fallback to direct ALPM. Local lookup failures fail closed.
    #[cfg(feature = "arch")]
    return crate::package_managers::alpm_direct::list_all_package_names()
        .context("Failed to list official package names for completion");
    #[cfg(not(feature = "arch"))]
    Ok(Vec::new())
}

/// Complete runtime names
#[inline]
fn complete_runtime_names(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
) -> Vec<String> {
    let runtimes = crate::cli::runtimes::known_runtimes().unwrap_or_else(|error| {
        tracing::debug!("Failed to list mise-installed runtimes for completion: {error}");
        crate::runtimes::SUPPORTED_RUNTIMES
            .iter()
            .map(std::string::ToString::to_string)
            .collect()
    });
    engine.fuzzy_match(current, runtimes)
}

/// Complete tool subcommands
#[inline]
fn complete_tool_commands(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
) -> Vec<String> {
    engine.fuzzy_match(
        current,
        TOOL_COMMANDS
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
    )
}

/// Complete env subcommands
#[inline]
fn complete_env_commands(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
) -> Vec<String> {
    engine.fuzzy_match(
        current,
        ENV_COMMANDS
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
    )
}

/// Complete task runner task names
#[inline]
fn complete_task_names(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
) -> Result<Vec<String>> {
    let tasks = crate::core::task_runner::detect_tasks()
        .context("Failed to detect project tasks for completion")?;
    let names = tasks.into_iter().map(|task| task.name).collect();
    Ok(engine.fuzzy_match(current, names))
}

/// Complete new project templates
#[inline]
fn complete_templates(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
) -> Vec<String> {
    engine.fuzzy_match(
        current,
        NEW_TEMPLATES
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
    )
}

/// Complete shell names for completion generation
#[inline]
fn complete_shells(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
) -> Vec<String> {
    engine.fuzzy_match(
        current,
        SHELL_COMPLETIONS
            .iter()
            .map(std::string::ToString::to_string)
            .collect(),
    )
}

/// Fallback completion for runtime versions and other contexts
fn complete_fallback(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
    last: &str,
    in_tool: bool,
    in_env: bool,
) -> Result<Vec<String>> {
    // Check if completing runtime version (e.g., 'omg use node <TAB>')
    if crate::cli::runtimes::known_runtimes()
        .unwrap_or_else(|_| {
            crate::runtimes::SUPPORTED_RUNTIMES
                .iter()
                .map(std::string::ToString::to_string)
                .collect()
        })
        .iter()
        .any(|rt| rt == last)
    {
        return complete_runtime_versions(engine, current, last);
    }

    // Fallback to tool/env subcommands if in those contexts
    if in_env {
        return Ok(engine.fuzzy_match(
            current,
            ENV_COMMANDS
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
        ));
    }
    if in_tool {
        return Ok(engine.fuzzy_match(
            current,
            TOOL_COMMANDS
                .iter()
                .map(std::string::ToString::to_string)
                .collect(),
        ));
    }

    Ok(Vec::new())
}

/// Complete runtime version numbers
fn complete_runtime_versions(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
    runtime: &str,
) -> Result<Vec<String>> {
    // Priority 1: Context awareness (package.json, .nvmrc, etc.)
    let mut suggestions = engine.probe_context(runtime);

    // Priority 2: Installed versions
    let data_dir = crate::core::Database::default_path()?
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Invalid database path"))?
        .to_path_buf();
    let runtime_dir = data_dir.join("versions").join(runtime);
    let installed_versions = crate::runtimes::common::list_installed_versions(&runtime_dir)
        .with_context(|| format!("Failed to list installed {runtime} versions for completion"))?;

    let fuzzy_installed = engine.fuzzy_match(current, installed_versions);
    suggestions.extend(fuzzy_installed);
    suggestions.dedup();
    Ok(suggestions)
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

    // Modern header without old styling
    use crate::cli::modern_ui;
    modern_ui::print_phase_header("📊", "System Status", "overview");

    // ULTRA FAST: Try binary status file first (zero IPC, sub-ms)
    let (total, explicit, orphans, updates, security_vulnerabilities, cached_runtimes) =
        if let Some(fast) = crate::core::fast_status::FastStatus::read_from_file(
            &crate::core::paths::fast_status_path(),
        ) {
            (
                fast.total_packages as usize,
                fast.explicit_packages as usize,
                fast.orphan_packages as usize,
                fast.updates_available as usize,
                0,
                None::<Vec<(String, String)>>,
            )
        } else if use_debian_backend() {
            #[cfg(feature = "debian")]
            {
                let s = apt_get_system_status().unwrap_or((0, 0, 0, 0));
                (s.0, s.1, s.2, s.3, 0, None::<Vec<(String, String)>>)
            }
            #[cfg(not(feature = "debian"))]
            {
                (0, 0, 0, 0, 0, None::<Vec<(String, String)>>)
            }
        } else {
            // Try daemon (Unix only), then fallback to local status
            #[cfg(unix)]
            {
                if let Ok(mut client) = crate::core::client::DaemonClient::connect_sync() {
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
                            (s.0, s.1, s.2, s.3, 0, None::<Vec<(String, String)>>)
                        }
                        #[cfg(not(feature = "arch"))]
                        {
                            (0, 0, 0, 0, 0, None::<Vec<(String, String)>>)
                        }
                    }
                } else {
                    // Fallback to local optimized ALPM query
                    #[cfg(feature = "arch")]
                    {
                        let s = get_system_status().unwrap_or((0, 0, 0, 0));
                        (s.0, s.1, s.2, s.3, 0, None::<Vec<(String, String)>>)
                    }
                    #[cfg(not(feature = "arch"))]
                    {
                        (0, 0, 0, 0, 0, None::<Vec<(String, String)>>)
                    }
                }
            }
            #[cfg(not(unix))]
            {
                // Windows fallback
                #[cfg(feature = "arch")]
                {
                    let s = get_system_status().unwrap_or((0, 0, 0, 0));
                    (s.0, s.1, s.2, s.3, 0, None::<Vec<(String, String)>>)
                }
                #[cfg(not(feature = "arch"))]
                {
                    (0, 0, 0, 0, 0, None::<Vec<(String, String)>>)
                }
            }
        };

    // Modern minimal status layout - no box drawing
    println!();

    // Package stats
    println!(
        "  {} {} total, {} explicit",
        "Packages".bold(),
        total.to_string().cyan(),
        explicit.to_string().cyan()
    );

    // Updates
    if updates > 0 {
        println!(
            "  {} {} available",
            "Updates".bold(),
            updates.to_string().yellow().bold()
        );
    } else {
        println!("  {} {}", "Updates".bold(), "Up to date".green());
    }

    // Orphans (only show if > 0)
    if orphans > 0 {
        println!(
            "  {} {}",
            "Orphans".bold(),
            format!("{orphans} packages").yellow()
        );
    }

    // Security
    if security_vulnerabilities > 0 {
        println!(
            "  {} {}",
            "Security".bold(),
            format!("{security_vulnerabilities} vulnerabilities")
                .red()
                .bold()
        );
    } else {
        println!("  {} {}", "Security".bold(), "No known issues".green());
    }

    // Daemon status
    #[cfg(unix)]
    {
        let socket = crate::core::client::default_socket_path();
        if socket.exists() {
            println!("  {} {}", "Daemon".bold(), "Running".green());
        } else {
            println!("  {} {}", "Daemon".bold(), "Offline".dimmed());
        }
    }

    // Runtimes - INSTANT FROM CACHE (Unix only, from daemon)
    #[cfg(unix)]
    {
        println!();
        println!("  {}", "Runtimes".bold());

        if let Some(versions) = cached_runtimes {
            for (rt_name, v) in &versions {
                let label = runtime_display_name(rt_name);
                println!("    {} {} {}", "·".cyan(), label, v.dimmed());
            }
        } else {
            // Fallback to local probing if daemon is down
            for rt_name in &["node", "python", "rust", "go", "bun", "java", "ruby"] {
                if let Some(v) = crate::runtimes::probe_version(rt_name) {
                    let label = runtime_display_name(rt_name);
                    println!("    {} {} {}", "·".cyan(), label, v.dimmed());
                }
            }
        }
    }

    // Modern tip styling
    println!();
    if updates > 0 {
        println!(
            "  {} Run {} to see detailed package changes",
            "Tip".dimmed().italic(),
            "omg update --check".cyan()
        );
    } else if security_vulnerabilities > 0 {
        println!(
            "  {} Run {} to identify and fix security issues",
            "Tip".dimmed().italic(),
            "omg audit".cyan()
        );
    } else {
        println!(
            "  {} Your system is healthy! Run {} for an interactive view",
            "Tip".dimmed().italic(),
            "omg dash".cyan()
        );
    }

    println!();
    Ok(())
}

/// Show system metrics in Prometheus format
#[cfg(unix)]
pub async fn metrics() -> Result<()> {
    let mut client = crate::core::client::DaemonClient::connect().await?;

    match client
        .call(crate::daemon::protocol::Request::Metrics { id: 0 })
        .await?
    {
        crate::daemon::protocol::ResponseResult::Metrics(snapshot) => {
            // Output in Prometheus text format
            println!("# HELP omg_requests_total Total number of requests handled");
            println!("# TYPE omg_requests_total counter");
            println!("omg_requests_total {}", snapshot.requests_total);

            println!("# HELP omg_requests_failed_total Total number of failed requests");
            println!("# TYPE omg_requests_failed_total counter");
            println!("omg_requests_failed_total {}", snapshot.requests_failed);

            println!("# HELP omg_rate_limit_hits_total Total number of rate limit exceeded events");
            println!("# TYPE omg_rate_limit_hits_total counter");
            println!("omg_rate_limit_hits_total {}", snapshot.rate_limit_hits);

            println!(
                "# HELP omg_validation_failures_total Total number of input validation failures"
            );
            println!("# TYPE omg_validation_failures_total counter");
            println!(
                "omg_validation_failures_total {}",
                snapshot.validation_failures
            );

            println!("# HELP omg_active_connections Number of currently active client connections");
            println!("# TYPE omg_active_connections gauge");
            println!("omg_active_connections {}", snapshot.active_connections);

            println!(
                "# HELP omg_security_audit_requests_total Total number of security audits performed"
            );
            println!("# TYPE omg_security_audit_requests_total counter");
            println!(
                "omg_security_audit_requests_total {}",
                snapshot.security_audit_requests
            );

            println!("# HELP omg_bytes_received_total Total bytes received by daemon");
            println!("# TYPE omg_bytes_received_total counter");
            println!("omg_bytes_received_total {}", snapshot.bytes_received);

            println!("# HELP omg_bytes_sent_total Total bytes sent by daemon");
            println!("# TYPE omg_bytes_sent_total counter");
            println!("omg_bytes_sent_total {}", snapshot.bytes_sent);
        }
        _ => anyhow::bail!("Unexpected response from daemon"),
    }
    Ok(())
}

/// Start the daemon
#[cfg(unix)]
pub fn daemon(foreground: bool) -> Result<()> {
    if foreground {
        println!("{} Run 'omgd' directly for daemon mode", style::info("→"));
    } else {
        // Check if daemon is already running
        let socket_path = crate::core::client::default_socket_path();

        if let Ok(mut client) = crate::core::client::DaemonClient::connect_sync()
            && client.ping_sync().is_ok()
        {
            println!(
                "{} Daemon is already running at {}",
                style::success("✓"),
                socket_path.display()
            );
            return Ok(());
        }

        // SECURITY: Atomic remove avoids TOCTOU race condition
        if let Err(e) = std::fs::remove_file(&socket_path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            return Err(e.into());
        }

        // Start daemon in background
        // 1. Try to find omgd in the same directory as the current executable (ensures version match)
        let omgd_path = if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            let local_omgd = dir.join("omgd");
            if local_omgd.exists() {
                local_omgd
            } else {
                std::path::PathBuf::from("omgd")
            }
        } else {
            std::path::PathBuf::from("omgd")
        };

        let status = Command::new(omgd_path)
            .arg("--")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();

        match status {
            Ok(_) => {
                // Poll for daemon readiness: 30 attempts × 100ms = 3 seconds max.
                // The daemon needs time to build its in-memory index and bind the socket,
                // which can take >500ms on large package databases.
                let mut started = false;
                for _ in 0..30 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if socket_path.exists()
                        && let Ok(mut client) = crate::core::client::DaemonClient::connect_sync()
                        && client.ping_sync().is_ok()
                    {
                        started = true;
                        break;
                    }
                }

                if started {
                    println!("{} Daemon started", style::success("✓"));
                } else if socket_path.exists() {
                    println!(
                        "{} Daemon started but not responding (check logs)",
                        style::warning("⚠")
                    );
                } else {
                    println!(
                        "{} Daemon started but socket not created (check logs)",
                        style::warning("⚠")
                    );
                }
            }
            Err(e) => println!("{} Failed to start daemon: {}", style::error("✗"), e),
        }
    }
    Ok(())
}

/// Get or set configuration
pub fn config(key: Option<&str>, value: Option<&str>) -> Result<()> {
    if let Some(k) = key {
        // SECURITY: Validate config key
        if k.chars()
            .any(|c| !c.is_ascii_alphanumeric() && c != '.' && c != '_')
        {
            anyhow::bail!("Invalid configuration key: {k}");
        }
    }

    match (key, value) {
        (Some(k), Some(v)) => {
            // SECURITY: Basic validation for values
            if v.len() > 1024 {
                anyhow::bail!("Configuration value too long");
            }

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

pub fn history(
    limit: usize,
    search: Option<&str>,
    transaction_type: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<()> {
    let history_mgr = crate::core::history::HistoryManager::new()?;
    let entries = history_mgr.load()?;

    // Parse optional transaction type filter
    let type_filter = transaction_type.map(|t| match t.to_lowercase().as_str() {
        "install" => crate::core::history::TransactionType::Install,
        "remove" => crate::core::history::TransactionType::Remove,
        "update" => crate::core::history::TransactionType::Update,
        _ => crate::core::history::TransactionType::Sync,
    });

    // Parse date filters
    let from_date = from.and_then(|d| jiff::civil::Date::strptime("%Y-%m-%d", d).ok());
    let to_date = to.and_then(|d| jiff::civil::Date::strptime("%Y-%m-%d", d).ok());

    // Build header
    let header = if search.is_some() || transaction_type.is_some() || from.is_some() || to.is_some()
    {
        "Transaction History (filtered)".to_string()
    } else {
        format!("Transaction History (last {limit})")
    };

    println!("{} {}\n", style::header("OMG"), header);

    if entries.is_empty() {
        println!("  {}", style::dim("No transactions recorded yet"));
        return Ok(());
    }

    // Filter entries
    let filtered: Vec<_> = entries
        .iter()
        .rev()
        .filter(|entry| {
            // Filter by transaction type
            if let Some(ref t) = type_filter
                && entry.transaction_type != *t
            {
                return false;
            }

            // Filter by search term (package name)
            if let Some(query) = search {
                let query_lower = query.to_lowercase();
                let matches = entry
                    .changes
                    .iter()
                    .any(|c| c.name.to_lowercase().contains(&query_lower));
                if !matches {
                    return false;
                }
            }

            // Filter by date range
            if let Some(ref from_d) = from_date {
                let entry_date = entry
                    .timestamp
                    .to_zoned(jiff::tz::TimeZone::system())
                    .date();
                if entry_date < *from_d {
                    return false;
                }
            }
            if let Some(ref to_d) = to_date {
                let entry_date = entry
                    .timestamp
                    .to_zoned(jiff::tz::TimeZone::system())
                    .date();
                if entry_date > *to_d {
                    return false;
                }
            }

            true
        })
        .take(limit)
        .collect();

    if filtered.is_empty() {
        println!("  {}", style::dim("No matching transactions found"));
        if search.is_some() {
            println!(
                "  {}",
                style::dim("Try a different search term or remove filters.")
            );
        }
        return Ok(());
    }

    for entry in filtered {
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
            style::warning(&entry.transaction_type.to_string()),
            style::dim(&format!("({} changes)", entry.changes.len()))
        );

        // If searching, highlight matching packages
        for change in &entry.changes {
            let pkg_display = if let Some(query) = search {
                if change.name.to_lowercase().contains(&query.to_lowercase()) {
                    style::success(&change.name)
                } else {
                    style::package(&change.name)
                }
            } else {
                style::package(&change.name)
            };

            println!(
                "    {} {} {} → {}",
                style::arrow("→"),
                pkg_display,
                style::dim(change.old_version.as_deref().unwrap_or("None")),
                style::version(change.new_version.as_deref().unwrap_or("None"))
            );
        }
        println!();
    }

    Ok(())
}

#[allow(clippy::unused_async)] // Async required: feature-gated branches call .await; fallback stubs omit it
pub async fn rollback(id: Option<String>, yes: bool) -> Result<()> {
    if let Some(ref r_id) = id {
        // SECURITY: Validate transaction ID
        if r_id.chars().any(|c| !c.is_ascii_hexdigit()) {
            anyhow::bail!("Invalid transaction ID format");
        }
    }

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

        ui::print_header("OMG", "Rollback Transaction");
        ui::print_spacer();

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

        let selection = Select::with_theme(&ui::prompt_theme())
            .items(&options)
            .default(0)
            .interact()?;

        entries
            .get(entries.len() - 1 - selection)
            .ok_or_else(|| anyhow::anyhow!("Invalid selection"))?
    };

    println!(
        "\n{} Rolling back to state from {} [{}]",
        style::warning("⚠"),
        target.timestamp.strftime("%Y-%m-%d %H:%M:%S"),
        style::info(&target.id[..8])
    );

    // Check if we're in interactive mode
    if console::user_attended()
        && !Confirm::with_theme(&ui::prompt_theme())
            .with_prompt("Proceed with rollback?")
            .default(false)
            .interact()?
    {
        return Ok(());
    }

    // Non-interactive mode: require --yes flag for destructive operation
    if !yes {
        anyhow::bail!(
            "This destructive command requires --yes flag in non-interactive mode.\n\n\
                 For automation/CI, use: omg rollback <id> --yes\n\
                 Or run in interactive mode to select a transaction."
        );
    }

    // Identify packages that were changed and build install list
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
        #[cfg(feature = "arch")]
        {
            println!(
                "{} Rolling back {} packages...",
                style::info("→"),
                to_install.len()
            );
            let pacman = crate::package_managers::ArchPackageManager::new();
            pacman.install(&to_install).await?;
            println!("{}", style::success("✓ Rollback completed successfully"));
        }

        #[cfg(feature = "debian")]
        {
            if crate::core::env::distro::use_debian_backend() {
                #[cfg(feature = "debian")]
                {
                    println!(
                        "{} Rolling back {} packages...",
                        style::info("→"),
                        to_install.len()
                    );
                    let apt = crate::package_managers::AptPackageManager::new();
                    use crate::package_managers::PackageManager as _;
                    apt.install(&to_install).await?;
                    println!("{}", style::success("✓ Rollback completed successfully"));
                }

                #[cfg(not(feature = "debian"))]
                {
                    println!(
                        "{} Rollback requires APT package installation support.",
                        style::warning("⚠")
                    );
                    println!(
                        "{}",
                        style::dim(
                            "To enable full rollback support, rebuild with 'debian' feature."
                        )
                    );
                    println!(
                        "{}",
                        style::dim(
                            "Alternatively, you can manually install the following packages:"
                        )
                    );
                    for pkg in &to_install {
                        println!("  apt install {}", style::package(pkg));
                    }
                }
            } else {
                anyhow::bail!("Rollback is only supported on Arch Linux and Debian/Ubuntu systems");
            }
        }

        #[cfg(not(any(feature = "arch", feature = "debian")))]
        {
            println!(
                "{} Rollback requires package manager support.",
                style::warning("⚠")
            );
            println!(
                "{}",
                style::dim("The following packages need to be restored:")
            );
            for pkg in &to_install {
                println!("  {}", style::package(pkg));
            }
            anyhow::bail!("Rollback is only supported on Arch Linux and Debian/Ubuntu systems");
        }
    }

    Ok(())
}

/// Show usage statistics
pub fn stats() -> Result<()> {
    use crate::cli::style;
    use crate::core::usage::UsageStats;

    let stats = UsageStats::load();

    println!("\n{}", style::header("OMG Usage Statistics"));
    println!("{}", style::dim(&"─".repeat(50)));

    // Time saved
    println!(
        "\n  {} {}",
        style::success("Time Saved:"),
        style::header(&stats.time_saved_human())
    );

    // Total commands
    println!(
        "  {} {}",
        style::info("Total Commands:"),
        stats.total_commands
    );

    // Today's activity
    println!(
        "  {} {}",
        style::info("Queries Today:"),
        stats.queries_today
    );
    println!(
        "  {} {}",
        style::info("Queries This Month:"),
        stats.queries_this_month
    );

    // Top commands
    let top = stats.top_commands();
    if !top.is_empty() {
        println!("\n  {}", style::header("Most Used Commands:"));
        for (cmd, count) in top {
            println!(
                "    {} {} ({}x)",
                style::arrow("→"),
                style::command(&cmd),
                count
            );
        }
    }

    // Streak info
    if stats.current_streak > 0 {
        println!(
            "\n  {} {} day streak {}",
            style::success("🔥 Current Streak:"),
            stats.current_streak,
            if stats.current_streak == stats.longest_streak {
                "(personal best!)"
            } else {
                ""
            }
        );
        if stats.longest_streak > stats.current_streak {
            println!(
                "    {} Longest: {} days",
                style::dim("📊"),
                stats.longest_streak
            );
        }
    }

    // Achievements
    if !stats.achievements.is_empty() {
        println!("\n  {}", style::header("🏆 Achievements:"));
        for achievement in &stats.achievements {
            println!(
                "    {} {} - {}",
                achievement.emoji(),
                style::success(achievement.name()),
                style::dim(achievement.description())
            );
        }
    }

    // Pro features (if applicable)
    if stats.sbom_generated > 0 || stats.vulnerabilities_found > 0 {
        println!("\n  {}", style::header("Security Stats:"));
        println!(
            "    {} SBOMs Generated: {}",
            style::arrow("→"),
            stats.sbom_generated
        );
        println!(
            "    {} Vulnerabilities Found: {}",
            style::arrow("→"),
            stats.vulnerabilities_found
        );
    }

    // Dollar savings calculation
    let time_saved_hours = stats.time_saved_ms as f64 / 3_600_000.0;
    let hourly_rate = 150_000.0 / 2080.0; // $150k/year, 2080 work hours
    let dollar_savings = time_saved_hours * hourly_rate;

    if dollar_savings > 0.01 {
        println!(
            "\n  {} ${:.2}",
            style::success("Estimated Value Saved:"),
            dollar_savings
        );
        println!(
            "    {}",
            style::dim("(Based on $150k/yr avg software engineer salary)")
        );
    }

    println!();

    // Sync hint
    if let Some(license) = crate::core::license::load_license() {
        println!(
            "  {} Synced to dashboard ({})",
            style::success("✓"),
            style::info(&license.tier)
        );
    } else {
        println!(
            "  {} {}",
            style::dim("Tip:"),
            style::dim("Register at pyro1121.com to sync stats to your dashboard")
        );
    }

    println!();
    Ok(())
}
