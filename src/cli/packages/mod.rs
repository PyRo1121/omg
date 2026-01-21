//! Package management CLI operations
//!
//! This module provides all package-related CLI functionality:
//! - Search: Find packages in repositories and AUR
//! - Install: Install packages with security grading
//! - Remove: Uninstall packages
//! - Update: System-wide package updates
//! - Info: Display package information
//! - Clean: Remove orphans and clear caches
//! - Explicit: List explicitly installed packages
//! - Sync: Synchronize package databases

mod clean;
mod common;
mod explicit;
mod info;
mod install;
pub mod local;
mod remove;
mod search;
mod status;
mod sync_db;
mod update;

// Re-export all public functions
pub use clean::clean;
pub use explicit::{explicit, explicit_sync};
pub use info::{info, info_aur, info_sync, info_sync_cli};
pub use install::install;
pub use remove::remove;
pub use search::{search, search_sync_cli};
pub use status::status;
pub use sync_db::sync_databases as sync;
pub use update::update;
