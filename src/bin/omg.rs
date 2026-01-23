//! OMG CLI Binary
//!
//! The main command-line interface for OMG package manager.
//!
//! Uses a single tokio runtime for all async operations (Rust 2024 best practice).

use anyhow::Result;
use clap::Parser;

#[cfg(feature = "license")]
use omg_lib::cli::LicenseCommands;
use omg_lib::cli::doctor;
use omg_lib::cli::env;
use omg_lib::cli::new;
use omg_lib::cli::packages;
use omg_lib::cli::runtimes;
use omg_lib::cli::security;
use omg_lib::cli::tool;
use omg_lib::cli::{
    CiCommands, Cli, Commands, ContainerCommands, EnterpriseCommands, EnterprisePolicyCommands,
    EnvCommands, FleetCommands, GoldenPathCommands, MigrateCommands, NotifyCommands,
    ServerCommands, SnapshotCommands, TeamCommands, TeamRoleCommands, ToolCommands, commands,
};
use omg_lib::cli::{
    blame, ci, diff, enterprise, fleet, migrate, outdated, pin, size, snapshot, why,
};
use omg_lib::core::{RuntimeBackend, elevate_if_needed, is_root, task_runner};
use omg_lib::hooks;

// Using system allocator (pure Rust - no C dependency)

fn has_help_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "--help" || a == "-h")
}

/// Ultra-fast path for explicit --count (bypasses tokio entirely)
/// This shaves ~2ms by avoiding runtime initialization
fn try_fast_explicit_count(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    // Check for "omg explicit --count" or "omg explicit -c"
    if args.len() >= 2 && args[1] == "explicit" && args.iter().any(|a| a == "--count" || a == "-c")
    {
        // Try fast path first
        if let Some(count) = omg_lib::core::fast_status::FastStatus::read_explicit_count() {
            println!("{count}");
            return true;
        }
        // Fallback to sync path
        if packages::explicit_sync(true).is_ok() {
            return true;
        }
    }
    false
}

/// Ultra-fast path for simple search (bypasses tokio entirely)
/// This shaves ~2ms by avoiding runtime initialization
fn try_fast_search(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    // Check for "omg search <query>" or "omg s <query>" (simple search, no flags)
    if args.len() == 3 && (args[1] == "search" || args[1] == "s") {
        let query = &args[2];
        // Skip if query looks like a flag
        if query.starts_with('-') {
            return false;
        }
        // Use sync path directly
        if packages::search_sync_cli(query, false, false).unwrap_or(false) {
            return true;
        }
    }
    false
}

/// Ultra-fast path for simple info (bypasses tokio entirely)
fn try_fast_info(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    // Check for "omg info <package>" (simple info, no flags)
    if args.len() == 3 && args[1] == "info" {
        let package = &args[2];
        // Skip if package looks like a flag
        if package.starts_with('-') {
            return false;
        }
        // Use sync path directly
        if packages::info_sync_cli(package).unwrap_or(false) {
            return true;
        }
    }
    false
}

/// Ultra-fast path for completions (bypasses tokio entirely)
fn try_fast_completions(args: &[String]) -> Result<bool> {
    if has_help_flag(args) {
        return Ok(false);
    }

    // Check for "omg completions <shell>" or "omg completions <shell> --stdout"
    if args.len() >= 3 && args[1] == "completions" {
        let shell = &args[2];
        // Skip if shell looks like a flag
        if shell.starts_with('-') {
            return Ok(false);
        }

        let stdout = args.iter().any(|a| a == "--stdout");

        // We can only do this fast if it's one of the shells that uses include_str!
        // or if we don't mind building the Command object once without tokio.
        // For Bash, Zsh, Fish, it's instant.
        match shell.to_lowercase().as_str() {
            "bash" | "zsh" | "fish" => {
                hooks::completions::generate_completions(shell, stdout)?;
                return Ok(true);
            }
            _ => {
                // For other shells, we'll let it fall through to the normal path
                // since they need Cli::command() which is slow anyway,
                // or we could implement it here too.
                return Ok(false);
            }
        }
    }
    Ok(false)
}

/// Ultra-fast path for which command (bypasses tokio entirely)
fn try_fast_which(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    // Check for "omg which <runtime>" (simple which, no flags)
    if args.len() == 3 && args[1] == "which" {
        let runtime = &args[2];
        // Skip if runtime looks like a flag
        if runtime.starts_with('-') {
            return false;
        }

        if let Some(version) = runtimes::resolve_active_version(runtime) {
            println!("{runtime} {version}");
        } else {
            println!("{runtime}: no version set (check .tool-versions, .nvmrc, etc.)");
        }
        return true;
    }
    false
}

