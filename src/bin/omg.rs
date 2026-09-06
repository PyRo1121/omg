//! OMG CLI Binary
//!
//! The main command-line interface for OMG package manager.
//!
//! Uses a single tokio runtime for all async operations (Rust 2024 best practice).

// Use mimalloc as global allocator for 10-20% faster allocations
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

use anyhow::{Context, Result};
use clap::Parser;

#[cfg(feature = "license")]
use omg_lib::cli::AccountCommands;
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
    let msg = if packages.len() == 1 {
        format!("1 package {action}")
    } else {
        format!("{} packages {action}", packages.len())
    };
    omg_lib::cli::modern_ui::print_success_with_packages(&msg, packages);
}

#[cfg(feature = "arch")]
fn execute_fast_system_update(suffix: &str) -> Result<()> {
    use omg_lib::core::history::HistoryManager;

    let history = HistoryManager::new()?;
    let updates = omg_lib::package_managers::get_update_list()?;
    execute_recorded_system_update(&history, updates, || {
        omg_lib::package_managers::execute_transaction(
            Vec::new(),
            omg_lib::package_managers::TransactionKind::SystemUpgrade,
            None,
        )
    })?;
    println!(
        "\n  {} System updated successfully{suffix}\n",
        omg_lib::cli::style::positive("✓")
    );
    Ok(())
}

#[cfg(feature = "arch")]
fn execute_recorded_system_update(
    history: &omg_lib::core::history::HistoryManager,
    updates: Vec<omg_lib::package_managers::types::UpdateInfo>,
    operation: impl FnOnce() -> Result<()>,
) -> Result<()> {
    use omg_lib::core::history::{PackageChange, TransactionType};

    let changes: Vec<PackageChange> = updates
        .into_iter()
        .map(|update| PackageChange {
            name: update.name,
            old_version: Some(update.old_version),
            new_version: Some(update.new_version),
            source: update.repo,
        })
        .collect();

    history.finish_operation(TransactionType::Update, changes, operation())
}

/// Split an elevated re-exec invocation into its sub-command and trailing
/// package tokens (`omg <command> ... -- <packages...>`).
///
/// Returns `None` when the invocation must fall through to full clap parsing:
/// missing sub-command or `--` separator, any token between the command and
/// separator, or any flag-looking package token. The minimal path accepts only
/// the internal protocol shape `omg <command> -- <packages...>`. Install and
/// remove without a history-owning parent use the normal CLI history owner.
#[cfg(feature = "arch")]
fn split_elevated_invocation(args: &[String], parent_records: bool) -> Option<(&str, &[String])> {
    let command = args.get(1)?;
    if matches!(command.as_str(), "install" | "remove") && !parent_records {
        return None;
    }
    let separator_pos = args.iter().position(|a| a == "--")?;
    if separator_pos != 2 {
        return None;
    }
    // Flag-looking package tokens select behavior this path cannot honor, so
    // the full CLI re-dispatch must handle them instead. Silently dropping
    // `--check` or `--dry-run` would turn a read-only request into a
    // destructive mutation.
    if args[separator_pos + 1..]
        .iter()
        .any(|arg| arg.starts_with('-'))
    {
        return None;
    }
    let packages = &args[separator_pos + 1..];
    Some((command.as_str(), packages))
}

#[cfg(feature = "arch")]
/// Derive the rendering policy for the elevated fast path, which bypasses
/// clap, so transaction lanes see the same quiet/verbose contract as normal
/// dispatch. Global flags are scanned directly instead of running full
/// argument parsing to keep the fast path fast; `--` ends flag scanning.
fn configure_fast_path_output(args: &[String]) {
    let mut verbose = 0u8;
    let mut quiet = false;
    for token in args.iter().skip(1) {
        if token == "--" {
            break;
        }
        if let Some(short_flags) = token.strip_prefix('-').filter(|t| !t.starts_with('-')) {
            for flag in short_flags.chars() {
                match flag {
                    'v' => verbose = verbose.saturating_add(1),
                    'q' => quiet = true,
                    _ => {}
                }
            }
        } else {
            match token.as_str() {
                "--verbose" => verbose = verbose.saturating_add(1),
                "--quiet" => quiet = true,
                _ => {}
            }
        }
    }
    omg_lib::cli::modern_ui::configure_output(verbose, quiet);
}

