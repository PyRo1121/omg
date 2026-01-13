//! Optional shim system for IDE compatibility
//!
//! Shims are disabled by default - PATH modification is faster.
//! Enable with: omg config shims.enabled true

mod generator;

pub use generator::generate_shims;
