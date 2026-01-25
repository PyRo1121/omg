//! Test infrastructure and utilities for TDD
//!
//! This module provides:
//! - Fixture builders for common test scenarios
//! - Dependency injection abstractions
//! - Test helpers and utilities
//! - Mock implementations

pub mod fixtures;
pub mod helpers;
pub mod mocks;

// Re-export commonly used test utilities
pub use fixtures::*;
pub use helpers::*;
pub use mocks::*;
