use std::path::PathBuf;

use crate::core::paths;

pub(crate) mod eol;

static DATA_DIR: std::sync::LazyLock<PathBuf> = std::sync::LazyLock::new(paths::data_dir);

pub(crate) mod bun;
pub(crate) mod common;
pub(crate) mod go;
pub(crate) mod java;
pub(crate) mod node;
pub(crate) mod pi;
pub(crate) mod python;
pub(crate) mod ruby;
pub(crate) mod rust;

pub(crate) use bun::BunManager;
pub(crate) use go::GoManager;
pub(crate) use java::JavaManager;
pub(crate) use node::NodeManager;
pub(crate) use pi::PiManager;
pub(crate) use python::PythonManager;
pub(crate) use ruby::RubyManager;
pub(crate) use rust::RustManager;

/// Runtimes managed natively by OMG.
pub(crate) const SUPPORTED_RUNTIMES: &[&str] =
    &["node", "python", "go", "rust", "ruby", "java", "bun", "pi"];

/// Resolve a partial version request (`20`, `20.1`) against known release
/// names. Exact and non-numeric requests pass through unchanged. One shared
/// shape so the five managers cannot drift apart.
#[must_use]
pub(crate) fn resolve_version_request(names: &[String], requested: &str) -> String {
    if !common::is_partial_version(requested) {
        return requested.to_owned();
    }
    common::resolve_partial_version(names, requested).unwrap_or_else(|| requested.to_owned())
}

/// Fast probing for active runtime versions. The current symlink must
/// resolve to a real version directory inside the runtime versions tree;
/// missing or external targets are not reported as active.
#[must_use]
pub(crate) fn probe_version(runtime: &str) -> Option<String> {
    common::get_current_version(&DATA_DIR.join("versions").join(runtime))
}
