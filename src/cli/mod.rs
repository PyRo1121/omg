//! CLI module for OMG
//!
//! Handles command-line argument parsing and command definitions.

// trait_variant macro generates Send bounds correctly but clippy can't see through the expansion
#![expect(clippy::future_not_send)]

use anyhow::Result;

mod args;
pub mod blame;
pub mod ci;
pub mod commands;
pub mod components;
pub mod config;
pub mod container;
pub mod daemon_status;
pub mod diff;
pub mod doctor;
pub mod enterprise;
pub mod env;
pub mod fleet;
pub mod git_hooks;
pub mod help;
pub mod init;
#[cfg(feature = "license")]
pub mod license;
pub mod man;
pub mod migrate;
pub mod modern_ui;
pub mod new;
pub mod outdated;
pub mod packages;
pub mod run;
pub mod runtimes;
pub mod security;
pub mod self_update;
pub mod size;
pub mod snapshot;
pub mod style;
pub mod tea;
pub mod team;
pub mod telemetry;
pub mod tool;
pub mod tui;
pub mod ui;
pub mod why;
pub mod workspace;

#[cfg(feature = "license")]
pub use args::LicenseCommands;
pub use args::{
    AuditCommands, CiCommands, Cli, Commands, ConfigCommands, ContainerCommands,
    EnterpriseCommands, EnterprisePolicyCommands, EnvCommands, FleetCommands, GoldenPathCommands,
    HooksCommands, MigrateCommands, PrivacyCommands, ServerCommands, ShellKind, SnapshotCommands,
    TeamCommands, TeamRoleCommands, ToolCommands, TransactionTypeFilter, WorkspaceCommands,
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
#[allow(
    clippy::future_not_send,
    reason = "trait_variant generates the Send-bounded public variant"
)]
pub trait LocalCommandRunner {
    /// Execute the command
    async fn execute(&self, ctx: &CliContext) -> Result<()>;
}
