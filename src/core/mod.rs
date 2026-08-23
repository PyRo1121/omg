//! Core module - shared types, database, and utilities

pub mod analytics;
pub(crate) mod archive;
pub mod caps;
#[cfg(unix)]
pub mod client;
pub mod completion;
pub mod container;
mod database;
pub mod env;
pub mod error;
pub mod fast_status;
pub mod format;
pub mod history;
pub mod http;
pub mod license;
pub mod metrics;
pub mod packages;
#[cfg(feature = "arch")]
pub mod pacman_conf;
pub mod paths;
pub mod privilege;
pub mod runtime_resolver;
pub mod safe_ops;
pub mod security;
#[cfg(unix)]
pub mod sudoloop;
pub mod sysinfo;
pub mod task_runner;
pub mod telemetry;
pub mod telemetry_client;
/// Test fixtures, helpers, and mocks. Compiled only for tests and debug
/// builds so release binaries do not ship test infrastructure.
#[cfg(any(test, debug_assertions))]
pub mod testing;
mod types;
pub mod usage;

pub use caps::{can_write_pacman_db, has_package_caps, is_elevated, maybe_show_turbo_hint};
pub use database::Database;
pub use privilege::{is_root, set_yes_flag};
// Explicit names only (no globs): one discoverable path per concept.
// Canonical paths: `core::types` items live at `core::{Package, PackageSource,
// RuntimeBackend}`; caps predicates stay at `core::caps::*`; the typed error
// contract lives at `core::error::OmgError` (unused — pending deletion, see
// wave-4 HANDOFF).
pub use types::{Package, PackageSource, RuntimeBackend};
