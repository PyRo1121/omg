//! Command implementations for OMG CLI
//!
//! Uses direct libalpm access for 10-100x faster queries.

use anyhow::{Context, Result};
use std::process::{Command, Stdio};

#[cfg(feature = "arch")]
use crate::package_managers::PackageManager;
#[cfg(feature = "debian")]
use crate::package_managers::{apt_get_system_status, apt_list_all_package_names};

use crate::cli::{TransactionTypeFilter, style, ui};
use crate::core::env::distro::use_debian_backend;
use dialoguer::{Confirm, Select};
use owo_colors::OwoColorize;

#[cfg(feature = "arch")]
use crate::package_managers::get_system_status;

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
#[must_use]
fn runtime_display_name(name: &str) -> &str {
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
    let engine = crate::core::completion::CompletionEngine::new();

    let full = full.unwrap_or_default();
    let in_tool = full.split_whitespace().any(|token| token == "tool");
    let in_env = full.split_whitespace().any(|token| token == "env");

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
        "tool" => complete_static_candidates(&engine, current, TOOL_COMMANDS),
        "env" => complete_static_candidates(&engine, current, ENV_COMMANDS),
        "run" => complete_task_names(&engine, current)?,
        "new" => complete_static_candidates(&engine, current, NEW_TEMPLATES),
        "completions" => complete_static_candidates(&engine, current, SHELL_COMPLETIONS),
        _ => {
            let mut results = complete_fallback(&engine, current, last, in_tool, in_env)?;
            results.truncate(limit);
            results
        }
    };

    for suggestion in suggestions {
        println!("{suggestion}");
    }
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
    // Official lookup failures fail closed; AUR names remain optional enrichment.
    #[allow(
        unused_mut,
        reason = "mutated only when the Arch completion branch is compiled"
    )]
    let mut names = get_package_names_with_fallback().await?;

    // Include AUR packages on Arch. Skip on Debian even if Arch is compiled in.
    #[cfg(feature = "arch")]
    {
        #[cfg(any(feature = "debian", feature = "debian-pure"))]
        if crate::core::env::distro::is_debian_like() {
            return Ok(engine.fuzzy_match(current, names));
        }

        if let Ok(aur_names) = engine.get_aur_package_names().await {
            names.extend(aur_names);
            names.sort();
            names.dedup();
        }
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
#[allow(
    clippy::needless_return,
    reason = "additive backend feature branches return before compiled fallbacks"
)]
fn get_installed_package_names() -> Result<Vec<String>> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return crate::package_managers::debian_db::list_installed_fast()
            .map(|installed| installed.into_iter().map(|pkg| pkg.name).collect())
            .context("Failed to list installed packages for completion");
    }

    #[cfg(feature = "arch")]
    {
        #[allow(
            clippy::needless_return,
            reason = "required when additive backend features compile later fallback blocks"
        )]
        return crate::package_managers::list_installed_fast()
            .map(|installed| installed.into_iter().map(|pkg| pkg.name).collect())
            .context("Failed to list installed packages for completion");
    }

    #[cfg(all(
        any(feature = "debian", feature = "debian-pure"),
        not(feature = "arch")
    ))]
    {
        return crate::package_managers::debian_db::list_installed_fast()
            .map(|installed| installed.into_iter().map(|pkg| pkg.name).collect())
            .context("Failed to list installed packages for completion");
    }

    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    package_completion_requires_backend()
}

