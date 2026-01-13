//! CLI module for OMG
//!
//! Handles command-line argument parsing and command definitions.

mod args;
pub mod commands;
pub mod env;
pub mod packages;
pub mod runtimes;
pub mod security;

pub use args::{Cli, Commands, EnvCommands};
