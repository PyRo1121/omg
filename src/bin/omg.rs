//! OMG CLI Binary
//!
//! The main command-line interface for OMG package manager.
//!
//! Uses a single tokio runtime for all async operations (Rust 2024 best practice).

// Use mimalloc as global allocator for 10-20% faster allocations
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use anyhow::Result;
use clap::Parser;

#[cfg(feature = "license")]
use omg_lib::cli::LicenseCommands;
use omg_lib::cli::doctor;
use omg_lib::cli::new;
use omg_lib::cli::packages;
use omg_lib::cli::runtimes;
use omg_lib::cli::security;

use omg_lib::cli::{
    CiCommands, Cli, Commands, ContainerCommands, LocalCommandRunner, MigrateCommands,
    SnapshotCommands, commands,
};
use omg_lib::cli::{blame, ci, diff, migrate, outdated, size, snapshot, why};
#[cfg(feature = "arch")]
use omg_lib::core::is_elevated;
use omg_lib::core::{is_root, set_yes_flag};
use omg_lib::hooks;

/// Print minimal success message for fast elevated path
#[cfg(feature = "arch")]
fn print_fast_success(packages: &[String], action: &str) {
    use owo_colors::OwoColorize;

    let msg = if packages.len() == 1 {
        format!("  ✓ 1 {action}!  ")
    } else {
        format!("  ✓ {} packages {action}!  ", packages.len())
    };

    println!();
    println!(
        "  {}",
        "╭─────────────────────────────────────────╮".green()
    );
    println!("  {} {} {}", "│".green(), msg.bold().green(), "│".green());
    println!(
        "  {}",
        "╰─────────────────────────────────────────╯".green()
    );

    if packages.len() <= 5 {
        println!();
        for pkg in packages {
            println!("    {} {}", "✓".green().bold(), pkg.bold());
        }
    }
    println!();
}

/// Print system update success message
#[cfg(feature = "arch")]
fn print_system_updated(suffix: &str) {
    use owo_colors::OwoColorize;
    println!();
    println!(
        "  {} System updated successfully{suffix}",
        "✓".green().bold()
    );
    println!();
}

#[cfg(feature = "arch")]
fn execute_fast_system_update(suffix: &str) -> Result<()> {
    use omg_lib::core::history::{HistoryManager, PackageChange, TransactionType};

    // Snapshot pending updates BEFORE upgrading so the history entry carries
    // real old→new versions. This elevated arm is the sole writer for the
    // official portion of delegated updates (`update --fast`, `--turbo`, and
    // the deferred-sync leg of plain `omg update`); without it those
    // upgrades were invisible to `omg history` / rollback.
    let changes: Vec<PackageChange> = omg_lib::package_managers::get_update_list()
        .unwrap_or_default()
        .into_iter()
        .map(|update| PackageChange {
            name: update.name,
            old_version: Some(update.old_version),
            new_version: Some(update.new_version),
            source: update.repo,
        })
        .collect();

    let result = omg_lib::package_managers::execute_transaction(Vec::new(), false, true, None);

    // Record regardless of outcome (failures are part of history) with the
    // TRUE transaction result — a failed upgrade must not be recorded as a
    // success. anyhow::Error is not Clone, so the history entry carries only
    // a failure marker; the real pacman error is reported verbatim by the
    // `finish(result)` call below and by `omg update`'s non-elevated arm.
    // https://docs.rs/anyhow/latest/anyhow/struct.Error.html (no Clone impl)
    let record_result = if result.is_ok() {
        Ok(())
    } else {
        Err(anyhow::anyhow!("system upgrade transaction failed"))
    };
    if let Err(error) = HistoryManager::new().and_then(|history| {
        history.finish_operation(TransactionType::Update, changes, record_result)
    }) {
        tracing::warn!("Failed to record update history: {error:#}");
    }

    if result.is_ok() {
        print_system_updated(suffix);
    }
    result
}

/// Split an elevated re-exec invocation into its sub-command and trailing
/// package tokens (`omg <command> ... -- <packages...>`).
///
/// Returns `None` when the invocation must fall through to full clap parsing:
/// missing sub-command or `--` separator, or any flag-looking token after the
/// separator. Flags after `--` (e.g. `update -- --check`) select behavior this
/// minimal transaction path cannot honor, so they are never executed here.
#[cfg(feature = "arch")]
fn split_elevated_invocation(args: &[String]) -> Option<(&str, &[String])> {
    let command = args.get(1)?;
    let separator_pos = args.iter().position(|a| a == "--")?;
    // The minimal transaction path honors exactly `omg <cmd> -- pkgs...`.
    // ANY flag-looking token anywhere in the elevated invocation (before or
    // after the separator) selects behavior this path cannot honor, so the
    // full CLI re-dispatch must handle it instead. Silently dropping e.g.
    // `--check` or `--dry-run` would turn a read-only request into a
    // destructive mutation.
    if args[2..separator_pos]
        .iter()
        .chain(&args[separator_pos + 1..])
        .any(|arg| arg.starts_with('-'))
    {
        return None;
    }
    let packages = &args[separator_pos + 1..];
    Some((command.as_str(), packages))
}

