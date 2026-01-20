//! CLI module for OMG
//!
//! Handles command-line argument parsing and command definitions.

mod args;
pub mod blame;
pub mod ci;
pub mod commands;
pub mod container;
pub mod diff;
pub mod doctor;
pub mod enterprise;
pub mod env;
pub mod fleet;
pub mod init;
#[cfg(feature = "license")]
pub mod license;
pub mod migrate;
pub mod new;
pub mod outdated;
pub mod packages;
pub mod pin;
pub mod runtimes;
pub mod security;
pub mod size;
pub mod snapshot;
pub mod style;
pub mod team;
pub mod tool;
pub mod tui;
pub mod why;

pub use args::{
    AuditCommands, CiCommands, Cli, Commands, ContainerCommands, EnterpriseCommands,
    EnterprisePolicyCommands, EnvCommands, FleetCommands, GoldenPathCommands, MigrateCommands,
    NotifyCommands, ServerCommands, SnapshotCommands, TeamCommands, TeamRoleCommands, ToolCommands,
};
#[cfg(feature = "license")]
pub use args::LicenseCommands;