/// Ultra-fast path for list command (bypasses tokio entirely)
fn try_fast_list(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    // Check for "omg list" or "omg list <runtime>" (no --available)
    if args.len() >= 2 && (args[1] == "list" || args[1] == "ls") {
        // If --available or -a is present, we need tokio
        if args.iter().any(|a| a == "--available" || a == "-a") {
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

        // Use sync path directly
        if runtimes::list_versions_sync(runtime).is_ok() {
            return true;
        }
    }
    false
}

/// Ultra-fast path for status command (bypasses tokio entirely)
fn try_fast_status(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    if args.len() == 2 && args[1] == "status" && commands::status_sync().is_ok() {
        return true;
    }
    false
}

/// Ultra-fast path for hook commands (bypasses tokio entirely)
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
                        .map(String::as_str)
                        .unwrap_or("");
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
    // Collect args once to avoid multiple syscalls/allocations in fast paths
    let args: Vec<String> = std::env::args().collect();

    // ULTRA FAST PATHS: bypass tokio runtime for common commands
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

    // Normal path with tokio runtime (current_thread for faster startup)
    let result = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(async_main(args));

    // Handle errors with helpful suggestions
    if let Err(ref err) = result
        && let Some(suggestion) = omg_lib::core::error::suggest_for_anyhow(err)
    {
        eprintln!("\n💡 {suggestion}");
    }

    result
}

