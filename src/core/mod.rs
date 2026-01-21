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
pub mod paths;
pub mod privilege;
pub mod security;
pub mod sysinfo;
pub mod task_runner;
pub mod telemetry;
mod types;
pub mod usage;
pub mod validation;

pub use archive::{
    extract_auto, extract_auto_strip, extract_tar_gz, extract_tar_gz_strip, extract_zip,
    extract_zip_strip,
};
pub use database::Database;
pub use error::{OmgError, Result};
pub use privilege::{elevate_if_needed, is_root};
pub use types::*;
