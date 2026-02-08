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
use omg_lib::cli::{blame, ci, diff, migrate, outdated, pin, size, snapshot, why};
#[cfg(feature = "arch")]
use omg_lib::core::is_elevated;
use omg_lib::core::{elevate_if_needed, is_root, set_yes_flag};
use omg_lib::hooks;

/// Print minimal success message for fast elevated path
#[cfg(feature = "arch")]
fn print_fast_success(packages: &[String], action: &str) {
    use owo_colors::OwoColorize;

    let action_text = if packages.len() == 1 {
        action.to_string()
    } else {
        format!("packages {action}")
    };
    let msg = format!("  ✓ {} {}!  ", packages.len(), action_text);

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

/// ULTRA-FAST elevated path - when we're re-exec'd with sudo, skip ALL initialization
/// and go straight to the transaction. This eliminates ~150ms of startup overhead.
#[cfg(feature = "arch")]
fn try_fast_elevated(args: &[String]) -> Option<Result<()>> {
    // Only run this path when elevated via sudo
    if !is_elevated() {
        return None;
    }

    // Minimum args: ["omg", "command", "--"]
    if args.len() < 3 {
        return None;
    }

    let command = args.get(1)?;
    let separator_pos = args.iter().position(|a| a == "--")?;
    let packages: Vec<String> = args[separator_pos + 1..].to_vec();

    // Handle commands that may have packages
    match command.as_str() {
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
            if result.is_ok() {
                print_fast_success(&packages, "removed");
            }
            Some(result)
        }
        "update" | "upgrade" => {
            let result =
                omg_lib::package_managers::execute_transaction(Vec::new(), false, true, None);
            if result.is_ok() {
                print_system_updated("");
            }
            Some(result)
        }
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

            let result =
                omg_lib::package_managers::execute_transaction(Vec::new(), false, true, None);
            if result.is_ok() {
                print_system_updated("");
            }
            Some(result)
        }
        "turboupdate" => {
            use owo_colors::OwoColorize;
            println!(
                "\n  {} Turbo upgrade (skipping sync)...\n",
                "🚀".bright_magenta().bold()
            );
            let result =
                omg_lib::package_managers::execute_transaction(Vec::new(), false, true, None);
            if result.is_ok() {
                print_system_updated(" (turbo)");
            }
            Some(result)
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
const fn try_fast_elevated(_args: &[String]) -> Option<Result<()>> {
    None
}

fn has_help_flag(args: &[String]) -> bool {
    args.iter().any(|a| matches!(a.as_str(), "--help" | "-h"))
}

fn has_all_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "--all")
}

/// Ultra-fast path for explicit --count (bypasses tokio entirely)
fn try_fast_explicit_count(args: &[String]) -> bool {
    if has_help_flag(args) {
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
fn try_fast_search(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    if args.len() == 3 && (args[1] == "search" || args[1] == "s") {
        let query = &args[2];
        if query.starts_with('-') {
            return false;
        }
        if packages::search_sync_cli(query, false, false, true).unwrap_or_default() {
            return true;
        }
    }
    false
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
        if packages::info_sync_cli(package).unwrap_or_default() {
            return true;
        }
    }
    false
}

/// Ultra-fast path for completions
fn try_fast_completions(args: &[String]) -> Result<bool> {
    if args.len() >= 3 && args[1] == "completions" {
        let shell = &args[2];
        if shell.starts_with('-') {
            return Ok(false);
        }

        if has_help_flag(args) {
            let use_all = has_all_flag(args);
            let cli = Cli::try_parse_from(args.iter()).unwrap_or(Cli {
                verbose: 0,
                quiet: false,
                all: use_all,
                json: false,
                command: Commands::Status { fast: false },
            });
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

        if let Some(version) = runtimes::resolve_active_version(runtime) {
            println!(
                "{} {}",
                omg_lib::cli::style::runtime(runtime),
                omg_lib::cli::style::version(&version)
            );
        } else {
            println!(
                "{}: no version set (check .tool-versions, .nvmrc, etc.)",
                omg_lib::cli::style::runtime(runtime)
            );
        }
        return true;
    }
    false
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

        let runtime = if args.len() == 3 {
            let rt = &args[2];
            if rt.starts_with('-') {
                None
            } else {
                Some(rt.as_str())
            }
        } else {
            None
        };

        if runtimes::list_versions_sync(runtime).is_ok() {
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
                if args.len() >= 3 {
                    let shell = args
                        .iter()
                        .find(|a| !a.starts_with('-') && *a != "hook-env")
                        .map_or("", String::as_str);
                    if hooks::hook_env(shell).is_ok() {
                        return true;
                    }
                }
            }
            _ => {}
        }
    }
    false
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // FASTEST PATH: Elevated re-exec - skip ALL initialization
    // This runs when sudo omg re-execs us with OMG_ELEVATED=1
    if let Some(result) = try_fast_elevated(&args) {
        return result;
    }

    if try_fast_explicit_count(&args) {
        return Ok(());
    }
    if try_fast_search(&args) {
        return Ok(());
    }
    if try_fast_info(&args) {
        return Ok(());
    }
    if try_fast_completions(&args)? {
        return Ok(());
    }
    if try_fast_which(&args) {
        return Ok(());
    }
    if try_fast_list(&args) {
        return Ok(());
    }
    if try_fast_status(&args) {
        return Ok(());
    }
    if try_fast_hooks(&args) {
        return Ok(());
    }

    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async_main(args));

    if let Err(ref err) = result
        && let Some(suggestion) = omg_lib::core::error::suggest_for_anyhow(err)
    {
        tracing::info!("Suggestion: {}", suggestion);
    }

    result
}

