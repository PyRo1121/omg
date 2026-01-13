//! OMG CLI Binary
//!
//! The main command-line interface for OMG package manager.

use anyhow::Result;
use clap::Parser;

use omg_lib::cli::env;
use omg_lib::cli::packages;
use omg_lib::cli::runtimes;
use omg_lib::cli::security;
use omg_lib::cli::{commands, Cli, Commands, EnvCommands};
use omg_lib::hooks;

#[tokio::main]
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

    // Handle commands
    match cli.command {
        Commands::Search { query, detailed } => {
            packages::search(&query, detailed).await?;
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
            packages::info(&package).await?;
        }
        Commands::Clean {
            orphans,
            cache,
            aur,
            all,
        } => {
            packages::clean(orphans, cache, aur, all).await?;
        }
        Commands::Explicit => {
            packages::explicit().await?;
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
            commands::daemon(foreground).await?;
        }
        Commands::Config { key, value } => {
            commands::config(key.as_deref(), value.as_deref()).await?;
        }
        Commands::Completions { shell } => {
            hooks::completions::generate_completions(&shell)?;
        }
        Commands::Which { runtime } => {
            let versions = hooks::get_active_versions();
            if let Some(version) = versions.get(&runtime.to_lowercase()) {
                println!("{} {}", runtime, version);
            } else {
                println!(
                    "{}: no version set (check .tool-versions, .nvmrc, etc.)",
                    runtime
                );
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
            commands::status().await?;
        }
        Commands::Audit => {
            security::audit().await?;
        }
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
    }

    Ok(())
}
