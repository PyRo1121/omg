//! CLI module for OMG
//!
//! Handles command-line argument parsing and command definitions.

// trait_variant macro generates Send bounds correctly but clippy can't see through the expansion
#![allow(clippy::future_not_send)]

use anyhow::Result;

mod args;
pub mod blame;
pub mod ci;
pub mod commands;
pub mod components;
pub mod container;
pub mod diff;
pub mod doctor;
pub mod enterprise;
pub mod env;
pub mod fleet;
pub mod help;
pub mod init;
pub mod json_output;
#[cfg(feature = "license")]
pub mod license;
pub mod migrate;
pub mod new;
pub mod outdated;
pub mod packages;
pub mod pin;
pub mod run;
pub mod runtimes;
pub mod security;
pub mod self_update;
pub mod size;
pub mod snapshot;
pub mod style;
pub mod tables;
pub mod tea;
pub mod team;
pub mod tool;
pub mod tui;
pub mod ui;
pub mod why;

#[cfg(feature = "license")]
pub use args::LicenseCommands;
pub use args::{
    AuditCommands, CiCommands, Cli, Commands, ContainerCommands, EnterpriseCommands,
    EnterprisePolicyCommands, EnvCommands, FleetCommands, GoldenPathCommands, MigrateCommands,
    NotifyCommands, ServerCommands, SnapshotCommands, TeamCommands, TeamRoleCommands, ToolCommands,
};

/// Global context for CLI command execution
pub struct CliContext {
    pub verbose: u8,
    pub json: bool,
    pub quiet: bool,
    pub no_color: bool,
}

/// A trait for modular CLI command execution with Send bounds
///
/// Uses `trait_variant` to generate Send-bounded async trait for multi-threaded execution.
/// This is the 2026 best practice for async traits with tokio multi-threaded runtime.
///
/// The macro generates:
/// - `CommandRunner`: Send-bounded variant for multi-threaded executors (default)
/// - `LocalCommandRunner`: Non-Send variant for single-threaded executors
#[trait_variant::make(CommandRunner: Send)]
#[allow(clippy::future_not_send)] // False positive: trait_variant macro generates Send bounds
pub trait LocalCommandRunner {
    /// Execute the command
    async fn execute(&self, ctx: &CliContext) -> Result<()>;
}

impl LocalCommandRunner for Commands {
    async fn execute(&self, ctx: &CliContext) -> Result<()> {
        match self {
            Commands::Env { command } => command.execute(ctx).await,
            Commands::Tool { command } => command.execute(ctx).await,
            Commands::Fleet { command } => command.execute(ctx).await,
            Commands::Team { command } => command.execute(ctx).await,
            Commands::Container { command } => command.execute(ctx).await,
            Commands::Enterprise { command } => command.execute(ctx).await,
            Commands::Run {
                task,
                args,
                runtime_backend,
                watch,
                parallel,
                using,
                all,
            } => {
                let run_cmd = run::RunCommand {
                    task: task.clone(),
                    args: args.clone(),
                    runtime_backend: runtime_backend.clone(),
                    watch: *watch,
                    parallel: *parallel,
                    using: using.clone(),
                    all: *all,
                };
                run_cmd.execute(ctx).await
            }
            _ => anyhow::bail!("Command not yet implemented via CommandRunner"),
        }
    }
}
