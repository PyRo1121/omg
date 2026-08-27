//! Runtime Resolution and Path Management
//!
//! Shared utilities for resolving runtime binary paths across native installations
//! and mise-managed toolchains. Used by both task runner and hooks system.
//!
//! This module eliminates ~150 lines of duplication between `task_runner.rs` and `hooks/mod.rs`

use std::collections::{HashMap, HashSet};
use std::hash::BuildHasher;
use std::path::PathBuf;
use std::process::Command;

use super::{RuntimeBackend, paths};

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

/// Check if mise is available on the system
#[inline]
#[must_use]
pub fn mise_available() -> bool {
    find_in_path("mise").is_some()
}

/// Resolve the binary directory path for a mise-managed runtime
///
/// Queries mise using `mise where` to find the installation directory.
/// Returns `Some(path)` if mise has the runtime installed, `None` otherwise.
pub fn mise_runtime_bin_path(runtime: &str, version: &str) -> Option<PathBuf> {
    let tool_spec = if version.is_empty() {
        runtime.to_string()
    } else {
        format!("{runtime}@{version}")
    };

    let output = Command::new("mise")
        .args(["where", "--", &tool_spec])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let install_dir = PathBuf::from(stdout.trim());
    if install_dir.as_os_str().is_empty() {
        return None;
    }

    let bin_dir = install_dir.join("bin");
    if crate::runtimes::common::is_valid_version_dir(&bin_dir) {
        Some(bin_dir)
    } else if crate::runtimes::common::is_valid_version_dir(&install_dir) {
        Some(install_dir)
    } else {
        None
    }
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

/// Add mise runtime paths to `path_additions`, respecting backend preference
///
/// This function adds bin directories for mise-managed runtimes to the PATH additions vector.
/// It deduplicates paths and respects the backend preference (native-first or mise-only).
pub fn add_mise_path_fallbacks<S: BuildHasher>(
    versions: &HashMap<String, String, S>,
    path_additions: &mut Vec<String>,
    backend: RuntimeBackend,
) {
    if !matches!(
        backend,
        RuntimeBackend::Mise | RuntimeBackend::NativeThenMise
    ) {
        return;
    }

    if !mise_available() {
        return;
    }

    let mut seen: HashSet<String> = path_additions.iter().cloned().collect();
    for (runtime, version) in versions {
        // Skip if native runtime is preferred and available
        if backend == RuntimeBackend::NativeThenMise
            && native_runtime_bin_path(runtime, version).is_some()
        {
            continue;
        }

        if let Some(bin_dir) = mise_runtime_bin_path(runtime, version) {
            let bin = bin_dir.display().to_string();
            if seen.insert(bin.clone()) {
                path_additions.push(bin);
            }
        }
    }
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

    #[test]
    fn test_add_mise_path_fallbacks_respects_backend() {
        let versions = HashMap::new();
        let mut path_additions = Vec::new();

        // Should not add anything with Native backend
        add_mise_path_fallbacks(&versions, &mut path_additions, RuntimeBackend::Native);
        assert!(path_additions.is_empty());
    }
}
