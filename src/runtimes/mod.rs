use std::path::PathBuf;

use crate::core::paths;

pub static DATA_DIR: std::sync::LazyLock<PathBuf> = std::sync::LazyLock::new(paths::data_dir);

pub mod bun;
pub mod common;
pub mod go;
pub mod java;
pub mod mise;
pub mod node;
pub mod python;
pub mod ruby;
pub mod rust;

pub use bun::BunManager;
pub use go::GoManager;
pub use java::JavaManager;
pub use mise::MiseManager;
pub use node::NodeManager;
pub use python::PythonManager;
pub use ruby::RubyManager;
pub use rust::RustManager;

pub const SUPPORTED_RUNTIMES: &[&str] = &["node", "python", "go", "rust", "ruby", "java", "bun"];

/// Fast probing for active runtime versions. The current symlink must
/// resolve to a real version directory inside the runtime versions tree;
/// missing or external targets are not reported as active.
pub fn probe_version(runtime: &str) -> Option<String> {
    common::get_current_version(&DATA_DIR.join("versions").join(runtime))
}
