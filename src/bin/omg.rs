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
use omg_lib::cli::new;
use omg_lib::cli::packages;
use omg_lib::cli::runtimes;
use omg_lib::cli::security;

use omg_lib::cli::{
    CiCommands, Cli, Commands, ContainerCommands, MigrateCommands, SnapshotCommands, commands,
};
use omg_lib::cli::{blame, ci, diff, migrate, outdated, pin, size, snapshot, why};
use omg_lib::core::{elevate_if_needed, is_root, set_yes_flag};
use omg_lib::hooks;

fn has_help_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "--help" || a == "-h")
}

fn has_all_flag(args: &[String]) -> bool {
    args.iter().any(|a| a == "--all")
}

/// Ultra-fast path for explicit --count (bypasses tokio entirely)
fn try_fast_explicit_count(args: &[String]) -> bool {
    if has_help_flag(args) {
        return false;
    }

    if args.len() >= 2 && args[1] == "explicit" && args.iter().any(|a| a == "--count" || a == "-c")
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
        if packages::search_sync_cli(query, false, false).unwrap_or(false) {
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
        if packages::info_sync_cli(package).unwrap_or(false) {
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

    if args.len() >= 2 && (args[1] == "list" || args[1] == "ls") {
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
        eprintln!("\n💡 {suggestion}");
    }

    result
}

async fn async_main(args: Vec<String>) -> Result<()> {
    omg_lib::cli::style::init_theme();
    let cmd_start = omg_lib::core::analytics::start_timer();
    let cli = Cli::parse_from(&args);

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

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_target(false)
        .with_ansi(console::colors_enabled())
        .init();

    if omg_lib::core::telemetry::is_first_run() && !omg_lib::core::telemetry::is_telemetry_opt_out()
    {
        tokio::spawn(async {
            let _ = omg_lib::core::telemetry::ping_install().await;
        });
    }

    let needs_root = matches!(&cli.command, Commands::Sync | Commands::Clean { .. });
    if needs_root && !is_root() {
        elevate_if_needed(&args)?;
    }

    let ctx = omg_lib::cli::CliContext {
        verbose: cli.verbose,
        json: false,
        quiet: cli.quiet,
        no_color: !console::colors_enabled(),
    };

    match &cli.command {
        Commands::Run { .. }
        | Commands::Tool { .. }
        | Commands::Env { .. }
        | Commands::Fleet { .. }
        | Commands::Team { .. }
        | Commands::Enterprise { .. } => {
            use omg_lib::cli::CommandRunner;
            cli.command.execute(&ctx).await?;
        }
        Commands::Search {
            query,
            detailed,
            interactive,
        } => {
            packages::search(query, *detailed, *interactive).await?;
        }
        Commands::Install { packages, yes } => {
            packages::install(packages, *yes).await?;
        }
        Commands::Remove {
            packages: pkgs,
            recursive,
            yes,
        } => {
            packages::remove(pkgs, *recursive, *yes).await?;
        }
        Commands::Update { check, yes } => {
            packages::update(*check, *yes).await?;
        }
        Commands::Info { package } => {
            if !packages::info_sync(package)? {
                packages::info_aur(package).await?;
            }
        }
        Commands::Clean {
            orphans,
            cache,
            aur,
            all,
        } => {
            packages::clean(*orphans, *cache, *aur, *all).await?;
        }
        Commands::Explicit { count } => {
            packages::explicit_sync(*count)?;
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
        Commands::HookEnv { shell } => {
            hooks::hook_env(shell)?;
        }
        Commands::Daemon { foreground } => {
            commands::daemon(*foreground)?;
        }
        Commands::Config { key, value } => {
            commands::config(key.as_deref(), value.as_deref())?;
        }
        Commands::SelfUpdate { force, version } => {
            omg_lib::cli::self_update::run(*force, version.clone()).await?;
        }
        Commands::Completions { shell, stdout } => {
            hooks::completions::generate_completions(shell, *stdout)?;
        }
        Commands::Which { runtime } => {
            if let Some(version) = runtimes::resolve_active_version(runtime) {
                println!("{runtime} {version}");
            } else {
                println!("{runtime}: no version set");
            }
        }
        Commands::Complete {
            shell,
            current,
            last,
            full,
        } => {
            commands::complete(shell, current, last, full.as_deref()).await?;
        }
        Commands::Status { fast } => {
            packages::status(*fast).await?;
        }
        Commands::Doctor => {
            doctor::run().await?;
        }
        Commands::Audit { command } => {
            use omg_lib::cli::CommandRunner;
            if let Some(cmd) = command {
                cmd.execute(&ctx).await?;
            } else {
                security::scan(&ctx).await?;
            }
        }
        Commands::New { stack, name } => {
            new::run(stack, name)?;
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
                        image,
                        cmd,
                        name.clone(),
                        *detach,
                        *interactive,
                        env,
                        volume,
                        workdir.clone(),
                    )?;
                }
                ContainerCommands::Shell {
                    image,
                    workdir,
                    env,
                    volume,
                } => {
                    container::shell(image.clone(), workdir.clone(), env, volume)?;
                }
                ContainerCommands::Build {
                    dockerfile,
                    tag,
                    no_cache,
                    build_arg,
                    target,
                } => {
                    container::build(dockerfile.clone(), tag, *no_cache, build_arg, target)?;
                }
                ContainerCommands::List => {
                    container::list()?;
                }
                ContainerCommands::Images => {
                    container::images()?;
                }
                ContainerCommands::Pull { image } => {
                    container::pull(image)?;
                }
                ContainerCommands::Stop { container: c } => {
                    container::stop(c)?;
                }
                ContainerCommands::Exec {
                    container: c,
                    command: cmd,
                } => {
                    container::exec(c, cmd)?;
                }
                ContainerCommands::Init { base } => {
                    container::init(base.clone())?;
                }
            }
        }
        #[cfg(feature = "license")]
        Commands::License { command } => {
            use omg_lib::cli::license;
            match command {
                LicenseCommands::Activate { key } => {
                    license::activate(key).await?;
                }
                LicenseCommands::Status => {
                    license::status()?;
                }
                LicenseCommands::Deactivate => {
                    license::deactivate()?;
                }
                LicenseCommands::Check { feature } => {
                    license::check_feature(feature)?;
                }
            }
        }
        Commands::History { limit } => {
            commands::history(*limit)?;
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
        Commands::Metrics => {
            commands::metrics().await?;
        }
        Commands::Init {
            defaults,
            skip_shell,
            skip_daemon,
        } => {
            if *defaults {
                omg_lib::cli::init::run_defaults().await?;
            } else {
                omg_lib::cli::init::run_interactive(*skip_shell, *skip_daemon).await?;
            }
        }
        Commands::Why { package, reverse } => {
            why::run(package, *reverse)?;
        }
        Commands::Outdated { security, json } => {
            outdated::run(*security, *json).await?;
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
        Commands::Snapshot { command } => match command {
            SnapshotCommands::Create { message } => {
                snapshot::create(message.clone()).await?;
            }
            SnapshotCommands::List => {
                snapshot::list()?;
            }
            SnapshotCommands::Restore { id, dry_run, yes } => {
                snapshot::restore(id, *dry_run, *yes).await?;
            }
            SnapshotCommands::Delete { id } => {
                snapshot::delete(id)?;
            }
        },
        Commands::Ci { command } => match command {
            CiCommands::Init { provider, advanced } => {
                ci::init(provider, *advanced)?;
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
                migrate::export(output).await?;
            }
            MigrateCommands::Import { manifest, dry_run } => {
                migrate::import(manifest, *dry_run).await?;
            }
        },
    }

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

    Ok(())
}
