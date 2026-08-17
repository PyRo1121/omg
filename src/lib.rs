//! # OMG - The Fastest Unified Package Manager
//!
//! This library contains all the shared functionality used by both
//! the `omg` CLI and `omgd` daemon.
//!
//! ## Performance
//! - **Search**: 6ms (22x faster than pacman)
//! - **Info**: 6.5ms (21x faster than pacman)
//! - **Explicit**: 1.2ms (12x faster than pacman)
//!
//!
//! ## Architecture
//! - [`daemon`] - Background daemon with Unix socket IPC
//! - [`cli`] - Command-line interface
//! - [`core`] - Shared types, database, and utilities
//! - [`package_managers`] - Arch (ALPM) and Debian (apt) backends
//! - [`runtimes`] - Node, Python, Rust, Go, Ruby, Java, Bun version managers

#![allow(clippy::missing_const_for_fn)] // Many of these can't actually be const

pub mod cli;
pub mod config;
pub mod core;
#[cfg(unix)]
pub mod daemon;
pub mod hooks;
pub mod package_managers;
pub mod runtimes;
pub mod shims;