/// Get package names from daemon with fallback to direct access
#[allow(
    clippy::needless_return,
    reason = "additive backend feature branches return before compiled fallbacks"
)]
async fn get_package_names_with_fallback() -> Result<Vec<String>> {
    #[cfg(feature = "debian")]
    if use_debian_backend() {
        return apt_list_all_package_names()
            .context("Failed to list official package names for completion");
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

    // Fallback to a direct query. Local lookup failures fail closed.
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return crate::package_managers::debian_db::search_fast("")
            .map(|packages| packages.into_iter().map(|pkg| pkg.name).collect())
            .context("Failed to list official package names for completion");
    }

    #[cfg(feature = "arch")]
    return crate::package_managers::alpm_direct::list_all_package_names()
        .context("Failed to list official package names for completion");

    #[cfg(all(feature = "debian", not(feature = "arch")))]
    return apt_list_all_package_names()
        .context("Failed to list official package names for completion");

    #[cfg(all(
        feature = "debian-pure",
        not(feature = "arch"),
        not(feature = "debian")
    ))]
    {
        return crate::package_managers::debian_db::search_fast("")
            .map(|packages| packages.into_iter().map(|pkg| pkg.name).collect())
            .context("Failed to list official package names for completion");
    }

    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    package_completion_requires_backend()
}

#[cfg(any(
    not(any(feature = "arch", feature = "debian", feature = "debian-pure")),
    test
))]
fn package_completion_requires_backend() -> Result<Vec<String>> {
    anyhow::bail!(
        "Package name completion is not available without an Arch or Debian package backend"
    )
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

fn complete_static_candidates(
    engine: &crate::core::completion::CompletionEngine,
    current: &str,
    candidates: &[&str],
) -> Vec<String> {
    engine.fuzzy_match(
        current,
        candidates.iter().map(ToString::to_string).collect(),
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
        return Ok(complete_static_candidates(engine, current, ENV_COMMANDS));
    }
    if in_tool {
        return Ok(complete_static_candidates(engine, current, TOOL_COMMANDS));
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
    let mut suggestions = engine.probe_context(runtime)?;

    // Priority 2: Installed versions
    let data_dir = crate::core::paths::data_dir();
    let runtime_dir = data_dir.join("versions").join(runtime);
    let installed_versions = crate::runtimes::common::list_installed_versions(&runtime_dir)
        .with_context(|| format!("Failed to list installed {runtime} versions for completion"))?;

    let fuzzy_installed = engine.fuzzy_match(current, installed_versions);
    suggestions.extend(fuzzy_installed);
    suggestions.dedup();
    Ok(engine.fuzzy_match(current, suggestions))
}

type StatusSnapshot = (
    usize,
    usize,
    usize,
    usize,
    Option<usize>,
    Option<Vec<(String, String)>>,
);

/// Read a status snapshot, failing closed when the underlying lookup fails.
/// A missing daemon or binary cache falls back to a direct query, but a failed
/// direct query is an error rather than a fake "healthy" zero report.
fn read_status_snapshot() -> Result<StatusSnapshot> {
    // ULTRA FAST: Try binary status file first (zero IPC, sub-ms)
    if let Some(fast) = crate::core::fast_status::FastStatus::read_from_file(
        &crate::core::paths::fast_status_path(),
    ) {
        return Ok((
            fast.total_packages as usize,
            fast.explicit_packages as usize,
            fast.orphan_packages as usize,
            fast.updates_available as usize,
            None,
            None,
        ));
    }

    if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            let s = apt_get_system_status().context("Failed to query system status from apt")?;
            return Ok((s.0, s.1, s.2, s.3, None, None));
        }
        #[cfg(not(feature = "debian"))]
        {
            anyhow::bail!("Debian backend is not compiled in");
        }
    }

    // Try the daemon on Unix, then fall back to the platform's direct query.
    #[cfg(unix)]
    if let Ok(mut client) = crate::core::client::DaemonClient::connect_sync()
        && let Ok(crate::daemon::protocol::ResponseResult::Status(res)) =
            client.call_sync(&crate::daemon::protocol::Request::Status { id: 0 })
    {
        return Ok((
            res.total_packages,
            res.explicit_packages,
            res.orphan_packages,
            res.updates_available,
            res.scanned_vulnerability_count(),
            Some(res.runtime_versions),
        ));
    }

    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        let s = crate::package_managers::debian_db::get_counts_fast()
            .context("Failed to query system status from the Debian package database")?;
        return Ok((s.0, s.1, s.2, s.3, None, None));
    }

    #[cfg(feature = "arch")]
    {
        let s = get_system_status().context("Failed to query system status via ALPM")?;
        Ok((s.0, s.1, s.2, s.3, None, None))
    }
    #[cfg(all(
        any(feature = "debian", feature = "debian-pure"),
        not(feature = "arch")
    ))]
    {
        let s = crate::package_managers::debian_db::get_counts_fast()
            .context("Failed to query system status from the Debian package database")?;
        Ok((s.0, s.1, s.2, s.3, None, None))
    }
    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    status_requires_backend()
}

