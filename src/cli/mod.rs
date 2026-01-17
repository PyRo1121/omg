//! CLI module for OMG
//!
//! Handles command-line argument parsing and command definitions.

mod args;
pub mod commands;
pub mod container;
pub mod doctor;
pub mod env;
pub mod license;
pub mod new;
pub mod packages;
pub mod runtimes;
pub mod security;
pub mod style;
pub mod team;
pub mod tool;
pub mod tui;

pub use args::{
    AuditCommands, Cli, Commands, ContainerCommands, EnvCommands, LicenseCommands, TeamCommands,
    ToolCommands,
};
