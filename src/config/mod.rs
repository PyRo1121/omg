//! Configuration module for OMG

pub(crate) mod mise_env;
mod settings;

pub use settings::{AurBuildMethod, Settings};

pub(crate) use settings::{validate_build_concurrency, validate_makeflags};
