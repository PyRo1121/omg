use std::path::PathBuf;

pub static DATA_DIR: std::sync::LazyLock<PathBuf> = std::sync::LazyLock::new(|| {
    directories::ProjectDirs::from("com", "omg", "omg")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| {
            home::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".omg")
        })
});

pub mod bun;
pub mod go;
pub mod java;
mod manager;
pub mod node;
pub mod python;
pub mod ruby;
pub mod rust;

pub use bun::BunManager;
pub use go::GoManager;
pub use java::JavaManager;
pub use manager::RuntimeManager;
pub use node::NodeManager;
pub use python::PythonManager;
pub use ruby::RubyManager;
pub use rust::RustManager;

pub const SUPPORTED_RUNTIMES: &[&str] = &["node", "python", "go", "rust", "ruby", "java", "bun"];

/// Fast, zero-allocation probing for active runtime versions
pub fn probe_version(runtime: &str) -> Option<String> {
    let current_link = DATA_DIR.join("versions").join(runtime).join("current");

    std::fs::read_link(&current_link)
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
}
