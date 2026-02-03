//! AUR (Arch User Repository) client with build support
//!
//! This module provides the `AurClient` for searching, building, and installing
//! packages from the Arch User Repository.

mod client;
mod error;
mod utils;

pub use client::{AurClient, AurPackageDetail, search_detailed};