/// ULTRA-FAST elevated path - when we're re-exec'd with sudo, skip ALL initialization
/// and go straight to the transaction. This eliminates ~150ms of startup overhead.
#[cfg(feature = "arch")]
fn try_fast_elevated(args: &[String], reexec_elevated: bool) -> Option<Result<()>> {
    // Only run this path when elevated via sudo. The re-exec marker is the
    // authoritative signal (env_reset strips OMG_ELEVATED); accept the
    // legacy env flag too for direct `sudo omg` invocations.
    if !((reexec_elevated || is_elevated()) && omg_lib::core::privilege::is_root()) {
        return None;
    }

    // Privileged re-exec entrypoints: "upgrade", "fullupdate", and "turboupdate"
    // are NOT clap-defined user commands. They exist so internal privileged flows
    // can re-exec `sudo omg …` and land directly on the minimal transaction path
    // (see ALLOWED_ROOT_OPS in src/core/privilege.rs and the run_privileged_operation
    // labels in src/cli/packages/update/arch.rs). They still require the literal `--`
    // separator, and every token after it must be a package name; anything else
    // falls through to clap via split_elevated_invocation.
    let (command, package_tokens) = split_elevated_invocation(args)?;
    let mut packages: Vec<String> = package_tokens.to_vec();

    // Mid-flow delegations whose parent owns the history record carry this
    // trailing token; strip it before package validation and skip the child's
    // own recording so each mutation is written exactly once.
    let parent_records =
        packages.last().map(String::as_str) == Some(omg_lib::core::privilege::FLOW_PARENT_RECORDS);
    if parent_records {
        packages.pop();
        if packages.is_empty() {
            return None;
        }
    }

    // Handle commands that may have packages
    match command {
        "install" if !packages.is_empty() => {
            // Validate package names or local package files (security)
            omg_lib::core::security::validate_package_names_or_files(&packages).ok()?;
            // Direct transaction with minimal success output
            let result = omg_lib::package_managers::execute_transaction(
                packages.clone(),
                false,
                false,
                None,
            );
            let result = if parent_records {
                result
            } else {
                record_fast_transaction(
                    omg_lib::core::history::TransactionType::Install,
                    &packages,
                    result,
                )
            };
            if result.is_ok() {
                print_fast_success(&packages, "installed");
            }
            Some(result)
        }
        "remove" if !packages.is_empty() => {
            // Validate package names (security)
            omg_lib::core::security::validate_package_names(&packages).ok()?;
            let result =
                omg_lib::package_managers::execute_transaction(packages.clone(), true, false, None);
            let result = if parent_records {
                result
            } else {
                record_fast_transaction(
                    omg_lib::core::history::TransactionType::Remove,
                    &packages,
                    result,
                )
            };
            if result.is_ok() {
                print_fast_success(&packages, "removed");
            }
            Some(result)
        }
        "update" | "upgrade" => Some(execute_fast_system_update("")),
        "fullupdate" => {
            use owo_colors::OwoColorize;
            println!();
            println!("  {} Syncing package databases...", "→".cyan().bold());

            let sync_result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|rt| rt.block_on(omg_lib::package_managers::sync_databases_parallel()));

            if let Err(e) = sync_result {
                return Some(Err(e));
            }

            println!("  {} Upgrading system...", "→".cyan().bold());
            println!();

            Some(execute_fast_system_update(""))
        }
        "turboupdate" => {
            use owo_colors::OwoColorize;
            println!(
                "\n  {} Turbo upgrade (skipping sync)...\n",
                "🚀".bright_magenta().bold()
            );
            Some(execute_fast_system_update(" (turbo)"))
        }
        "sync" => {
            // Database sync - run in blocking context
            Some(
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(anyhow::Error::from)
                    .and_then(|rt| {
                        rt.block_on(omg_lib::package_managers::sync_databases_parallel())
                    }),
            )
        }
        _ => None,
    }
}

#[cfg(not(feature = "arch"))]
const fn try_fast_elevated(_args: &[String], _reexec_elevated: bool) -> Option<Result<()>> {
    None
}

#[cfg(feature = "arch")]
/// Record an elevated fast-path transaction in package history.
///
/// Elevated (`sudo omg ... --`) invocations previously mutated the system
/// without any history entry, leaving those packages invisible to
/// `omg history` / `omg rollback`.
fn record_fast_transaction(
    kind: omg_lib::core::history::TransactionType,
    packages: &[String],
    result: anyhow::Result<()>,
) -> anyhow::Result<()> {
    use omg_lib::core::history::{HistoryManager, PackageChange};

    let changes: Vec<PackageChange> = packages
        .iter()
        .map(|name| PackageChange {
            name: name.clone(),
            old_version: None,
            new_version: None,
            source: "pacman".to_string(),
        })
        .collect();
    HistoryManager::new()?.finish_operation(kind, changes, result)
}

/// Parse a `--limit` value accepted by the fast search path.
/// Returns `None` for non-numeric or zero values so the invocation defers to
/// clap, whose own validation owns the user-facing error.
fn parse_fast_limit(value: &str) -> Option<usize> {
    match value.parse::<usize>() {
        Ok(limit @ 1..) => Some(limit),
        _ => None,
    }
}

fn has_help_flag(args: &[String]) -> bool {
    args.iter().any(|a| matches!(a.as_str(), "--help" | "-h"))
}

fn has_all_flag(args: &[String]) -> bool {
    // `--all` is a subcommand-scoped flag (clean/list), not a global flag;
    // only the global `--all-commands` toggles full help output here.
    args.iter().any(|a| a == "--all-commands")
}

/// True when the global `--json` flag appears anywhere in the raw arguments.
/// Pre-parse fast paths that cannot render JSON must defer to clap when set.
fn has_json_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "--json")
}

/// Ultra-fast path for explicit --count (bypasses tokio entirely)
fn try_fast_explicit_count(args: &[String]) -> bool {
    if has_help_flag(args) || has_json_flag(args) {
        return false;
    }

    if args.len() >= 2
        && args[1] == "explicit"
        && args.iter().any(|a| matches!(a.as_str(), "--count" | "-c"))
    {
        if let Some(count) = omg_lib::core::fast_status::FastStatus::read_explicit_count() {
            println!("{count}");
            return true;
        }
        if packages::explicit_sync(true).is_ok() {
            return true;
        }
    }
    false
}