#[cfg(any(
    not(any(feature = "arch", feature = "debian", feature = "debian-pure")),
    test
))]
fn status_requires_backend() -> Result<StatusSnapshot> {
    anyhow::bail!("No supported package manager backend available")
}

pub fn status_sync() -> Result<()> {
    // Modern header without old styling
    use crate::cli::modern_ui;
    modern_ui::print_phase_header("📊", "System Status", "overview");

    let (total, explicit, orphans, updates, security_vulnerabilities, cached_runtimes) =
        read_status_snapshot()?;

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
    match security_vulnerabilities {
        Some(0) => {
            println!("  {} {}", "Security".bold(), "No known issues".green());
        }
        Some(count) => {
            println!(
                "  {} {}",
                "Security".bold(),
                format!("{count} vulnerabilities").red().bold()
            );
        }
        None => {
            println!("  {} {}", "Security".bold(), "Not scanned".dimmed());
        }
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
    } else if security_vulnerabilities.is_some_and(|count| count > 0) {
        println!(
            "  {} Run {} to identify and fix security issues",
            "Tip".dimmed().italic(),
            "omg audit".cyan()
        );
    } else if security_vulnerabilities.is_none() {
        println!(
            "  {} Run {} to scan for vulnerabilities",
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
    let response = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        let mut client = crate::core::client::DaemonClient::connect().await?;
        client
            .call(crate::daemon::protocol::Request::Metrics { id: 0 })
            .await
    })
    .await
    .context("Timed out waiting for daemon metrics")??;

    match response {
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

        let spawn = status.map(|_| ()).map_err(|e| e.to_string());
        let mut ready = false;
        if spawn.is_ok() {
            // Poll for daemon readiness: 30 attempts × 100ms = 3 seconds max.
            // The daemon needs time to build its in-memory index and bind the socket,
            // which can take >500ms on large package databases.
            // NOTE: this is an intentional short synchronous block on the CLI's own
            // current-thread runtime; the command does nothing else while waiting and
            // the process exits right after, so no other task is starved.
            for _ in 0..30 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if socket_path.exists()
                    && let Ok(mut client) = crate::core::client::DaemonClient::connect_sync()
                    && client.ping_sync().is_ok()
                {
                    ready = true;
                    break;
                }
            }
        }

        return daemon_start_result(spawn, ready, socket_path.exists());
    }
    Ok(())
}

#[cfg(unix)]
fn daemon_start_result(spawn: Result<(), String>, ready: bool, socket_exists: bool) -> Result<()> {
    // Failure branches report through the error return only; the top-level
    // error handler prints it once (no duplicate println+bail reporting).
    match spawn {
        Ok(()) if ready => {
            println!("{} Daemon started", style::success("✓"));
            Ok(())
        }
        Ok(()) if socket_exists => {
            anyhow::bail!("Daemon started but not responding (check logs)")
        }
        Ok(()) => {
            anyhow::bail!("Daemon started but socket not created (check logs)")
        }
        Err(e) => {
            anyhow::bail!("Failed to start daemon: {e}")
        }
    }
}

#[cfg(all(test, unix))]
mod daemon_start_tests {
    use super::daemon_start_result;

