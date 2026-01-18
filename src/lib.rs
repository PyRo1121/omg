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
//! ## Architecture
//! - [`daemon`] - Background daemon with Unix socket IPC
//! - [`cli`] - Command-line interface
//! - [`core`] - Shared types, database, and utilities
//! - [`package_managers`] - Arch (ALPM) and Debian (apt) backends
//! - [`runtimes`] - Node, Python, Rust, Go, Ruby, Java, Bun version managers

// Production-ready clippy configuration
#![warn(clippy::pedantic)]
#![warn(clippy::perf)]
#![warn(clippy::suspicious)]
// Allow documentation lints - internal code, not public API
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::must_use_candidate)]
// Allow some pedantic lints that are too strict for this codebase
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::similar_names)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::significant_drop_tightening)]
// Allow pedantic lints that are not critical
#![allow(clippy::type_complexity)]

pub mod cli;
pub mod config;
pub mod core;
pub mod daemon;
pub mod hooks;
pub mod package_managers;
pub mod runtimes;
pub mod shims;
