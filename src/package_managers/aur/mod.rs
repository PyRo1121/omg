//! AUR (Arch User Repository) client with build support
//!
//! This module provides the `AurClient` for searching, building, and installing
//! packages from the Arch User Repository.

mod client;
mod error;
pub mod parallel_build;
mod utils;

pub use client::{AurClient, AurPackageDetail, search_detailed};
pub use parallel_build::{BuildJob, ParallelBuilder};
