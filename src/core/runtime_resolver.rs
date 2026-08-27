//! Runtime Resolution and Path Management
//!
//! Shared utilities for resolving native runtime binary paths. Used by both
//! the task runner and hooks system.
//!
//! This module eliminates ~150 lines of duplication between `task_runner.rs` and `hooks/mod.rs`

use std::path::PathBuf;

use super::paths;

/// Resolve the binary directory path for a native OMG-managed runtime
///
/// Returns `Some(path)` if the runtime version is installed natively, `None` otherwise.
///
/// Supported runtimes: node, python, go, ruby, java, bun, rust
pub fn native_runtime_bin_path(runtime: &str, version: &str) -> Option<PathBuf> {
    let versions_dir = paths::data_dir().join("versions");
    let bin_path = match runtime {
        "node" | "python" | "go" | "ruby" | "java" | "pi" => {
            versions_dir.join(runtime).join(version).join("bin")
        }
        "bun" => versions_dir.join("bun").join(version),
        "rust" => {
            let toolchain = crate::runtimes::rust::RustToolchainSpec::parse(version)
                .ok()?
                .name();
            versions_dir.join("rust").join(toolchain).join("bin")
        }
        _ => return None,
    };

    crate::runtimes::common::is_valid_version_dir(&bin_path).then_some(bin_path)
}

/// Find a binary in the system PATH
///
/// Returns the full path to the binary if found, `None` otherwise.
pub fn find_in_path(binary: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_runtime_bin_path_returns_none_for_unknown() {
        assert!(native_runtime_bin_path("unknown-runtime", "1.0.0").is_none());
    }

    #[test]
    fn native_runtime_bin_path_rejects_a_regular_file_impostor() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let bin_path = temp.path().join("versions/node/20.0.0/bin");
        std::fs::create_dir_all(bin_path.parent().expect("parent")).expect("create version dir");
        std::fs::write(&bin_path, b"not a directory").expect("write impostor");
        // The helper uses the process data dir, so this only documents the
        // local predicate used by PATH resolution.
        assert!(!crate::runtimes::common::is_valid_version_dir(&bin_path));
    }

    #[test]
    fn rust_native_path_uses_omg_toolchain_layout() {
        let parsed = crate::runtimes::rust::RustToolchainSpec::parse("stable")
            .expect("stable toolchain spec");
        let expected = crate::core::paths::data_dir()
            .join("versions/rust")
            .join(parsed.name())
            .join("bin");
        let resolved = native_runtime_bin_path("rust", "stable");
        if crate::runtimes::common::is_valid_version_dir(&expected) {
            assert_eq!(resolved.as_deref(), Some(expected.as_path()));
        } else {
            assert!(resolved.is_none());
        }
    }

    #[test]
    fn test_find_in_path_finds_known_binaries() {
        // sh should exist on all Unix systems
        #[cfg(unix)]
        assert!(find_in_path("sh").is_some());
    }

    #[test]
    fn test_find_in_path_returns_none_for_nonexistent() {
        assert!(find_in_path("this-binary-definitely-does-not-exist-12345").is_none());
    }
}