/// Ultra-fast path for simple search
///
/// Handles: `omg search <query>`, `omg s <query>`, `omg search <query> --no-aur`
/// Falls through to the async path for `--detailed` or `--json`.
fn try_fast_search(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    // Need at least: ["omg", "search", "<query>"]
    if args.len() < 3 {
        return false;
    }

    let cmd = &args[1];
    if cmd != "search" && cmd != "s" {
        return false;
    }

    // Find query (first non-flag arg after the command) and parse flags
    let mut query: Option<&str> = None;
    let mut no_aur = false;
    let mut limit: usize = 50;
    let mut i = 2usize;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--no-aur" => no_aur = true,
            "--limit" => {
                i += 1;
                if i >= args.len() {
                    return false;
                }
                let Some(parsed) = parse_fast_limit(&args[i]) else {
                    return false;
                };
                limit = parsed;
            }
            s if s.starts_with("--limit=") => {
                let Some(parsed) = parse_fast_limit(&s["--limit=".len()..]) else {
                    return false;
                };
                limit = parsed;
            }
            // Any other flag means this search needs the full async path
            s if s.starts_with('-') => return false,
            s => {
                if query.is_some() {
                    // Multiple positional args — not a simple search
                    return false;
                }
                query = Some(s);
            }
        }
        i += 1;
    }

    let Some(query) = query else {
        return false;
    };

    match packages::search_sync_cli_with_limit(query, false, no_aur, limit) {
        Ok(handled) => handled,
        Err(error) => {
            tracing::debug!("Fast search path failed, deferring to async path: {error}");
            false
        }
    }
}

/// Ultra-fast path for simple info
fn try_fast_info(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    if args.len() == 3 && args[1] == "info" {
        let package = &args[2];
        if package.starts_with('-') {
            return false;
        }
        if matches!(packages::info_sync(package), Ok(true)) {
            return true;
        }
    }
    false
}

fn try_fast_completions(args: &[String]) -> Result<bool> {
    if args.len() >= 3 && args[1] == "completions" {
        let shell = &args[2];
        if shell.starts_with('-') {
            return Ok(false);
        }

        if has_help_flag(args) {
            let use_all = has_all_flag(args);
            // Defer malformed invocations to clap's own error/help rendering.
            let Ok(cli) = Cli::try_parse_from(args.iter()) else {
                return Ok(false);
            };
            omg_lib::cli::help::print_help(&cli, use_all)?;
            return Ok(true);
        }

        let stdout = args.iter().any(|a| a == "--stdout");

        match shell.to_lowercase().as_str() {
            "bash" | "zsh" | "fish" => {
                hooks::completions::generate_completions(shell, stdout)?;
                return Ok(true);
            }
            _ => {
                return Ok(false);
            }
        }
    }
    Ok(false)
}

/// Ultra-fast path for which command
fn try_fast_which(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    if args.len() == 3 && args[1] == "which" {
        let runtime = &args[2];
        if runtime.starts_with('-') {
            return false;
        }

        // On failure, defer so the standard error reporter owns the message and
        // the process exits non-zero.
        return handle_which_command(runtime).is_ok();
    }
    false
}

/// Parsed form of a pre-parse `omg list|ls` invocation tail: optional runtime
/// plus whether the global `--json` flag was set. `None` means any token forces
/// the full clap path (which also owns unknown-flag error reporting).
fn parse_fast_list_tail(tail: &[String]) -> Option<(Option<&str>, bool)> {
    let mut runtime = None;
    let mut json = false;
    for arg in tail {
        match arg.as_str() {
            "--json" => json = true,
            s if s.starts_with('-') => return None,
            s => {
                if runtime.is_some() {
                    return None;
                }
                runtime = Some(s);
            }
        }
    }
    Some((runtime, json))
}

/// Ultra-fast path for list command
fn try_fast_list(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    if args.len() >= 2 && matches!(args[1].as_str(), "list" | "ls") {
        if args
            .iter()
            .any(|a| matches!(a.as_str(), "--available" | "-a"))
        {
            return false;
        }

        let Some((runtime, json)) = parse_fast_list_tail(&args[2..]) else {
            return false;
        };

        if runtimes::list_versions_sync(runtime, json).is_ok() {
            return true;
        }
    }
    false
}

/// Ultra-fast path for status command
fn try_fast_status(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    if args.len() == 2 && args[1] == "status" && commands::status_sync().is_ok() {
        return true;
    }
    false
}