#[cfg(feature = "arch")]
fn try_fast_elevated(
    args: &[String],
    reexec_elevated: bool,
    parent_records: bool,
) -> Option<Result<()>> {
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
    let (command, package_tokens) = split_elevated_invocation(args, parent_records)?;
    let packages: Vec<String> = package_tokens.to_vec();

    // Handle commands that may have packages
    match command {
        "install" if !packages.is_empty() => {
            // Validate package names or local package files (security).
            omg_lib::core::security::validate_package_names_or_files(&packages).ok()?;
            // Direct transaction with minimal success output
            let is_local_artifact = packages
                .iter()
                .any(|package| omg_lib::core::security::is_local_package_file(package));
            let kind = if is_local_artifact {
                omg_lib::package_managers::TransactionKind::InstallAurArtifact
            } else {
                omg_lib::package_managers::TransactionKind::Install
            };
            let result =
                omg_lib::package_managers::execute_transaction(packages.clone(), kind, None);
            if result.is_ok() {
                print_fast_success(&packages, "installed");
            }
            Some(result)
        }
        "remove" if !packages.is_empty() => {
            // Validate package names (security)
            omg_lib::core::security::validate_package_names(&packages).ok()?;
            let result = omg_lib::package_managers::execute_transaction(
                packages.clone(),
                omg_lib::package_managers::TransactionKind::Remove { recursive: false },
                None,
            );
            if result.is_ok() {
                print_fast_success(&packages, "removed");
            }
            Some(result)
        }
        "update" | "upgrade" => Some(execute_fast_system_update("")),
        "fullupdate" => {
            println!();
            println!(
                "  {} Syncing package databases...",
                omg_lib::cli::style::accent("→")
            );

            let sync_result = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(anyhow::Error::from)
                .and_then(|rt| rt.block_on(omg_lib::package_managers::sync_databases_parallel()));

            if let Err(e) = sync_result {
                return Some(Err(e));
            }

            println!("  {} Upgrading system...", omg_lib::cli::style::accent("→"));
            println!();

            Some(execute_fast_system_update(""))
        }
        "turboupdate" => {
            println!(
                "\n  {} Turbo upgrade (skipping sync)...\n",
                omg_lib::cli::style::community("🚀")
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

fn root_help_selection(args: &[String]) -> Option<bool> {
    let tail = args.get(1..)?;
    if tail.is_empty()
        || tail.len() > 2
        || tail
            .iter()
            .any(|arg| !matches!(arg.as_str(), "--help" | "-h" | "help" | "--all-commands"))
    {
        return None;
    }

    tail.iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h" | "help"))
        .then(|| tail.iter().any(|arg| arg == "--all-commands"))
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

    if args.len() == 3 && args[1] == "explicit" && matches!(args[2].as_str(), "--count" | "-c") {
        if omg_lib::core::paths::test_mode() {
            return packages::explicit_sync(true).is_ok();
        }
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
    if has_help_flag(args) {
        return Ok(false);
    }

    if matches!(args.len(), 3 | 4)
        && args[1] == "completions"
        && (args.len() == 3 || args[3] == "--stdout")
        && !has_json_flag(args)
    {
        let shell = &args[2];
        if shell.starts_with('-') {
            return Ok(false);
        }

        let stdout = args.len() == 4;

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
                if args.len() == 3 && !args[2].starts_with('-') && hooks::hook_env(&args[2]).is_ok()
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

fn strip_internal_invocation_markers(args: &mut Vec<String>, is_root: bool) -> (bool, bool) {
    let reexec_elevated = is_root
        && args.get(1).map(String::as_str) == Some(omg_lib::core::privilege::ELEVATED_MARKER);
    if !reexec_elevated {
        return (false, false);
    }
    args.remove(1);

    let parent_records =
        args.get(1).map(String::as_str) == Some(omg_lib::core::privilege::FLOW_PARENT_RECORDS);
    if parent_records {
        args.remove(1);
    }
    (true, parent_records)
}

#[cfg(unix)]
fn restore_sigpipe_default() -> Result<()> {
    use nix::sys::signal::{SigHandler, Signal, signal};

    // SAFETY: This runs once, before the runtime or any worker thread exists.
    // SIG_DFL restores normal Unix pipeline behavior without installing a
    // callback or accessing shared state from a signal handler.
    #[expect(
        unsafe_code,
        reason = "setting process signal disposition is inherently unsafe"
    )]
    unsafe {
        signal(Signal::SIGPIPE, SigHandler::SigDfl)?;
    }
    Ok(())
}

fn main() {
    #[cfg(unix)]
    if let Err(error) = restore_sigpipe_default() {
        finish(Err(error.context("Failed to configure Unix pipe handling")));
    }

    let mut args: Vec<String> = std::env::args().collect();

    if let Some(show_all) = root_help_selection(&args) {
        finish(omg_lib::cli::help::print_root_help(show_all));
    }

    // Strip the sudo re-exec marker. Elevation travels via argv because
    // sudo's env_reset strips OMG_ELEVATED from the child environment (see
    // ELEVATED_MARKER in core::privilege). The marker is honored ONLY for a
    // root process: anyone else invoking the reserved token keeps their
    // arguments untouched and gets clap's unknown-command error.
    // `reexec_elevated` is consumed only by the arch-gated fast path below;
    // backend-less builds legitimately ignore elevation markers entirely.
    #[cfg_attr(not(feature = "arch"), allow(unused_variables))]
    let (reexec_elevated, parent_records) =
        strip_internal_invocation_markers(&mut args, omg_lib::core::privilege::is_root());
    if reexec_elevated
        && args
            .get(1)
            .is_some_and(|arg| arg.starts_with(omg_lib::core::security::policy::POLICY_MARKER))
    {
        if let Err(error) = omg_lib::core::security::policy::inherit_policy(&args.remove(1)) {
            finish(Err(error));
        }
    }
    // The marker has already been authenticated by root re-exec parsing.
    // Preserve its history-ownership contract if flags route the child through
    // the full clap path instead of the minimal transaction path.
    omg_lib::core::privilege::set_parent_owns_history(parent_records);

    // FASTEST PATH: Elevated re-exec - skip ALL initialization
    // This runs when sudo omg re-execs us as root. Arch-only: the elevated
    // transaction path does not exist in backend-less builds, so the whole
    // block is gated out there instead of stubbed — no no-op stand-ins.
    #[cfg(feature = "arch")]
    {
        configure_fast_path_output(&args);
        if let Some(result) = try_fast_elevated(&args, reexec_elevated, parent_records) {
            finish(result);
        }
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
            eprintln!("Error: {error:#}");
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

    // Configure terminal rendering before any long-running command starts.
    omg_lib::cli::modern_ui::configure_output(cli.verbose, cli.quiet);

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

    if !omg_lib::core::paths::test_mode() && command_requires_root(&cli.command) && !is_root() {
        // Use run_self_sudo directly — elevate_if_needed creates a nested tokio
        // runtime which panics with "Cannot start a runtime from within a runtime"
        let args_refs: Vec<&str> = args.iter().skip(1).map(String::as_str).collect();
        omg_lib::core::privilege::run_self_sudo(&args_refs).await?;
        std::process::exit(0);
    }

    let ctx = omg_lib::cli::CliContext { json: cli.json };

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
        } => {
            let native_fedora = cfg!(feature = "fedora")
                && matches!(
                    omg_lib::core::env::distro::detect_distro(),
                    omg_lib::core::env::distro::Distro::Fedora,
                );
            !dry_run && (*orphans || *cache || *all) && !native_fedora
        }
        _ => false,
    }
}

fn validate_package_security(command: &Commands) -> Result<()> {
    match command {
        Commands::Install { packages, .. } => {
            #[cfg(any(feature = "debian", feature = "debian-pure"))]
            if omg_lib::core::env::distro::is_debian_like() {
                omg_lib::core::security::validate_debian_package_names_or_files(packages)?;
            } else {
                omg_lib::core::security::validate_package_names_or_files(packages)?;
            }
            #[cfg(not(any(feature = "debian", feature = "debian-pure")))]
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
        .with_writer(std::io::stderr)
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
        Commands::Account { .. } => "account",
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
        WorkspaceCommands::Check => omg_lib::cli::workspace::check(),
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

fn handle_privacy_command(command: Option<&omg_lib::cli::PrivacyCommands>) -> Result<()> {
    use omg_lib::cli::PrivacyCommands;
    use omg_lib::cli::telemetry;

    match command {
        Some(PrivacyCommands::Status) | None => telemetry::privacy_status(),
        Some(PrivacyCommands::Export { output }) => telemetry::export_data(output.as_deref()),
        Some(PrivacyCommands::OptOut) => telemetry::opt_out_api(),
        Some(PrivacyCommands::OptIn) => telemetry::opt_in_api(),
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
async fn handle_license_command(command: &AccountCommands) -> Result<()> {
    use omg_lib::cli::license;
    match command {
        AccountCommands::Link { token_stdin } => {
            let token = if *token_stdin {
                use std::io::Read;
                let mut token = String::new();
                std::io::stdin().take(16385).read_to_string(&mut token)?;
                anyhow::ensure!(
                    token.len() <= 16384,
                    "Dashboard token exceeds the input limit"
                );
                token.trim_end().to_owned()
            } else {
                std::env::var("OMG_DASHBOARD_TOKEN")
                    .context("Set OMG_DASHBOARD_TOKEN or use --token-stdin")?
            };
            anyhow::ensure!(!token.is_empty(), "Dashboard token is empty");
            license::activate(&token).await
        }
        AccountCommands::Status => license::status(),
        AccountCommands::Unlink => license::deactivate(),
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
        omg_lib::cli::init::run_defaults(skip_shell, skip_daemon).await
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
            watch,
            parallel,
            using,
            all,
        } => {
            let run_cmd = omg_lib::cli::run::RunCommand {
                task: task.clone(),
                args: args.clone(),
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
            allow_local_file,
        } => {
            packages::install(pkgs, *yes, *dry_run, *allow_local_file).await?;
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
        Commands::Use {
            runtime,
            version,
            uninstall,
        } => {
            if *uninstall {
                let Some(version) = version.as_deref() else {
                    anyhow::bail!(
                        "--uninstall requires a version: omg use <runtime> <version> --uninstall"
                    );
                };
                runtimes::uninstall_version(runtime, version)?;
            } else {
                runtimes::use_version(runtime, version.as_deref()).await?;
            }
        }
        Commands::List { runtime, available } => {
            runtimes::list_versions(runtime.as_deref(), *available, ctx.json).await?;
        }
        Commands::Hook { shell, uninstall } => {
            if *uninstall {
                if hooks::remove_hook(shell.as_str())? {
                    println!("Shell integration removed (rc file backed up with .omg-backup)");
                } else {
                    println!("No OMG shell integration found");
                }
            } else {
                hooks::print_hook(shell.as_str())?;
            }
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
        Commands::Privacy { command } => handle_privacy_command(command.as_ref())?,
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
        Commands::Account { command } => handle_license_command(command).await?,
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
            why::run(package, *reverse).await?;
        }
        Commands::Outdated => {
            outdated::run(ctx.json).await?;
        }
        Commands::Size { tree, limit } => {
            size::run(tree.as_deref(), *limit).await?;
        }
        Commands::Blame { package } => {
            blame::run(package).await?;
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
    use super::{
        has_json_flag, parse_fast_list_tail, root_help_selection, try_fast_completions,
        try_fast_explicit_count, try_fast_hooks,
    };

    #[cfg(feature = "arch")]
    use super::{split_elevated_invocation, strip_internal_invocation_markers};

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(std::string::ToString::to_string).collect()
    }

    #[cfg(feature = "arch")]
    #[test]
    fn elevated_local_archive_requires_parent_validated_consent() {
        let argv = args(&[
            "omg",
            "install",
            "--",
            "/var/cache/pkg/example-1.0-1-x86_64.pkg.tar.zst",
        ]);
        assert!(
            split_elevated_invocation(&argv, false).is_none(),
            "unowned local archives must use normal CLI consent validation"
        );
        assert_eq!(
            split_elevated_invocation(&argv, true),
            Some(("install", &argv[3..]))
        );
    }

    #[cfg(feature = "arch")]
    #[test]
    fn parent_history_marker_is_accepted_only_inside_root_reexec_protocol() {
        let mut internal = args(&[
            "omg",
            omg_lib::core::privilege::ELEVATED_MARKER,
            omg_lib::core::privilege::FLOW_PARENT_RECORDS,
            "install",
            "--",
            "ripgrep",
        ]);
        assert_eq!(
            strip_internal_invocation_markers(&mut internal, true),
            (true, true)
        );
        assert_eq!(internal, args(&["omg", "install", "--", "ripgrep"]));

        let mut direct = args(&[
            "omg",
            "install",
            "--",
            "ripgrep",
            omg_lib::core::privilege::FLOW_PARENT_RECORDS,
        ]);
        let original = direct.clone();
        assert_eq!(
            strip_internal_invocation_markers(&mut direct, true),
            (false, false)
        );
        assert_eq!(
            direct, original,
            "direct root argv must not gain protocol authority"
        );

        let mut non_root = args(&[
            "omg",
            omg_lib::core::privilege::ELEVATED_MARKER,
            omg_lib::core::privilege::FLOW_PARENT_RECORDS,
            "install",
        ]);
        let original = non_root.clone();
        assert_eq!(
            strip_internal_invocation_markers(&mut non_root, false),
            (false, false)
        );
        assert_eq!(non_root, original);
    }

    #[cfg(feature = "arch")]
    #[test]
    fn elevated_flags_after_separator_fall_through_to_clap() {
        // Regression for the blocker: `update -- --check` previously discarded
        // the flag tokens and force-ran a full system upgrade.
        assert!(
            split_elevated_invocation(&args(&["omg", "update", "--", "--check"]), true).is_none(),
            "check-only update must not take the transaction fast path"
        );
        assert!(
            split_elevated_invocation(&args(&["omg", "update", "--", "--dry-run"]), true).is_none(),
            "dry-run must not perform a real upgrade"
        );
        assert!(
            split_elevated_invocation(&args(&["omg", "install", "--", "-y", "ripgrep"]), true)
                .is_none(),
            "flag-looking package tokens must defer to clap"
        );
    }

    #[cfg(feature = "arch")]
    #[test]
    fn elevated_package_lists_require_the_exact_internal_shape() {
        assert!(
            split_elevated_invocation(
                &args(&["omg", "install", "extra", "--", "ripgrep", "jq"]),
                true
            )
            .is_none(),
            "pre-separator packages must fall through instead of disappearing"
        );
        let argv = args(&["omg", "install", "--", "ripgrep", "jq"]);
        let parsed = split_elevated_invocation(&argv, true)
            .map(|(command, packages)| (command, packages.to_vec()));
        assert_eq!(
            parsed,
            Some(("install", vec!["ripgrep".to_string(), "jq".to_string()]))
        );
    }

    #[cfg(feature = "arch")]
    #[test]
    fn elevated_invocations_without_separator_or_command_are_rejected() {
        assert!(split_elevated_invocation(&args(&["omg"]), true).is_none());
        assert!(split_elevated_invocation(&args(&["omg", "update"]), true).is_none());
        assert!(split_elevated_invocation(&args(&["--", "update"]), true).is_none());
        assert!(split_elevated_invocation(&args(&["omg", "--", "update"]), true).is_none());
        // `update --` with no trailing tokens stays on the fast path (empty
        // package list), matching the original elevated behavior.
        let argv = args(&["omg", "update", "--"]);
        let parsed =
            split_elevated_invocation(&argv, false).map(|(command, pkgs)| (command, pkgs.to_vec()));
        assert_eq!(parsed, Some(("update", Vec::new())));
    }

    #[cfg(feature = "arch")]
    #[test]
    fn unowned_elevated_mutations_defer_to_full_history_owner() {
        for command in ["remove", "install"] {
            let argv = args(&["omg", command, "--", "ripgrep"]);
            assert!(
                split_elevated_invocation(&argv, false).is_none(),
                "unowned {command} must use the normal history owner"
            );
            assert_eq!(
                split_elevated_invocation(&argv, true),
                Some((command, &argv[3..]))
            );
        }
        for command in ["update", "upgrade", "fullupdate", "turboupdate", "sync"] {
            let argv = args(&["omg", command, "--"]);
            assert_eq!(
                split_elevated_invocation(&argv, false),
                Some((command, &argv[3..]))
            );
        }
    }

    #[test]
    fn malformed_fast_path_tails_defer_to_clap() {
        assert!(!try_fast_explicit_count(&args(&[
            "omg", "explicit", "extra", "--count",
        ])));
        assert!(
            !try_fast_completions(&args(&["omg", "completions", "bash", "--stdout", "extra",]))
                .expect("fast completion parser")
        );
        assert!(!try_fast_hooks(&args(&[
            "omg", "hook-env", "bash", "extra",
        ])));
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
    fn root_help_selection_rejects_subcommand_scoped_flags() {
        assert_eq!(
            root_help_selection(&args_or_panic(&["omg", "--help", "--all-commands"])),
            Some(true)
        );
        assert_eq!(
            root_help_selection(&args_or_panic(&["omg", "help"])),
            Some(false)
        );
        assert_eq!(
            root_help_selection(&args_or_panic(&["omg", "clean", "--all"])),
            None
        );
        assert_eq!(
            root_help_selection(&args_or_panic(&["omg", "completions", "bash", "--help"])),
            None
        );
    }

    fn args_or_panic(list: &[&str]) -> Vec<String> {
        list.iter().map(std::string::ToString::to_string).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "arch")]
    fn pending_update() -> Vec<omg_lib::package_managers::types::UpdateInfo> {
        vec![omg_lib::package_managers::types::UpdateInfo {
            name: "example".to_string(),
            old_version: "1.0".to_string(),
            new_version: "2.0".to_string(),
            repo: "core".to_string(),
        }]
    }

    #[cfg(feature = "arch")]
    #[test]
    fn system_update_reports_history_failure_after_mutation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("history.json");
        std::fs::create_dir(&path)?;
        let history = omg_lib::core::history::HistoryManager::new_in(path)?;
        let mut mutated = false;

        let result = execute_recorded_system_update(&history, pending_update(), || {
            mutated = true;
            Ok(())
        });

        assert!(mutated);
        let error = result.expect_err("history failure must not report complete success");
        assert!(error.to_string().contains("Package operation succeeded"));
        Ok(())
    }

    #[cfg(feature = "arch")]
    #[test]
    fn system_update_preserves_both_operation_and_history_failures() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("history.json");
        std::fs::create_dir(&path)?;
        let history = omg_lib::core::history::HistoryManager::new_in(path)?;

        let error = execute_recorded_system_update(&history, pending_update(), || {
            Err(anyhow::anyhow!("transaction failed"))
        })
        .expect_err("both failures must be returned");
        let message = format!("{error:#}");
        assert!(message.contains("transaction failed"));
        assert!(message.contains("history persistence also failed"));
        Ok(())
    }

    #[cfg(feature = "arch")]
    #[test]
    fn system_update_records_versions_and_preserves_operation_error() -> Result<()> {
        use omg_lib::core::history::{HistoryManager, TransactionType};

        let directory = tempfile::tempdir()?;
        let history = HistoryManager::new_in(directory.path().join("history.json"))?;
        execute_recorded_system_update(&history, pending_update(), || Ok(()))?;
        let error = execute_recorded_system_update(&history, pending_update(), || {
            Err(anyhow::anyhow!("transaction failed"))
        })
        .expect_err("operation failure must be returned");
        assert_eq!(error.to_string(), "transaction failed");

        let transactions = history.load()?;
        assert_eq!(transactions.len(), 2);
        assert!(transactions[0].success);
        assert!(!transactions[1].success);
        for transaction in transactions {
            assert_eq!(transaction.transaction_type, TransactionType::Update);
            assert_eq!(transaction.changes[0].old_version.as_deref(), Some("1.0"));
            assert_eq!(transaction.changes[0].new_version.as_deref(), Some("2.0"));
        }
        Ok(())
    }

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
    fn clean_elevation_uses_native_fedora_or_root_dispatch() {
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
        let native_fedora = cfg!(feature = "fedora")
            && matches!(
                omg_lib::core::env::distro::detect_distro(),
                omg_lib::core::env::distro::Distro::Fedora,
            );
        assert_eq!(command_requires_root(&cache), !native_fedora);
        assert!(command_requires_root(&Commands::Sync));
    }
}
