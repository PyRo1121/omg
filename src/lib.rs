//! OMG Library - Shared code for CLI and daemon
//!
//! This library contains all the shared functionality used by both
//! the `omg` CLI and `omgd` daemon.

// Production-ready clippy configuration
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
// Allow documentation lints - internal code, not public API
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::must_use_candidate)]
// Allow some pedantic lints that are too strict for this codebase
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::similar_names)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::significant_drop_tightening)]
// Allow style lints that don't affect correctness
#![allow(clippy::redundant_else)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::unused_async)]
#![allow(clippy::unused_self)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::future_not_send)]
// Allow nursery lints that are noisy
#![allow(clippy::option_if_let_else)]
#![allow(clippy::redundant_pub_crate)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::or_fun_call)]
// Allow pedantic lints that are not critical
#![allow(clippy::case_sensitive_file_extension_comparisons)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::double_ended_iterator_last)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::type_complexity)]
#![allow(clippy::derive_partial_eq_without_eq)]

pub mod cli;
pub mod config;
pub mod core;
pub mod daemon;
pub mod hooks;
pub mod package_managers;
pub mod runtimes;
pub mod shims;