/// Main entry point - uses single tokio runtime for all async operations
/// This eliminates the overhead of creating a new runtime for each command
#[allow(clippy::too_many_lines)]
async fn async_main(args: Vec<String>) -> Result<()> {
    // Start analytics timer
    let cmd_start = omg_lib::core::analytics::start_timer();

    // Parse CLI arguments first so we can use them to configure tracing
    // and other early initialization steps.
    let cli = Cli::parse_from(&args);

    // SECURITY: Validate all package names in relevant commands
    match &cli.command {
        Commands::Install { packages, .. } | Commands::Remove { packages, .. } => {
            omg_lib::core::security::validate_package_names(packages)?;
        }
        Commands::Info { package }
        | Commands::Why { package, .. }
        | Commands::Blame { package } => {
            omg_lib::core::security::validate_package_name(package)?;
        }
        _ => {}
    }

    // Initialize tracing (deferred until after parsing)
    // Only use full formatting if not a simple command
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_target(false)
        .with_ansi(console::colors_enabled())
        .init();

    // First-run telemetry (opt-out with OMG_TELEMETRY=0)
    if omg_lib::core::telemetry::is_first_run() && !omg_lib::core::telemetry::is_telemetry_opt_out()
    {
        tokio::spawn(async {
            if let Err(e) = omg_lib::core::telemetry::ping_install().await {
                tracing::debug!("Failed to ping install telemetry: {}", e);
            }
        });
    }

    // Commands that require root - auto-elevate with sudo
    // Note: Install/Remove/Update handle elevation internally via subprocess
    // to allow user-level operations (like AUR building) to persist.
    let needs_root = matches!(&cli.command, Commands::Sync | Commands::Clean { .. });

    if needs_root && !is_root() {
        // Re-execute with sudo - this replaces the current process
        elevate_if_needed(&args)?;
        // Never reaches here
    }

    // Handle commands - all async operations use the single runtime
    match cli.command {
        Commands::Search {
            query,
            detailed,
            interactive,
        } => {
            // PURE SYNC PATH (Sub-10ms)
            if !packages::search_sync_cli(&query, detailed, interactive)? {
                // Fallback to async if needed
                packages::search(&query, detailed, interactive).await?;
            }
        }
        Commands::Install { packages, yes } => {
            packages::install(&packages, yes).await?;
        }
        Commands::Remove {
            packages: pkgs,
            recursive,
            yes,
        } => {
            packages::remove(&pkgs, recursive, yes).await?;
        }
        Commands::Update { check, yes } => {
            packages::update(check, yes).await?;
        }
        Commands::Info { package } => {
            // 1. Try SYNC PATH (Official + Local)
            if !packages::info_sync(&package)? {
                // 2. Fallback to ASYNC PATH (AUR)
                packages::info_aur(&package).await?;
            }
        }
        Commands::Clean {
            orphans,
            cache,
            aur,
            all,
        } => {
            packages::clean(orphans, cache, aur, all).await?;
        }
        Commands::Explicit { count } => {
            // PURE SYNC PATH (Sub-10ms)
            packages::explicit_sync(count)?;
        }
        Commands::Sync => {
            packages::sync().await?;
        }
        Commands::Use { runtime, version } => {
            runtimes::use_version(&runtime, version.as_deref()).await?;
        }
        Commands::List { runtime, available } => {
            runtimes::list_versions(runtime.as_deref(), available).await?;
        }
        Commands::Hook { shell } => {
            hooks::print_hook(&shell)?;
        }
        Commands::HookEnv { shell } => {
            hooks::hook_env(&shell)?;
        }
        Commands::Daemon { foreground } => {
            commands::daemon(foreground)?;
        }
        Commands::Config { key, value } => {
            commands::config(key.as_deref(), value.as_deref())?;
        }
        Commands::Completions { shell, stdout } => {
            hooks::completions::generate_completions(&shell, stdout)?;
        }
        Commands::Which { runtime } => {
            if let Some(version) = runtimes::resolve_active_version(&runtime) {
                println!("{runtime} {version}");
            } else {
                println!("{runtime}: no version set (check .tool-versions, .nvmrc, etc.)");
            }
        }
        Commands::Complete {
            shell,
            current,
            last,
            full,
        } => {
            commands::complete(&shell, &current, &last, full.as_deref()).await?;
        }
        Commands::Status { fast } => {
            packages::status(fast).await?;
        }
        Commands::Doctor => {
            doctor::run().await?;
        }
        Commands::Audit { command } => {
            use omg_lib::cli::AuditCommands;
            match command {
                Some(AuditCommands::Scan) | None => {
                    security::scan().await?;
                }
                Some(AuditCommands::Sbom { output, vulns }) => {
                    security::generate_sbom(output, vulns).await?;
                }
                Some(AuditCommands::Secrets { path }) => {
                    security::scan_secrets(path)?;
                }
                Some(AuditCommands::Log { limit, severity }) => {
                    security::view_audit_log(limit, severity)?;
                }
                Some(AuditCommands::Verify) => {
                    security::verify_audit_log()?;
                }
                Some(AuditCommands::Policy) => {
                    security::show_policy()?;
                }
                Some(AuditCommands::Slsa { package }) => {
                    security::check_slsa(&package).await?;
                }
            }
        }
        Commands::Run {
            task,
            args,
            runtime_backend,
            watch,
            parallel,
        } => {
            let backend = runtime_backend
                .as_deref()
                .map(str::parse::<RuntimeBackend>)
                .transpose()
                .map_err(|err| anyhow::anyhow!(err))?;
            if watch {
                task_runner::run_task_watch(&task, &args, backend).await?;
            } else if parallel {
                task_runner::run_tasks_parallel(&task, &args, backend).await?;
            } else {
                task_runner::run_task(&task, &args, backend)?;
            }
        }
        Commands::New { stack, name } => {
            new::run(&stack, &name)?;
        }
        Commands::Tool { command } => match command {
            ToolCommands::Install { name } => {
                tool::install(&name).await?;
            }
            ToolCommands::List => {
                tool::list()?;
            }
            ToolCommands::Remove { name } => {
                tool::remove(&name)?;
            }
            ToolCommands::Update { name } => {
                tool::update(&name).await?;
            }
            ToolCommands::Search { query } => {
                tool::search(&query)?;
            }
            ToolCommands::Registry => {
                tool::registry()?;
            }
        },
        Commands::Env { command } => match command {
            EnvCommands::Capture => {
                env::capture().await?;
            }
            EnvCommands::Check => {
                env::check().await?;
            }
            EnvCommands::Share {
                description,
                public,
            } => {
                env::share(description, public).await?;
            }
            EnvCommands::Sync { url } => {
                env::sync(url).await?;
            }
        },
        Commands::Team { command } => {
            use omg_lib::cli::team;
            match command {
                TeamCommands::Init { team_id, name } => {
                    team::init(&team_id, name.as_deref())?;
                }
                TeamCommands::Join { url } => {
                    team::join(&url).await?;
                }
                TeamCommands::Status => {
                    team::status().await?;
                }
                TeamCommands::Push => {
                    team::push().await?;
                }
                TeamCommands::Pull => {
                    team::pull().await?;
                }
                TeamCommands::Members => {
                    team::members().await?;
                }
                TeamCommands::Dashboard => {
                    team::dashboard().await?;
                }
                TeamCommands::Invite { email, role } => {
                    team::invite(email.as_deref(), &role)?;
                }
                TeamCommands::Roles { command: role_cmd } => match role_cmd {
                    TeamRoleCommands::List => {
                        team::roles::list()?;
                    }
                    TeamRoleCommands::Assign { member, role } => {
                        team::roles::assign(&member, &role)?;
                    }
                    TeamRoleCommands::Remove { member } => {
                        team::roles::remove(&member)?;
                    }
                },
                TeamCommands::Propose { message } => {
                    team::propose(&message).await?;
                }
                TeamCommands::Proposals => {
                    team::list_proposals().await?;
                }
                TeamCommands::Review { id, approve, .. } => {
                    team::review(id, approve).await?;
                }
                TeamCommands::GoldenPath { command: gp_cmd } => match gp_cmd {
                    GoldenPathCommands::Create {
                        name,
                        node,
                        python,
                        packages,
                    } => {
                        team::golden_path::create(
                            &name,
                            node.as_deref(),
                            python.as_deref(),
                            packages.as_deref(),
                        )?;
                    }
                    GoldenPathCommands::List => {
                        team::golden_path::list()?;
                    }
                    GoldenPathCommands::Delete { name } => {
                        team::golden_path::delete(&name)?;
                    }
                },
                TeamCommands::Compliance { export, enforce } => {
                    team::compliance(export.as_deref(), enforce)?;
                }
                TeamCommands::Activity { days } => {
                    team::activity(days).await?;
                }
                TeamCommands::Notify {
                    command: notify_cmd,
                } => match notify_cmd {
                    NotifyCommands::Add { notify_type, url } => {
                        team::notify::add(&notify_type, &url)?;
                    }
                    NotifyCommands::List => {
                        team::notify::list()?;
                    }
                    NotifyCommands::Remove { id } => {
                        team::notify::remove(&id)?;
                    }
                    NotifyCommands::Test { id } => {
                        team::notify::test(&id)?;
                    }
                },
            }
        }
        Commands::Container { command } => {
            use omg_lib::cli::container;
            match command {
                ContainerCommands::Status => {
                    container::status()?;
                }
                ContainerCommands::Run {
                    image,
                    command: cmd,
                    name,
                    detach,
                    interactive,
                    env,
                    volume,
                    workdir,
                } => {
                    container::run(
                        &image,
                        &cmd,
                        name,
                        detach,
                        interactive,
                        &env,
                        &volume,
                        workdir,
                    )?;
                }
                ContainerCommands::Shell {
                    image,
                    workdir,
                    env,
                    volume,
                } => {
                    container::shell(image, workdir, &env, &volume)?;
                }
                ContainerCommands::Build {
                    dockerfile,
                    tag,
                    no_cache,
                    build_arg,
                    target,
                } => {
                    container::build(dockerfile, &tag, no_cache, &build_arg, &target)?;
                }
                ContainerCommands::List => {
                    container::list()?;
                }
                ContainerCommands::Images => {
                    container::images()?;
                }
                ContainerCommands::Pull { image } => {
                    container::pull(&image)?;
                }
                ContainerCommands::Stop { container: c } => {
                    container::stop(&c)?;
                }
                ContainerCommands::Exec {
                    container: c,
                    command: cmd,
                } => {
                    container::exec(&c, &cmd)?;
                }
                ContainerCommands::Init { base } => {
                    container::init(base)?;
                }
            }
        }
        #[cfg(feature = "license")]
        Commands::License { command } => {
            use omg_lib::cli::license;
            match command {
                LicenseCommands::Activate { key } => {
                    license::activate(&key).await?;
                }
                LicenseCommands::Status => {
                    license::status()?;
                }
                LicenseCommands::Deactivate => {
                    license::deactivate()?;
                }
                LicenseCommands::Check { feature } => {
                    license::check_feature(&feature)?;
                }
            }
        }
        Commands::History { limit } => {
            commands::history(limit)?;
        }
        Commands::Rollback { id } => {
            commands::rollback(id)?;
        }
        Commands::Dash => {
            omg_lib::cli::tui::run().await?;
        }
        Commands::Stats => {
            commands::stats()?;
        }
        Commands::Metrics => {
            commands::metrics().await?;
        }
        Commands::Init {
            defaults,
            skip_shell,
            skip_daemon,
        } => {
            if defaults {
                omg_lib::cli::init::run_defaults().await?;
            } else {
                omg_lib::cli::init::run_interactive(skip_shell, skip_daemon).await?;
            }
        }
        // New commands
        Commands::Why { package, reverse } => {
            why::run(&package, reverse)?;
        }
        Commands::Outdated { security, json } => {
            outdated::run(security, json)?;
        }
        Commands::Pin {
            target,
            unpin,
            list,
        } => {
            pin::run(&target, unpin, list)?;
        }
        Commands::Size { tree, limit } => {
            size::run(tree.as_deref(), limit)?;
        }
        Commands::Blame { package } => {
            blame::run(&package)?;
        }
        Commands::Diff { from, to } => {
            diff::run(from.as_deref(), &to).await?;
        }
        Commands::Snapshot { command } => match command {
            SnapshotCommands::Create { message } => {
                snapshot::create(message).await?;
            }
            SnapshotCommands::List => {
                snapshot::list()?;
            }
            SnapshotCommands::Restore { id, dry_run, yes } => {
                snapshot::restore(&id, dry_run, yes).await?;
            }
            SnapshotCommands::Delete { id } => {
                snapshot::delete(&id)?;
            }
        },
        Commands::Ci { command } => match command {
            CiCommands::Init { provider } => {
                ci::init(&provider)?;
            }
            CiCommands::Validate => {
                ci::validate().await?;
            }
            CiCommands::Cache => {
                ci::cache()?;
            }
        },
        Commands::Migrate { command } => match command {
            MigrateCommands::Export { output } => {
                migrate::export(&output).await?;
            }
            MigrateCommands::Import { manifest, dry_run } => {
                migrate::import(&manifest, dry_run).await?;
            }
        },
        Commands::Fleet { command } => match command {
            FleetCommands::Status => {
                fleet::status().await?;
            }
            FleetCommands::Push { team, message } => {
                fleet::push(team.as_deref(), message.as_deref())?;
            }
            FleetCommands::Remediate { dry_run, confirm } => {
                fleet::remediate(dry_run, confirm)?;
            }
        },
        Commands::Enterprise { command } => match command {
            EnterpriseCommands::Reports {
                report_type,
                format,
            } => {
                enterprise::reports(&report_type, &format).await?;
            }
            EnterpriseCommands::Policy {
                command: policy_cmd,
            } => match policy_cmd {
                EnterprisePolicyCommands::Set { scope, rule } => {
                    enterprise::policy::set(&scope, &rule)?;
                }
                EnterprisePolicyCommands::Show { scope } => {
                    enterprise::policy::show(scope.as_deref()).await?;
                }
                EnterprisePolicyCommands::Inherit { from, to } => {
                    enterprise::policy::inherit(&from, &to)?;
                }
            },
            EnterpriseCommands::AuditExport {
                format,
                period,
                output,
            } => {
                enterprise::audit_export(&format, period.as_deref(), &output)?;
            }
            EnterpriseCommands::LicenseScan { export } => {
                enterprise::license_scan(export.as_deref())?;
            }
            EnterpriseCommands::Server {
                command: server_cmd,
            } => match server_cmd {
                ServerCommands::Init {
                    license,
                    storage,
                    domain,
                } => {
                    enterprise::server::init(&license, &storage, &domain)?;
                }
                ServerCommands::Mirror { upstream } => {
                    enterprise::server::mirror(&upstream)?;
                }
            },
        },
    }

    // Track command execution for analytics
    let cmd_duration = omg_lib::core::analytics::end_timer(cmd_start);
    let cmd_name = std::env::args().nth(1).unwrap_or_default();
    let subcmd = std::env::args().nth(2);
    omg_lib::core::analytics::track_command(&cmd_name, subcmd.as_deref(), cmd_duration, true);

    // Send heartbeat and analytics - wait for completion to ensure telemetry is sent
    omg_lib::core::analytics::maybe_heartbeat();
    let _ = omg_lib::core::analytics::flush_events().await;

    // Sync usage - wait for completion to ensure telemetry is sent
    omg_lib::core::usage::sync_usage_now().await;

    Ok(())
}
