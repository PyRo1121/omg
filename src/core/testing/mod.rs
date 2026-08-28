//! Test infrastructure and utilities for TDD
//!
//! This module provides fixture builders and a package-manager test double.

pub mod fixtures;
pub mod mocks;

pub use fixtures::{PackageFixture, UpdateFixture};
pub use mocks::TestPackageManager;
