//! Core module - shared types, database, and utilities

pub mod archive;
pub mod client;
pub mod completion;
mod database;
pub mod env;
mod error;
pub mod security;
mod types;

pub use archive::{
    extract_auto, extract_auto_strip, extract_tar_gz, extract_tar_gz_strip, extract_zip,
    extract_zip_strip,
};
pub use database::Database;
pub use error::{OmgError, Result};
pub use types::*;
