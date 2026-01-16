//! OMG CLI Binary
//!
//! The main command-line interface for OMG package manager.
//!
//! Uses a single tokio runtime for all async operations (Rust 2024 best practice).

use anyhow::Result;
use clap::Parser;

use omg_lib::cli::doctor;
use omg_lib::cli::env;
use omg_lib::cli::new;
use omg_lib::cli::packages;
use omg_lib::cli::runtimes;
use omg_lib::cli::security;
use omg_lib::cli::tool;
use omg_lib::cli::{Cli, Commands, EnvCommands, ToolCommands, commands};
use omg_lib::core::{RuntimeBackend, elevate_if_needed, is_root, task_runner};
use omg_lib::hooks;

// Using system allocator (pure Rust - no C dependency)

/// Main entry point - uses single tokio runtime for all async operations
/// This eliminates the overhead of creating a new runtime for each command
#[tokio::main(flavor = "multi_thread")]
#[allow(clippy::too_many_lines)]
async fn main() -> Result<()> {
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
        } => {
            packages::remove(&pkgs, recursive).await?;
        }
        Commands::Update { check } => {
            packages::update(check).await?;
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
        Commands::Status => {
            // PURE SYNC PATH (Sub-10ms)
            commands::status_sync()?;
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
        } => {
            let backend = runtime_backend
                .as_deref()
                .map(str::parse::<RuntimeBackend>)
                .transpose()
                .map_err(|err| anyhow::anyhow!(err))?;
            task_runner::run_task(&task, &args, backend)?;
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
        Commands::History { limit } => {
            commands::history(limit)?;
        }
        Commands::Rollback { id } => {
            commands::rollback(id).await?;
        }
        Commands::Dash => {
            omg_lib::cli::tui::run().await?;
        }
    }

    Ok(())
}