async fn async_main(args: Vec<String>) -> Result<()> {
    // Record startup time for telemetry
    omg_lib::core::telemetry::record_startup_time();

    omg_lib::cli::style::init_theme();
    let cmd_start = omg_lib::core::analytics::start_timer();
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
    init_logging(cli.verbose);

    spawn_telemetry_ping();

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

    let needs_root = matches!(&cli.command, Commands::Sync | Commands::Clean { .. });
    if needs_root && !is_root() {
        elevate_if_needed(&args)?;
    }

    let ctx = omg_lib::cli::CliContext {
        verbose: cli.verbose,
        json: cli.json,
        quiet: cli.quiet,
        no_color: !console::colors_enabled(),
    };

    // Dispatch to command handlers
    dispatch_command(&cli.command, &ctx, cli.json).await?;

    // Track analytics
    track_command_analytics(cmd_start).await;

    Ok(())
}

/// Validate package names for security
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
fn init_logging(verbose: u8) {
    let default_level = if verbose > 0 || std::env::var("RUST_LOG").is_ok() {
        tracing::Level::INFO
    } else {
        tracing::Level::WARN
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive(default_level.into()),
        )
        .with_target(false)
        .with_ansi(console::colors_enabled())
        .without_time()
        .init();
}

/// Spawn telemetry ping on first run
fn spawn_telemetry_ping() {
    if omg_lib::core::telemetry::is_first_run() && !omg_lib::core::telemetry::is_telemetry_opt_out()
    {
        tokio::spawn(async {
            let _ = omg_lib::core::telemetry::ping_install().await;
        });
    }
}

