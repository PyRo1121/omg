//! Smart dependency resolution for AUR packages
//!
//! Parses .SRCINFO and checks which dependencies are already installed
//! to avoid redundant pacman operations.

use alpm_srcinfo::SourceInfoV1;
use alpm_types::SystemArchitecture;
use anyhow::{Context, Result};
use std::path::Path;

/// Parsed dependency information from .SRCINFO
#[derive(Debug, Clone)]
pub struct DependencyInfo {
    /// Dependencies that need to be installed
    pub missing: Vec<String>,
    /// Dependencies already installed
    pub satisfied: Vec<String>,
    /// Total dependency count
    pub total: usize,
}

/// Get the current system architecture
fn current_arch() -> Option<SystemArchitecture> {
    match std::env::consts::ARCH {
        "x86_64" => Some(SystemArchitecture::X86_64),
        "aarch64" => Some(SystemArchitecture::Aarch64),
        "arm" => Some(SystemArchitecture::Arm),
        "i686" => Some(SystemArchitecture::I686),
        _ => None,
    }
}

/// Parse .SRCINFO and check which dependencies are missing
pub fn check_dependencies(pkg_dir: &Path) -> Result<DependencyInfo> {
    let srcinfo_path = pkg_dir.join(".SRCINFO");

    if !srcinfo_path.exists() {
        // No .SRCINFO means we can't pre-check, fallback to makepkg
        return Ok(DependencyInfo {
            missing: Vec::new(),
            satisfied: Vec::new(),
            total: 0,
        });
    }

    let content = std::fs::read_to_string(&srcinfo_path).context("Failed to read .SRCINFO")?;

    let srcinfo = SourceInfoV1::from_string(&content).context("Failed to parse .SRCINFO")?;

    let base = &srcinfo.base;
    let estimated_deps =
        base.dependencies.len() + base.make_dependencies.len() + base.check_dependencies.len();
    let mut all_deps = Vec::with_capacity(estimated_deps + 16);

    // Collect runtime dependencies from base
    for dep in &base.dependencies {
        all_deps.push(dep.to_string());
    }

    // Collect make dependencies from base
    for dep in &base.make_dependencies {
        all_deps.push(dep.name.to_string());
    }

    // Collect check dependencies from base
    for dep in &base.check_dependencies {
        all_deps.push(dep.name.to_string());
    }

    // Also collect architecture-specific dependencies if available
    if let Some(arch) = current_arch()
        && let Some(arch_props) = base.architecture_properties.get(&arch)
    {
        for dep in &arch_props.dependencies {
            all_deps.push(dep.to_string());
        }
        for dep in &arch_props.make_dependencies {
            all_deps.push(dep.name.to_string());
        }
        for dep in &arch_props.check_dependencies {
            all_deps.push(dep.name.to_string());
        }
    }

    // Note: Split packages have Override<Vec> for dependencies which may override
    // or clear base dependencies. For simplicity, we rely on base dependencies
    // which covers the common case. makepkg will handle any edge cases.

    // Remove duplicates
    all_deps.sort();
    all_deps.dedup();

    let total = all_deps.len();

    // Check which ones are installed using alpm
    let (satisfied, missing) = crate::package_managers::alpm_direct::with_handle(|alpm| {
        let localdb = alpm.localdb();
        let mut satisfied = Vec::with_capacity(total);
        let mut missing = Vec::with_capacity(total / 4 + 1);

        for dep in all_deps {
            // Extract package name (strip version constraints like >=, <=, =, >, <)
            let pkg_name = extract_package_name(&dep);

            if localdb.pkg(pkg_name).is_ok() {
                satisfied.push(dep);
            } else {
                missing.push(dep);
            }
        }

        Ok((satisfied, missing))
    })?;

    Ok(DependencyInfo {
        missing,
        satisfied,
        total,
    })
}

/// Extract package name from a dependency string, stripping version constraints
///
/// Handles formats like:
/// - "package" -> "package"
/// - "package>=1.0" -> "package"
/// - "package>1.0" -> "package"
/// - "package=1.0" -> "package"
/// - "package<=1.0" -> "package"
/// - "package<1.0" -> "package"
fn extract_package_name(dep: &str) -> &str {
    // Find the first occurrence of any version operator
    let operators = ['>', '<', '='];

    for (i, c) in dep.char_indices() {
        if operators.contains(&c) {
            return &dep[..i];
        }
    }

    dep
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_package_name() {
        assert_eq!(extract_package_name("gcc"), "gcc");
        assert_eq!(extract_package_name("gcc>=10"), "gcc");
        assert_eq!(extract_package_name("gcc>10"), "gcc");
        assert_eq!(extract_package_name("gcc=10"), "gcc");
        assert_eq!(extract_package_name("gcc<=10"), "gcc");
        assert_eq!(extract_package_name("gcc<10"), "gcc");
        assert_eq!(
            extract_package_name("lib32-gcc-libs>=10.2"),
            "lib32-gcc-libs"
        );
    }
}
