//! OMG Library - Shared code for CLI and daemon
//!
//! This library contains all the shared functionality used by both
//! the `omg` CLI and `omgd` daemon.

pub mod cli;
pub mod config;
pub mod core;
pub mod daemon;
pub mod hooks;
pub mod package_managers;
pub mod runtimes;
pub mod shims;