/// Track command analytics and flush
async fn track_command_analytics(cmd_start: std::time::Instant) {
    let cmd_duration = omg_lib::core::analytics::end_timer(cmd_start);
    let cmd_name = std::env::args().nth(1).unwrap_or_default();
    let subcmd = std::env::args().nth(2);
    omg_lib::core::analytics::track_command(
        &cmd_name,
        subcmd.as_deref(),
        cmd_duration,
        true,
        Some(&omg_lib::core::telemetry::get_backend()),
    );
    omg_lib::core::analytics::maybe_heartbeat();
    omg_lib::core::analytics::maybe_flush().await;
    omg_lib::core::usage::maybe_sync_background();

    // Flush enhanced telemetry events if needed
    omg_lib::core::telemetry::maybe_flush_background();
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

async fn handle_privacy_command(
    command: Option<&omg_lib::cli::PrivacyCommands>,
) -> Result<()> {
    use omg_lib::cli::PrivacyCommands;
    use omg_lib::cli::telemetry;

    match command {
        Some(PrivacyCommands::Status) | None => telemetry::privacy_status().await,
        Some(PrivacyCommands::Export { output }) => {
            telemetry::export_data(output.as_deref()).await
        }
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

fn handle_which_command(runtime: &str) {
    if let Some(version) = runtimes::resolve_active_version(runtime) {
        println!(
            "{} {}",
            omg_lib::cli::style::runtime(runtime),
            omg_lib::cli::style::version(&version)
        );
    } else {
        println!(
            "{}: no version set (check .tool-versions, .nvmrc, etc.)",
            omg_lib::cli::style::runtime(runtime)
        );
    }
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
        CiCommands::Init { provider, advanced } => ci::init(provider, *advanced),
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

async fn handle_info_command(package: &str) -> Result<()> {
    if !packages::info_sync(package)? {
        packages::info_aur(package).await?;
    }
    Ok(())
}

#[expect(clippy::fn_params_excessive_bools)] // Maps directly to CLI flags: --detailed, --interactive, --json, --no-aur
async fn handle_search_command(
    query: &str,
    detailed: bool,
    interactive: bool,
    json_flag: bool,
    no_aur: bool,
) -> Result<()> {
    packages::search_with_json(query, detailed, interactive, json_flag, no_aur).await
}

async fn handle_install_command(packages: &[String], yes: bool, dry_run: bool) -> Result<()> {
    packages::install(packages, yes, dry_run).await
}

async fn handle_remove_command(
    packages: &[String],
    recursive: bool,
    yes: bool,
    dry_run: bool,
) -> Result<()> {
    packages::remove(packages, recursive, yes, dry_run).await
}

#[expect(clippy::fn_params_excessive_bools)] // Maps directly to CLI flags: --orphans, --cache, --aur, --all
async fn handle_clean_command(
    orphans: bool,
    cache: bool,
    aur: bool,
    all: bool,
    dry_run: bool,
) -> Result<()> {
    packages::clean(orphans, cache, aur, all, dry_run).await
}

async fn handle_complete_command(
    shell: &str,
    current: &str,
    last: &str,
    full: Option<&str>,
) -> Result<()> {
    commands::complete(shell, current, last, full).await
}

fn handle_history_command(
    limit: usize,
    search: Option<&str>,
    transaction_type: Option<&str>,
    from: Option<&str>,
    to: Option<&str>,
) -> Result<()> {
    commands::history(limit, search, transaction_type, from, to)
}

/// Main command dispatcher - routes commands to appropriate handlers
#[expect(clippy::too_many_lines)]
async fn dispatch_command(
    command: &Commands,
    ctx: &omg_lib::cli::CliContext,
    json_flag: bool,
) -> Result<()> {
    match command {
        Commands::Run { .. }
        | Commands::Tool { .. }
        | Commands::Env { .. }
        | Commands::Fleet { .. }
        | Commands::Team { .. }
        | Commands::Enterprise { .. } => {
            command.execute(ctx).await?;
        }
        Commands::Search {
            query,
            detailed,
            interactive,
            no_aur,
        } => {
            handle_search_command(query, *detailed, *interactive, json_flag, *no_aur).await?;
        }
        Commands::Install {
            packages,
            yes,
            dry_run,
        } => {
            handle_install_command(packages, *yes, *dry_run).await?;
        }
        Commands::Remove {
            packages: pkgs,
            recursive,
            yes,
            dry_run,
        } => {
            handle_remove_command(pkgs, *recursive, *yes, *dry_run).await?;
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
        Commands::Info { package } => handle_info_command(package).await?,
        Commands::Clean {
            orphans,
            cache,
            aur,
            all,
            dry_run,
        } => {
            handle_clean_command(*orphans, *cache, *aur, *all, *dry_run).await?;
        }
        Commands::Explicit { count } => {
            packages::explicit_sync_with_json(*count, json_flag)?;
        }
        Commands::Sync => {
            packages::sync().await?;
        }
        Commands::Use { runtime, version } => {
            runtimes::use_version(runtime, version.as_deref()).await?;
        }
        Commands::List { runtime, available } => {
            runtimes::list_versions(runtime.as_deref(), *available).await?;
        }
        Commands::Hook { shell } => {
            hooks::print_hook(shell)?;
        }
        Commands::Hooks { command } => handle_hooks_command(command)?,
        Commands::Workspace { command } => handle_workspace_command(command).await?,
        Commands::HookEnv { shell } => {
            hooks::hook_env(shell)?;
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
            hooks::completions::generate_completions(shell, *stdout)?;
        }
        Commands::Which { runtime } => handle_which_command(runtime),
        Commands::Complete {
            shell,
            current,
            last,
            full,
        } => {
            handle_complete_command(shell, current, last, full.as_deref()).await?;
        }
        Commands::Status { fast } => {
            packages::status_with_json(*fast, json_flag).await?;
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
            new::run(stack, name)?;
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
            handle_history_command(
                *limit,
                search.as_deref(),
                transaction_type.as_deref(),
                from.as_deref(),
                to.as_deref(),
            )?;
        }
        Commands::Rollback { id, yes } => {
            commands::rollback(id.clone(), *yes).await?;
        }
        Commands::Dash => {
            omg_lib::cli::tui::run().await?;
        }
        Commands::Stats => {
            commands::stats()?;
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
        Commands::Outdated { security } => {
            outdated::run(*security, json_flag).await?;
        }
        Commands::Pin {
            target,
            unpin,
            list,
        } => {
            pin::run(target, *unpin, *list)?;
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