    #[test]
    fn spawn_failure_returns_err() {
        let result = daemon_start_result(Err("omgd not found".to_string()), false, false);
        assert!(
            result.is_err(),
            "failed daemon spawn must be a CLI error so the process exits non-zero"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to start daemon: omgd not found"),
            "original spawn error must be preserved"
        );
    }

    #[test]
    fn not_ready_returns_err() {
        let result = daemon_start_result(Ok(()), false, true);
        assert!(
            result.is_err(),
            "a daemon that never becomes ready must be a CLI error"
        );
        assert!(
            result.unwrap_err().to_string().contains("not responding"),
            "not-ready error must describe the failure"
        );
    }

    #[test]
    fn socket_missing_returns_err() {
        let result = daemon_start_result(Ok(()), false, false);
        assert!(
            result.is_err(),
            "a daemon that never creates its socket must be a CLI error"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("socket not created"),
            "missing-socket error must describe the failure"
        );
    }

    #[test]
    fn ready_returns_ok() {
        assert!(
            daemon_start_result(Ok(()), true, true).is_ok(),
            "a ready daemon is a successful start"
        );
    }
}

/// First 8 characters of a transaction ID, tolerating shorter persisted IDs.
fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}

pub fn history(
    limit: usize,
    search: Option<&str>,
    transaction_type: Option<TransactionTypeFilter>,
    from: Option<&str>,
    to: Option<&str>,
    json: bool,
) -> Result<()> {
    let history_mgr = crate::core::history::HistoryManager::new()?;
    let entries = history_mgr.load()?;

    let type_filter = transaction_type.map(|filter| match filter {
        TransactionTypeFilter::Install => crate::core::history::TransactionType::Install,
        TransactionTypeFilter::Remove => crate::core::history::TransactionType::Remove,
        TransactionTypeFilter::Update => crate::core::history::TransactionType::Update,
        TransactionTypeFilter::Sync => crate::core::history::TransactionType::Sync,
    });

    let parse_date = |value: &str| {
        jiff::civil::Date::strptime("%Y-%m-%d", value)
            .with_context(|| format!("Invalid date '{value}'; expected YYYY-MM-DD"))
    };
    let from_date = from.map(parse_date).transpose()?;
    let to_date = to.map(parse_date).transpose()?;
    let search_lower = search.map(str::to_lowercase);

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

            // Filter by search term (package name).
            if let Some(query_lower) = &search_lower {
                let matches = entry
                    .changes
                    .iter()
                    .any(|c| c.name.to_lowercase().contains(query_lower));
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

    if json {
        println!("{}", serde_json::to_string_pretty(&filtered)?);
        return Ok(());
    }

    let header = if search.is_some() || transaction_type.is_some() || from.is_some() || to.is_some()
    {
        "Transaction History (filtered)".to_string()
    } else {
        format!("Transaction History (last {limit})")
    };
    println!("{} {}\n", style::header("OMG"), header);

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
            style::info(short_id(&entry.id)),
            style::warning(&entry.transaction_type.to_string()),
            style::dim(&format!("({} changes)", entry.changes.len()))
        );

        // If searching, highlight matching packages.
        for change in &entry.changes {
            let pkg_display = if let Some(ref query_lower) = search_lower {
                if change.name.to_lowercase().contains(query_lower) {
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

#[derive(Debug, PartialEq, Eq)]
enum RollbackAction {
    Remove(Vec<String>),
    Restore(Vec<(String, String)>),
    /// Nothing to reverse (e.g. a database sync transaction).
    NothingToDo,
}

fn normalize_transaction_id(id: &str) -> Result<String> {
    let normalized = id.to_ascii_lowercase();
    let is_hex_prefix = (1..=32).contains(&normalized.len())
        && normalized
            .chars()
            .all(|character| character.is_ascii_hexdigit());
    let is_uuid = uuid::Uuid::parse_str(&normalized).is_ok();

    if !is_hex_prefix && !is_uuid {
        anyhow::bail!("Invalid transaction ID format");
    }

    Ok(normalized)
}

fn rollback_action(transaction: &crate::core::history::Transaction) -> Result<RollbackAction> {
    anyhow::ensure!(
        transaction.success,
        "Cannot automatically roll back a failed or partially applied transaction"
    );

    match transaction.transaction_type {
        crate::core::history::TransactionType::Install => Ok(RollbackAction::Remove(
            transaction
                .changes
                .iter()
                .map(|change| change.name.clone())
                .collect(),
        )),
        crate::core::history::TransactionType::Remove
        | crate::core::history::TransactionType::Update => {
            let mut packages = Vec::with_capacity(transaction.changes.len());
            for change in &transaction.changes {
                anyhow::ensure!(
                    change.is_official_source(),
                    "Automatic rollback is unavailable for package '{}' from source '{}'",
                    change.name,
                    change.source
                );
                let version = change.old_version.clone().with_context(|| {
                    format!(
                        "Transaction does not record the old version of '{}'",
                        change.name
                    )
                })?;
                packages.push((change.name.clone(), version));
            }
            Ok(RollbackAction::Restore(packages))
        }
        crate::core::history::TransactionType::Sync => Ok(RollbackAction::NothingToDo),
    }
}

#[cfg(any(feature = "arch", test))]
fn find_cached_arch_package_in(
    cache_dir: &std::path::Path,
    package: &str,
    version: &str,
) -> Result<std::path::PathBuf> {
    let prefix = format!("{package}-{version}-");
    let mut matches = Vec::new();
    for entry in std::fs::read_dir(cache_dir)
        .with_context(|| format!("Failed to read pacman cache: {}", cache_dir.display()))?
    {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let filename = entry.file_name();
        let filename = filename.to_string_lossy();
        let is_package = [".pkg.tar.zst", ".pkg.tar.xz", ".pkg.tar.gz", ".pkg.tar.bz2"]
            .iter()
            .any(|extension| filename.ends_with(extension));
        if is_package && filename.starts_with(&prefix) {
            matches.push(entry.path());
        }
    }
    matches.sort_unstable();
    matches.into_iter().next().with_context(|| {
        format!(
            "Package {package} {version} is not available in the pacman cache at {}",
            cache_dir.display()
        )
    })
}

#[cfg(feature = "arch")]
fn find_cached_arch_package(package: &str, version: &str) -> Result<std::path::PathBuf> {
    let cache_dirs = crate::core::paths::pacman_cache_dirs();
    for cache_dir in &cache_dirs {
        if !cache_dir.exists() {
            continue;
        }
        std::fs::read_dir(cache_dir)
            .with_context(|| format!("Failed to read pacman cache: {}", cache_dir.display()))?;
        if let Ok(path) = find_cached_arch_package_in(cache_dir, package, version) {
            return Ok(path);
        }
    }

    anyhow::bail!(
        "Package {package} {version} is not available in configured pacman caches: {}",
        cache_dirs
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[allow(
    clippy::unused_async,
    reason = "feature-gated implementations await while fallback builds do not"
)]
pub async fn rollback(id: Option<String>, yes: bool) -> Result<()> {
    let id = match id {
        Some(id) => Some(normalize_transaction_id(&id)?),
        None => None,
    };

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
                    short_id(&e.id),
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
        style::info(short_id(&target.id))
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

    match rollback_action(target)? {
        RollbackAction::NothingToDo => {
            println!("{}", style::success("Nothing to roll back"));
        }
        RollbackAction::Remove(packages) if packages.is_empty() => {
            println!("{}", style::success("Nothing to roll back"));
        }
        RollbackAction::Remove(packages) => {
            println!(
                "{} Removing {} package(s)...",
                style::info("→"),
                packages.len()
            );
            let package_manager = crate::package_managers::get_package_manager()?;
            let result = package_manager.remove(&packages).await;
            // Record the rollback itself so history reflects reality: the
            // packages were removed by this rollback, not by a user remove.
            let changes = packages
                .iter()
                .map(|name| crate::core::history::PackageChange {
                    name: name.clone(),
                    old_version: None,
                    new_version: None,
                    source: "rollback".to_string(),
                })
                .collect();
            crate::core::history::HistoryManager::new()?.finish_operation(
                crate::core::history::TransactionType::Remove,
                changes,
                result,
            )?;
            println!("{}", style::success("✓ Rollback completed successfully"));
        }
        RollbackAction::Restore(packages) if packages.is_empty() => {
            println!("{}", style::success("Nothing to roll back"));
        }
        RollbackAction::Restore(packages) => {
            #[cfg(any(feature = "debian", feature = "debian-pure"))]
            if crate::core::env::distro::is_debian_like() {
                #[cfg(feature = "debian")]
                {
                    let to_install: Vec<String> = packages
                        .iter()
                        .map(|(name, version)| format!("{name}={version}"))
                        .collect();
                    println!(
                        "{} Restoring {} package(s)...",
                        style::info("→"),
                        to_install.len()
                    );
                    let apt = crate::package_managers::AptPackageManager::new();
                    use crate::package_managers::PackageManager as _;
                    apt.install(&to_install).await?;
                    println!("{}", style::success("✓ Rollback completed successfully"));
                    return Ok(());
                }
                #[cfg(not(feature = "debian"))]
                {
                    let names = packages
                        .iter()
                        .map(|(name, _)| name.clone())
                        .collect::<Vec<_>>();
                    return rollback_requires_backend(&names);
                }
            }

            #[cfg(feature = "arch")]
            {
                let cached_packages = packages
                    .iter()
                    .map(|(name, version)| {
                        find_cached_arch_package(name, version)
                            .map(|path| path.to_string_lossy().into_owned())
                    })
                    .collect::<Result<Vec<_>>>()?;
                println!(
                    "{} Restoring {} package(s) from the pacman cache...",
                    style::info("→"),
                    cached_packages.len()
                );
                let pacman = crate::package_managers::ArchPackageManager::new();
                let result = pacman.install(&cached_packages).await;
                let changes = packages
                    .iter()
                    .map(|(name, version)| crate::core::history::PackageChange {
                        name: name.clone(),
                        old_version: None,
                        new_version: Some(version.clone()),
                        source: "rollback".to_string(),
                    })
                    .collect();
                crate::core::history::HistoryManager::new()?.finish_operation(
                    crate::core::history::TransactionType::Install,
                    changes,
                    result,
                )?;
                println!("{}", style::success("✓ Rollback completed successfully"));
            }

            #[cfg(not(feature = "arch"))]
            {
                let names = packages
                    .iter()
                    .map(|(name, _)| name.clone())
                    .collect::<Vec<_>>();
                return rollback_requires_backend(&names);
            }
        }
    }

    Ok(())
}

#[cfg(any(not(feature = "arch"), feature = "debian-pure", test))]
fn rollback_requires_backend(packages: &[String]) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("Package rollback is not available without the Arch or APT backend");
    }
    anyhow::bail!(
        "Package rollback is not available without the Arch or APT backend. Packages to restore: {}",
        packages.join(", ")
    )
}

/// Show usage statistics
pub fn stats(json: bool) -> Result<()> {
    use crate::cli::style;
    use crate::core::usage::UsageStats;

    // Value-estimate model: average software-engineer compensation spread over
    // a standard 2080-hour work year. Presentation-only estimate.
    const ASSUMED_ANNUAL_SALARY_USD: f64 = 150_000.0;
    const ASSUMED_WORK_HOURS_PER_YEAR: f64 = 2080.0;

    let stats = UsageStats::load().context("Failed to load usage statistics")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&stats)?);
        return Ok(());
    }

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
    let hourly_rate = ASSUMED_ANNUAL_SALARY_USD / ASSUMED_WORK_HOURS_PER_YEAR;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_completion_without_backend_is_an_error() {
        let error = package_completion_requires_backend()
            .expect_err("completion with no backend must not look like an empty catalog");
        assert!(
            error
                .to_string()
                .contains("not available without an Arch or Debian package backend"),
            "got: {error}"
        );
    }

    #[test]
    fn rollback_without_backend_is_an_error() {
        let error = rollback_requires_backend(&["bash=5.2".to_string()])
            .expect_err("rollback with no backend must not look like success");
        let message = error.to_string();
        assert!(
            message.contains("not available without the Arch or APT backend"),
            "got: {message}"
        );
        assert!(
            message.contains("bash=5.2"),
            "packages to restore must be in the error, got: {message}"
        );
    }

    fn transaction(
        transaction_type: crate::core::history::TransactionType,
        source: &str,
        old_version: Option<&str>,
        success: bool,
    ) -> crate::core::history::Transaction {
        crate::core::history::Transaction {
            id: "test".to_string(),
            timestamp: jiff::Timestamp::now(),
            transaction_type,
            changes: vec![crate::core::history::PackageChange {
                name: "example".to_string(),
                old_version: old_version.map(str::to_string),
                new_version: Some("2.0-1".to_string()),
                source: source.to_string(),
            }],
            success,
        }
    }

    #[test]
    fn rollback_plan_reverses_installs_and_restores_official_updates() -> Result<()> {
        assert_eq!(
            rollback_action(&transaction(
                crate::core::history::TransactionType::Install,
                "aur",
                None,
                true,
            ))?,
            RollbackAction::Remove(vec!["example".to_string()])
        );
        assert_eq!(
            rollback_action(&transaction(
                crate::core::history::TransactionType::Update,
                "core",
                Some("1.0-1"),
                true,
            ))?,
            RollbackAction::Restore(vec![("example".to_string(), "1.0-1".to_string())])
        );
        Ok(())
    }

    #[test]
    fn rollback_accepts_displayed_prefixes_and_persisted_uuids() -> Result<()> {
        assert_eq!(normalize_transaction_id("A1B2C3D4")?, "a1b2c3d4");
        assert_eq!(
            normalize_transaction_id("b69d428a-f73b-441c-8d8c-628550e063af")?,
            "b69d428a-f73b-441c-8d8c-628550e063af"
        );
        assert!(normalize_transaction_id("").is_err());
        assert!(normalize_transaction_id("../../history").is_err());
        assert!(normalize_transaction_id("b69d428a-f73b-441c-8d8c").is_err());
        Ok(())
    }

    #[test]
    fn rollback_plan_rejects_failed_and_non_restorable_transactions() {
        assert!(
            rollback_action(&transaction(
                crate::core::history::TransactionType::Update,
                "core",
                Some("1.0-1"),
                false,
            ))
            .is_err()
        );
        assert!(
            rollback_action(&transaction(
                crate::core::history::TransactionType::Update,
                "aur",
                Some("1.0-1"),
                true,
            ))
            .is_err()
        );
    }

    #[test]
    fn arch_rollback_requires_the_exact_cached_version() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let expected = directory.path().join("example-1.0-1-x86_64.pkg.tar.zst");
        std::fs::write(&expected, b"fixture")?;
        std::fs::write(
            directory.path().join("example-2.0-1-x86_64.pkg.tar.zst"),
            b"wrong version",
        )?;

        assert_eq!(
            find_cached_arch_package_in(directory.path(), "example", "1.0-1")?,
            expected
        );
        assert!(find_cached_arch_package_in(directory.path(), "example", "3.0-1").is_err());
        Ok(())
    }

    #[test]
    fn status_without_backend_is_an_error() {
        let error = status_requires_backend()
            .expect_err("status with no backend must not invent a healthy zero report");
        assert!(
            error
                .to_string()
                .contains("No supported package manager backend available"),
            "got: {error}"
        );
    }
}
