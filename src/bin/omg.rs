//! OMG CLI Binary
//!
//! The main command-line interface for OMG package manager.

use anyhow::Result;
use clap::Parser;

use omg_lib::cli::doctor;
use omg_lib::cli::env;
use omg_lib::cli::new;
use omg_lib::cli::packages;
use omg_lib::cli::runtimes;
use omg_lib::cli::security;
use omg_lib::cli::tool;
use omg_lib::cli::{commands, Cli, Commands, EnvCommands, ToolCommands};
use omg_lib::core::{elevate_if_needed, is_root, task_runner};
use omg_lib::hooks;

#[cfg(not(target_env = "msvc"))]
use mimalloc::MiMalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[allow(clippy::too_many_lines)]
fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_target(false)
        .init();

    let cli = Cli::parse();

    // Commands that require root - auto-elevate with sudo
    // Note: Install/Remove/Update handle elevation internally via subprocess
    // to allow user-level operations (like AUR building) to persist.
    let needs_root = matches!(&cli.command, Commands::Sync | Commands::Clean { .. });

    if needs_root && !is_root() {
        // Re-execute with sudo - this replaces the current process
        elevate_if_needed()?;
        // Never reaches here
    }

    // Handle commands
    match cli.command {
        Commands::Search {
            query,
            detailed,
            interactive,
        } => {
            // PURE SYNC PATH (Sub-10ms)
            if !packages::search_sync_cli(&query, detailed, interactive)? {
                // Fallback to async if needed
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(packages::search(&query, detailed, interactive))?;
            }
        }
        Commands::Install { packages, yes } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(packages::install(&packages, yes))?;
        }
        Commands::Remove {
            packages: pkgs,
            recursive,
        } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(packages::remove(&pkgs, recursive))?;
        }
        Commands::Update { check } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(packages::update(check))?;
        }
        Commands::Info { package } => {
            // 1. Try SYNC PATH (Official + Local)
            if !packages::info_sync(&package)? {
                // 2. Fallback to ASYNC PATH (AUR)
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(packages::info_aur(&package))?;
            }
        }
        Commands::Clean {
            orphans,
            cache,
            aur,
            all,
        } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(packages::clean(orphans, cache, aur, all))?;
        }
        Commands::Explicit { count } => {
            // PURE SYNC PATH (Sub-10ms)
            packages::explicit_sync(count)?;
        }
        Commands::Sync => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(packages::sync())?;
        }
        Commands::Use { runtime, version } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(runtimes::use_version(&runtime, version.as_deref()))?;
        }
        Commands::List { runtime, available } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(runtimes::list_versions(runtime.as_deref(), available))?;
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
            let versions = hooks::get_active_versions();
            if let Some(version) = versions.get(&runtime.to_lowercase()) {
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
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(commands::complete(&shell, &current, &last, full.as_deref()))?;
        }
        Commands::Status => {
            // PURE SYNC PATH (Sub-10ms)
            commands::status_sync()?;
        }
        Commands::Doctor => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(doctor::run())?;
        }
        Commands::Audit => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(security::audit())?;
        }
        Commands::Run { task, args } => {
            task_runner::run_task(&task, &args)?;
        }
        Commands::New { stack, name } => {
            new::run(&stack, &name)?;
        }
        Commands::Tool { command } => match command {
            ToolCommands::Install { name } => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(tool::install(&name))?;
            }
            ToolCommands::List => {
                tool::list()?;
            }
            ToolCommands::Remove { name } => {
                tool::remove(&name)?;
            }
        },
        Commands::Env { command } => match command {
            EnvCommands::Capture => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(env::capture())?;
            }
            EnvCommands::Check => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(env::check())?;
            }
            EnvCommands::Share {
                description,
                public,
            } => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(env::share(description, public))?;
            }
            EnvCommands::Sync { url } => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(env::sync(url))?;
            }
        },
        Commands::History { limit } => {
            commands::history(limit)?;
        }
        Commands::Rollback { id } => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(commands::rollback(id))?;
        }
        Commands::Dash => {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()?;
            rt.block_on(omg_lib::cli::tui::run())?;
        }
    }

    Ok(())
}
