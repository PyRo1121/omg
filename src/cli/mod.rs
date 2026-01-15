//! CLI module for OMG
//!
//! Handles command-line argument parsing and command definitions.

mod args;
pub mod commands;
pub mod doctor;
pub mod env;
pub mod new;
pub mod packages;
pub mod runtimes;
pub mod security;
pub mod style;
pub mod tool;
pub mod tui;

pub use args::{Cli, Commands, EnvCommands, ToolCommands};
