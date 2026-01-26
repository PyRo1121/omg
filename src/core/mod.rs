//! Core module - shared types, database, and utilities

pub mod analytics;
pub mod archive;
pub mod client;
pub mod completion;
pub mod container;
mod database;
pub mod env;
pub mod error;
pub mod fast_status;
pub mod history;
pub mod http;
pub mod license;
pub mod metrics;
pub mod packages;
#[cfg(feature = "arch")]
pub mod pacman_conf;
pub mod paths;
pub mod privilege;
pub mod safe_ops;
pub mod security;
pub mod sysinfo;
pub mod task_runner;
pub mod telemetry;
pub mod testing;
mod types;
pub mod usage;

pub use archive::{
    extract_auto, extract_auto_strip, extract_tar_gz, extract_tar_gz_strip, extract_zip,
    extract_zip_strip,
};
pub use database::Database;
pub use error::{OmgError, Result};
pub use privilege::{elevate_if_needed, get_yes_flag, is_root, set_yes_flag};
pub use types::*;
