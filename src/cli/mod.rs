//! CLI module for OMG
//!
//! Handles command-line argument parsing and command definitions.

use anyhow::Result;
use async_trait::async_trait;

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

/// A trait for modular CLI command execution
#[async_trait]
pub trait CommandRunner {
    /// Execute the command
    async fn execute(&self, ctx: &CliContext) -> Result<()>;
}

#[async_trait]
impl CommandRunner for Commands {
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