/// Ultra-fast path for hook commands
fn try_fast_hooks(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    if args.len() >= 2 {
        match args[1].as_str() {
            "hook" => {
                if args.len() == 3 {
                    let shell = &args[2];
                    if hooks::print_hook(shell).is_ok() {
                        return true;
                    }
                }
            }
            "hook-env" => {
                // args[1] is the "hook-env" token itself, so scanning from
                // args[2] finds the same shell argument the old scan did —
                // without ever invoking hook_env("") (which always fails and
                // should simply defer to the full CLI path). get(2..) keeps
                // bare `omg hook-env` invocations panic-free.
                if let Some(shell) = args
                    .get(2..)
                    .into_iter()
                    .flatten()
                    .map(String::as_str)
                    .find(|shell| !shell.starts_with('-'))
                    && hooks::hook_env(shell).is_ok()
                {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn try_fast_paths(args: &[String]) -> Result<bool> {
    if try_fast_explicit_count(args)
        || try_fast_search(args)
        || try_fast_info(args)
        || try_fast_which(args)
        || try_fast_list(args)
        || try_fast_status(args)
        || try_fast_hooks(args)
    {
        return Ok(true);
    }

    try_fast_completions(args)
}

fn main() {
    let mut args: Vec<String> = std::env::args().collect();

    // Strip the sudo re-exec marker. Elevation travels via argv because
    // sudo's env_reset strips OMG_ELEVATED from the child environment (see
    // ELEVATED_MARKER in core::privilege). The marker is honored ONLY for a
    // root process: anyone else invoking the reserved token keeps their
    // arguments untouched and gets clap's unknown-command error.
    let reexec_elevated = args.get(1).map(String::as_str)
        == Some(omg_lib::core::privilege::ELEVATED_MARKER)
        && omg_lib::core::privilege::is_root();
    if reexec_elevated {
        args.remove(1);
    }

    // FASTEST PATH: Elevated re-exec - skip ALL initialization
    // This runs when sudo omg re-execs us as root
    if let Some(result) = try_fast_elevated(&args, reexec_elevated) {
        finish(result);
    }

    match try_fast_paths(&args) {
        Ok(true) => finish(Ok(())),
        Ok(false) => {}
        Err(error) => finish(Err(error)),
    }

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build();
    let result = match runtime {
        Ok(runtime) => runtime.block_on(async_main(args)),
        Err(error) => Err(anyhow::Error::new(error).context("Failed to initialize async runtime")),
    };

    finish(result);
}

/// Single owner of process-level error reporting.
///
/// Prints the error chain exactly once, followed by actionable guidance on
/// stderr (user-facing content, not a trace diagnostic), and selects the
/// process exit code. Returning `!` replaces the old `Result`-returning main,
/// whose anyhow `Termination` impl printed errors but could never show
/// suggestions or differentiate exit codes.
fn finish(result: Result<()>) -> ! {
    match result {
        Ok(()) => std::process::exit(0),
        Err(error) => {
            eprintln!("Error: {error:?}");
            if let Some(suggestion) = omg_lib::core::error::suggest_for_anyhow(&error) {
                eprintln!("\nSuggestion: {suggestion}");
            }
            std::process::exit(1);
        }
    }
}

async fn async_main(args: Vec<String>) -> Result<()> {
    // Record startup time for telemetry
    omg_lib::core::telemetry::record_startup_time();

    let cmd_start = std::time::Instant::now();
    let cli = Cli::parse_from(&args);

    // Initialize session tracking (will create new session if expired)
    omg_lib::core::telemetry::track_session_start();

    // Set the yes flag globally based on command
    let yes_flag = matches!(
        &cli.command,
        Commands::Install { yes: true, .. }
            | Commands::Remove { yes: true, .. }
            | Commands::Update { yes: true, .. }
            | Commands::Rollback { yes: true, .. }
            | Commands::Snapshot {
                command: SnapshotCommands::Restore { yes: true, .. }
            }
    );
    set_yes_flag(yes_flag);

    // SECURITY: Validate package names
    validate_package_security(&cli.command)?;

    // Initialize logging
    init_logging(cli.verbose, cli.quiet);

    let telemetry_ping = spawn_telemetry_ping();

    if matches!(
        &cli.command,
        Commands::Install { .. }
            | Commands::Remove { .. }
            | Commands::Update { .. }
            | Commands::Sync
            | Commands::Clean { .. }
    ) {
        omg_lib::core::maybe_show_turbo_hint();
    }

    if command_requires_root(&cli.command) && !is_root() {
        // Use run_self_sudo directly — elevate_if_needed creates a nested tokio
        // runtime which panics with "Cannot start a runtime from within a runtime"
        let args_refs: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
        omg_lib::core::privilege::run_self_sudo(&args_refs).await?;
        std::process::exit(0);
    }

    let ctx = omg_lib::cli::CliContext {
        verbose: cli.verbose,
        json: cli.json,
        quiet: cli.quiet,
        no_color: !console::colors_enabled(),
    };

    let result = dispatch_command(&cli.command, &ctx).await;
    finish_command_telemetry(cmd_start, command_name(&cli.command), result.is_ok()).await;
    if let Some(task) = telemetry_ping {
        match task.await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => tracing::debug!("Install telemetry ping failed: {error}"),
            Err(error) => tracing::debug!("Install telemetry task failed: {error}"),
        }
    }
    result
}

/// Validate package names for security
fn command_requires_root(command: &Commands) -> bool {
    match command {
        Commands::Sync => true,
        Commands::Clean {
            orphans,
            cache,
            all,
            dry_run,
            ..
        } => !dry_run && (*orphans || *cache || *all),
        _ => false,
    }
}

fn validate_package_security(command: &Commands) -> Result<()> {
    match command {
        Commands::Install { packages, .. } => {
            // Install can accept package names OR local .pkg.tar.* files
            omg_lib::core::security::validate_package_names_or_files(packages)?;
        }
        Commands::Remove { packages, .. } => {
            omg_lib::core::security::validate_package_names(packages)?;
        }
        Commands::Info { package }
        | Commands::Why { package, .. }
        | Commands::Blame { package } => {
            omg_lib::core::security::validate_package_name(package)?;
        }
        _ => {}
    }
    Ok(())
}

/// Initialize tracing/logging subsystem
fn init_logging(verbose: u8, quiet: bool) {
    let env_filter = if std::env::var("RUST_LOG").is_ok() {
        // RUST_LOG owns filtering verbatim; do not override the user's choice.
        tracing_subscriber::EnvFilter::from_default_env()
    } else if quiet {
        // Quiet mode: errors only.
        tracing_subscriber::EnvFilter::new("error")
    } else {
        // Map -v counts to levels (0=WARN, 1=INFO, 2=DEBUG, 3+=TRACE).
        let level = match verbose {
            0 => tracing::Level::WARN,
            1 => tracing::Level::INFO,
            2 => tracing::Level::DEBUG,
            _ => tracing::Level::TRACE,
        };
        tracing_subscriber::EnvFilter::new(level.to_string())
    };

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_ansi(console::colors_enabled())
        .without_time()
        .init();
}

/// Spawn telemetry ping on first run
fn spawn_telemetry_ping() -> Option<tokio::task::JoinHandle<Result<()>>> {
    (omg_lib::core::telemetry::is_first_run() && !omg_lib::core::telemetry::is_telemetry_opt_out())
        .then(|| tokio::spawn(omg_lib::core::telemetry::ping_install()))
}

/// Track command analytics and flush
/// Canonical command name for analytics, independent of user-facing aliases
/// (`s`, `i`, `u`, `sy`, …) and of argument order.
const fn command_name(command: &Commands) -> &'static str {
    match command {
        Commands::Search { .. } => "search",
        Commands::Install { .. } => "install",
        Commands::Remove { .. } => "remove",
        Commands::Update { .. } => "update",
        Commands::Info { .. } => "info",
        Commands::Why { .. } => "why",
        Commands::Outdated => "outdated",
        Commands::Size { .. } => "size",
        Commands::Blame { .. } => "blame",
        Commands::Diff { .. } => "diff",
        Commands::Snapshot { .. } => "snapshot",
        Commands::Ci { .. } => "ci",
        Commands::Migrate { .. } => "migrate",
        Commands::Clean { .. } => "clean",
        Commands::Explicit { .. } => "explicit",
        Commands::Sync => "sync",
        Commands::Use { .. } => "use",
        Commands::List { .. } => "list",
        Commands::Hook { .. } => "hook",
        Commands::Hooks { .. } => "hooks",
        Commands::Workspace { .. } => "workspace",
        Commands::HookEnv { .. } => "hook-env",
        #[cfg(unix)]
        Commands::Daemon { .. } => "daemon",
        Commands::Config { .. } => "config",
        Commands::Privacy { .. } => "privacy",
        Commands::SelfUpdate { .. } => "self-update",
        Commands::Completions { .. } => "completions",
        Commands::Which { .. } => "which",
        Commands::Complete { .. } => "complete",
        Commands::Status { .. } => "status",
        Commands::Doctor { .. } => "doctor",
        Commands::GenerateMan { .. } => "generate-man",
        #[cfg(unix)]
        Commands::DaemonStatus => "daemon-status",
        Commands::Audit { .. } => "audit",
        Commands::New { .. } => "new",
        Commands::Run { .. } => "run",
        Commands::Tool { .. } => "tool",
        Commands::Env { .. } => "env",
        Commands::Team { .. } => "team",
        Commands::Container { .. } => "container",
        #[cfg(feature = "license")]
        Commands::License { .. } => "license",
        Commands::Fleet { .. } => "fleet",
        Commands::Enterprise { .. } => "enterprise",
        Commands::History { .. } => "history",
        Commands::Rollback { .. } => "rollback",
        Commands::Dash => "dash",
        Commands::Stats => "stats",
        #[cfg(unix)]
        Commands::Metrics => "metrics",
        Commands::Init { .. } => "init",
    }
}

async fn finish_command_telemetry(
    command_started: std::time::Instant,
    command_name: &str,
    success: bool,
) {
    let duration_ms = command_started.elapsed().as_millis() as u64;
    let backend = omg_lib::core::telemetry::get_backend();
    omg_lib::core::telemetry::track_command_event(
        command_name,
        duration_ms,
        success,
        Some(&backend),
    );
    omg_lib::core::telemetry::end_session_and_flush().await;
    omg_lib::core::usage::sync_usage_now().await;
}

fn handle_hooks_command(command: &omg_lib::cli::HooksCommands) -> Result<()> {
    use omg_lib::cli::HooksCommands;
    match command {
        HooksCommands::Install { force } => omg_lib::cli::git_hooks::install(*force),
        HooksCommands::Uninstall => omg_lib::cli::git_hooks::uninstall(),
        HooksCommands::Status => omg_lib::cli::git_hooks::status(),
        HooksCommands::Run { hook } => omg_lib::cli::git_hooks::run_hook(hook),
    }
}

async fn handle_workspace_command(command: &omg_lib::cli::WorkspaceCommands) -> Result<()> {
    use omg_lib::cli::WorkspaceCommands;
    match command {
        WorkspaceCommands::Init { name } => omg_lib::cli::workspace::init(name),
        WorkspaceCommands::Add { path, name } => {
            omg_lib::cli::workspace::add(path, name.as_deref())
        }
        WorkspaceCommands::Remove { project } => omg_lib::cli::workspace::remove(project),
        WorkspaceCommands::List => omg_lib::cli::workspace::list(),
        WorkspaceCommands::Run {
            command: cmd,
            args,
            parallel,
            filter,
        } => omg_lib::cli::workspace::run(cmd, args, *parallel, filter.as_deref()).await,
        WorkspaceCommands::Diff { branch } => omg_lib::cli::workspace::diff(branch),
        WorkspaceCommands::Sync { yes } => omg_lib::cli::workspace::sync(*yes),
        WorkspaceCommands::Status => omg_lib::cli::workspace::status(),
    }
}

fn handle_config_command(command: Option<&omg_lib::cli::ConfigCommands>) -> Result<()> {
    use omg_lib::cli::ConfigCommands;
    match command {
        Some(ConfigCommands::Get { key }) => omg_lib::cli::config::get(key),
        Some(ConfigCommands::Set { key, value }) => omg_lib::cli::config::set(key, value),
        Some(ConfigCommands::List) | None => omg_lib::cli::config::list(),
        Some(ConfigCommands::Validate) => omg_lib::cli::config::validate(),
        Some(ConfigCommands::Reset { yes }) => omg_lib::cli::config::reset(*yes),
        Some(ConfigCommands::Path) => omg_lib::cli::config::path(),
    }
}

async fn handle_privacy_command(command: Option<&omg_lib::cli::PrivacyCommands>) -> Result<()> {
    use omg_lib::cli::PrivacyCommands;
    use omg_lib::cli::telemetry;

    match command {
        Some(PrivacyCommands::Status) | None => telemetry::privacy_status().await,
        Some(PrivacyCommands::Export { output }) => telemetry::export_data(output.as_deref()).await,
        Some(PrivacyCommands::Delete { confirm }) => telemetry::delete_data(*confirm).await,
        Some(PrivacyCommands::OptOut) => telemetry::opt_out_api().await,
        Some(PrivacyCommands::OptIn) => telemetry::opt_in_api().await,
    }
}

fn handle_container_command(command: &ContainerCommands) -> Result<()> {
    use omg_lib::cli::container;
    match command {
        ContainerCommands::Status => container::status(),
        ContainerCommands::Run {
            image,
            command: cmd,
            name,
            detach,
            interactive,
            env,
            volume,
            workdir,
        } => container::run(
            image,
            cmd,
            name.clone(),
            *detach,
            *interactive,
            env,
            volume,
            workdir.clone(),
        ),
        ContainerCommands::Shell {
            image,
            workdir,
            env,
            volume,
        } => container::shell(image.clone(), workdir.clone(), env, volume),
        ContainerCommands::Build {
            dockerfile,
            tag,
            no_cache,
            build_arg,
            target,
        } => container::build(dockerfile.clone(), tag, *no_cache, build_arg, target),
        ContainerCommands::List => container::list(),
        ContainerCommands::Images => container::images(),
        ContainerCommands::Pull { image } => container::pull(image),
        ContainerCommands::Stop { container: c } => container::stop(c),
        ContainerCommands::Exec {
            container: c,
            command: cmd,
        } => container::exec(c, cmd),
        ContainerCommands::Init { base } => container::init(base.clone()),
    }
}

#[cfg(feature = "license")]
async fn handle_license_command(command: &LicenseCommands) -> Result<()> {
    use omg_lib::cli::license;
    match command {
        LicenseCommands::Activate { key } => license::activate(key).await,
        LicenseCommands::Status => license::status(),
        LicenseCommands::Deactivate => license::deactivate(),
        LicenseCommands::Check { feature } => license::check_feature(feature),
    }
}

#[expect(clippy::fn_params_excessive_bools)] // Maps directly to CLI flags: --check, --yes, --dry-run, --fast, --turbo
async fn handle_update_command(
    check: bool,
    yes: bool,
    dry_run: bool,
    fast: bool,
    turbo: bool,
) -> Result<()> {
    // Fast/turbo re-exec into non-interactive privileged flows that cannot
    // preview or skip; honoring --check/--dry-run there would be a lie, so
    // reject the combination instead of silently ignoring it (wave-5 F4).
    // (--yes is accepted: fast/turbo never prompt, so it is already implied.)
    if (fast || turbo) && (check || dry_run) {
        anyhow::bail!(
            "--{} cannot be combined with --fast/--turbo: fast and turbo updates run non-interactively without preview",
            if check { "check" } else { "dry-run" }
        );
    }
    if turbo {
        packages::update_turbo().await
    } else if fast {
        packages::update_fast().await
    } else {
        packages::update(check, yes, dry_run).await
    }
}

async fn handle_init_command(defaults: bool, skip_shell: bool, skip_daemon: bool) -> Result<()> {
    if defaults {
        omg_lib::cli::init::run_defaults().await
    } else {
        omg_lib::cli::init::run_interactive(skip_shell, skip_daemon).await
    }
}

async fn handle_doctor_command(network: bool, eol: bool, turbo: bool) -> Result<()> {
    if turbo {
        doctor::enable_turbo_mode()
    } else {
        doctor::run(network, eol).await
    }
}

fn handle_which_command(runtime: &str) -> Result<()> {
    match runtimes::resolve_active_version(runtime) {
        Ok(Some(version)) => {
            println!(
                "{} {}",
                omg_lib::cli::style::runtime(runtime),
                omg_lib::cli::style::version(&version)
            );
        }
        Ok(None) => {
            println!(
                "{}: no version set (check .tool-versions, .nvmrc, etc.)",
                omg_lib::cli::style::runtime(runtime)
            );
        }
        Err(error) => {
            // Propagate instead of only printing so the process exits non-zero.
            anyhow::bail!(
                "failed to resolve active version for {runtime}: {error}; check that a version is set (.tool-versions, .nvmrc) or run `omg use {runtime} <version>`"
            );
        }
    }
    Ok(())
}

async fn handle_audit_command(
    command: Option<&omg_lib::cli::AuditCommands>,
    ctx: &omg_lib::cli::CliContext,
) -> Result<()> {
    if let Some(cmd) = command {
        cmd.execute(ctx).await
    } else {
        security::scan(ctx).await
    }
}

async fn handle_snapshot_command(command: &SnapshotCommands) -> Result<()> {
    match command {
        SnapshotCommands::Create { message } => snapshot::create(message.clone()).await,
        SnapshotCommands::List => snapshot::list(),
        SnapshotCommands::Restore { id, dry_run, yes } => {
            snapshot::restore(id, *dry_run, *yes).await
        }
        SnapshotCommands::Delete { id } => snapshot::delete(id),
    }
}

async fn handle_ci_command(command: &CiCommands) -> Result<()> {
    match command {
        CiCommands::Init { provider, advanced } => ci::init(provider.as_str(), *advanced),
        CiCommands::Validate => ci::validate().await,
        CiCommands::Cache => ci::cache(),
    }
}

async fn handle_migrate_command(command: &MigrateCommands) -> Result<()> {
    match command {
        MigrateCommands::Export { output } => migrate::export(output).await,
        MigrateCommands::Import { manifest, dry_run } => migrate::import(manifest, *dry_run).await,
    }
}

/// Main command dispatcher - routes commands to appropriate handlers
#[expect(clippy::too_many_lines)]
async fn dispatch_command(command: &Commands, ctx: &omg_lib::cli::CliContext) -> Result<()> {
    // Global --json contract: reject unsupported combinations explicitly instead
    // of silently emitting human-readable output (wave-5 F3). `privacy` is
    // accepted because `privacy status` is the scripted JSON entrypoint.
    if ctx.json
        && !matches!(
            command,
            Commands::Search { .. }
                | Commands::Info { .. }
                | Commands::Explicit { .. }
                | Commands::List { .. }
                | Commands::Status { .. }
                | Commands::History { .. }
                | Commands::Stats
                | Commands::Outdated
                | Commands::Privacy { .. }
        )
    {
        anyhow::bail!(
            "--json is not supported for `{}`; supported: search, info, explicit, list, status, history, stats, outdated, privacy status",
            command_name(command)
        );
    }

    match command {
        // Single dispatcher: each group runner is invoked directly here; there is
        // deliberately no blanket `impl LocalCommandRunner for Commands` fallback.
        Commands::Run {
            task,
            args,
            runtime_backend,
            watch,
            parallel,
            using,
            all,
        } => {
            let run_cmd = omg_lib::cli::run::RunCommand {
                task: task.clone(),
                args: args.clone(),
                runtime_backend: runtime_backend.map(Into::into),
                watch: *watch,
                parallel: *parallel,
                using: using.clone(),
                all: *all,
            };
            run_cmd.execute(ctx).await?;
        }
        Commands::Tool { command } => command.execute(ctx).await?,
        Commands::Env { command } => command.execute(ctx).await?,
        Commands::Fleet { command } => command.execute(ctx).await?,
        Commands::Team { command } => command.execute(ctx).await?,
        Commands::Enterprise { command } => command.execute(ctx).await?,
        Commands::Search {
            query,
            detailed,
            no_aur,
            limit,
        } => {
            packages::search_with_json(query, *detailed, ctx.json, *no_aur, *limit).await?;
        }
        Commands::Install {
            packages: pkgs,
            yes,
            dry_run,
        } => {
            packages::install(pkgs, *yes, *dry_run).await?;
        }
        Commands::Remove {
            packages: pkgs,
            recursive,
            yes,
            dry_run,
        } => {
            packages::remove(pkgs, *recursive, *yes, *dry_run).await?;
        }
        Commands::Update {
            check,
            yes,
            dry_run,
            fast,
            turbo,
        } => {
            handle_update_command(*check, *yes, *dry_run, *fast, *turbo).await?;
        }
        Commands::Info { package } => packages::info_with_json(package, ctx.json).await?,
        Commands::Clean {
            orphans,
            cache,
            aur,
            all,
            dry_run,
        } => {
            packages::clean(*orphans, *cache, *aur, *all, *dry_run).await?;
        }
        Commands::Explicit { count } => {
            packages::explicit_sync_with_json(*count, ctx.json)?;
        }
        Commands::Sync => {
            packages::sync().await?;
        }
        Commands::Use { runtime, version } => {
            runtimes::use_version(runtime, version.as_deref()).await?;
        }
        Commands::List { runtime, available } => {
            runtimes::list_versions(runtime.as_deref(), *available, ctx.json).await?;
        }
        Commands::Hook { shell } => {
            hooks::print_hook(shell.as_str())?;
        }
        Commands::Hooks { command } => handle_hooks_command(command)?,
        Commands::Workspace { command } => handle_workspace_command(command).await?,
        Commands::HookEnv { shell } => {
            hooks::hook_env(shell.as_str())?;
        }
        #[cfg(unix)]
        Commands::Daemon { foreground } => {
            commands::daemon(*foreground)?;
        }
        Commands::Config { command } => handle_config_command(command.as_ref())?,
        Commands::Privacy { command } => handle_privacy_command(command.as_ref()).await?,
        Commands::SelfUpdate { force, version } => {
            omg_lib::cli::self_update::run(*force, version.clone()).await?;
        }
        Commands::Completions { shell, stdout } => {
            hooks::completions::generate_completions(shell.as_str(), *stdout)?;
        }
        Commands::Which { runtime } => handle_which_command(runtime)?,
        Commands::Complete {
            shell,
            current,
            last,
            full,
        } => {
            commands::complete(shell.as_str(), current, last, full.as_deref()).await?;
        }
        Commands::Status { fast } => {
            packages::status_with_json(*fast, ctx.json).await?;
        }
        Commands::Doctor {
            network,
            eol,
            turbo,
        } => {
            handle_doctor_command(*network, *eol, *turbo).await?;
        }
        Commands::GenerateMan { output } => {
            omg_lib::cli::man::generate(output.clone())?;
        }
        #[cfg(unix)]
        Commands::DaemonStatus => {
            omg_lib::cli::daemon_status::run().await?;
        }
        Commands::Audit { command } => handle_audit_command(command.as_ref(), ctx).await?,
        Commands::New { stack, name } => {
            new::run(stack.as_str(), name)?;
        }
        Commands::Container { command } => handle_container_command(command)?,
        #[cfg(feature = "license")]
        Commands::License { command } => handle_license_command(command).await?,
        Commands::History {
            limit,
            search,
            transaction_type,
            from,
            to,
        } => {
            commands::history(
                *limit,
                search.as_deref(),
                *transaction_type,
                from.as_deref(),
                to.as_deref(),
                ctx.json,
            )?;
        }
        Commands::Rollback { id, yes } => {
            commands::rollback(id.clone(), *yes).await?;
        }
        Commands::Dash => {
            omg_lib::cli::tui::run().await?;
        }
        Commands::Stats => {
            commands::stats(ctx.json)?;
        }
        #[cfg(unix)]
        Commands::Metrics => {
            commands::metrics().await?;
        }
        Commands::Init {
            defaults,
            skip_shell,
            skip_daemon,
        } => {
            handle_init_command(*defaults, *skip_shell, *skip_daemon).await?;
        }
        Commands::Why { package, reverse } => {
            why::run(package, *reverse)?;
        }
        Commands::Outdated => {
            outdated::run(ctx.json).await?;
        }
        Commands::Size { tree, limit } => {
            size::run(tree.as_deref(), *limit)?;
        }
        Commands::Blame { package } => {
            blame::run(package)?;
        }
        Commands::Diff { from, to } => {
            diff::run(from.as_deref(), to).await?;
        }
        Commands::Snapshot { command } => handle_snapshot_command(command).await?,
        Commands::Ci { command } => handle_ci_command(command).await?,
        Commands::Migrate { command } => handle_migrate_command(command).await?,
    }

    Ok(())
}

#[cfg(test)]
mod fast_path_tests {
    use super::{has_all_flag, has_json_flag, parse_fast_list_tail};

    #[cfg(feature = "arch")]
    use super::split_elevated_invocation;

    #[cfg(feature = "arch")]
    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(std::string::ToString::to_string).collect()
    }

    #[cfg(feature = "arch")]
    #[test]
    fn elevated_flags_after_separator_fall_through_to_clap() {
        // Regression for the blocker: `update -- --check` previously discarded
        // the flag tokens and force-ran a full system upgrade.
        assert!(
            split_elevated_invocation(&args(&["omg", "update", "--", "--check"])).is_none(),
            "check-only update must not take the transaction fast path"
        );
        assert!(
            split_elevated_invocation(&args(&["omg", "update", "--", "--dry-run"])).is_none(),
            "dry-run must not perform a real upgrade"
        );
        assert!(
            split_elevated_invocation(&args(&["omg", "install", "--", "-y", "ripgrep"])).is_none(),
            "flag-looking package tokens must defer to clap"
        );
    }

    #[cfg(feature = "arch")]
    #[test]
    fn elevated_plain_package_lists_still_split() {
        let argv = args(&["omg", "install", "extra", "--", "ripgrep", "jq"]);
        let parsed = split_elevated_invocation(&argv)
            .map(|(command, packages)| (command, packages.to_vec()));
        assert_eq!(
            parsed,
            Some(("install", vec!["ripgrep".to_string(), "jq".to_string()]))
        );
    }

    #[cfg(feature = "arch")]
    #[test]
    fn elevated_invocations_without_separator_or_command_are_rejected() {
        assert!(split_elevated_invocation(&args(&["omg"])).is_none());
        assert!(split_elevated_invocation(&args(&["omg", "update"])).is_none());
        // `update --` with no trailing tokens stays on the fast path (empty
        // package list), matching the original elevated behavior.
        let argv = args(&["omg", "update", "--"]);
        let parsed =
            split_elevated_invocation(&argv).map(|(command, pkgs)| (command, pkgs.to_vec()));
        assert_eq!(parsed, Some(("update", Vec::new())));
    }

    #[test]
    fn list_tail_parses_runtime_and_json_flag() {
        let args = args_or_panic(&["node"]);
        assert_eq!(parse_fast_list_tail(&args), Some((Some("node"), false)));

        let args = args_or_panic(&[]);
        assert_eq!(parse_fast_list_tail(&args), Some((None, false)));
    }

    #[test]
    fn list_json_flag_is_detected_instead_of_breaking_the_fast_path() {
        // Regression: `omg list --json` previously rendered human output.
        let args = args_or_panic(&["--json"]);
        assert_eq!(parse_fast_list_tail(&args), Some((None, true)));

        let args = args_or_panic(&["python", "--json"]);
        assert_eq!(parse_fast_list_tail(&args), Some((Some("python"), true)));
    }

    #[test]
    fn list_unknown_flags_and_extra_positionals_defer_to_clap() {
        assert_eq!(parse_fast_list_tail(&args_or_panic(&["--jsonx"])), None);
        assert_eq!(
            parse_fast_list_tail(&args_or_panic(&["node", "python"])),
            None
        );
    }

    #[test]
    fn json_flag_detector_matches_global_long_flag_only() {
        assert!(has_json_flag(&args_or_panic(&[
            "explicit", "--count", "--json"
        ])));
        assert!(!has_json_flag(&args_or_panic(&["explicit", "--count"])));
    }

    #[test]
    fn all_commands_flag_is_the_only_global_all_toggle() {
        // Regression: `--all` is a subcommand-scoped flag, not a global one.
        assert!(has_all_flag(&args_or_panic(&[
            "completions",
            "bash",
            "--all-commands"
        ])));
        assert!(!has_all_flag(&args_or_panic(&[
            "completions",
            "bash",
            "--all"
        ])));
    }

    fn args_or_panic(list: &[&str]) -> Vec<String> {
        list.iter().map(std::string::ToString::to_string).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_clean_never_requires_root() {
        let command = Commands::Clean {
            orphans: true,
            cache: true,
            aur: true,
            all: true,
            dry_run: true,
        };
        assert!(!command_requires_root(&command));
    }

    #[test]
    fn only_privileged_clean_actions_require_root() {
        let aur_only = Commands::Clean {
            orphans: false,
            cache: false,
            aur: true,
            all: false,
            dry_run: false,
        };
        let cache = Commands::Clean {
            orphans: false,
            cache: true,
            aur: false,
            all: false,
            dry_run: false,
        };
        assert!(!command_requires_root(&aur_only));
        assert!(command_requires_root(&cache));
        assert!(command_requires_root(&Commands::Sync));
    }
}
