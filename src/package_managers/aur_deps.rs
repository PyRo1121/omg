//! Smart dependency resolution for AUR packages
//!
//! Parses .SRCINFO and checks which dependencies are already installed
//! to avoid redundant pacman operations.

use alpm_srcinfo::SourceInfoV1;
use anyhow::{Context, Result};
use std::path::Path;

/// Parsed dependency information from .SRCINFO
#[derive(Debug, Clone)]
pub struct DependencyInfo {
    /// Dependencies that need to be installed
    pub missing: Vec<String>,
    /// Total dependency expressions inspected
    pub total: usize,
}

/// Parse .SRCINFO and check which dependencies are missing
pub fn check_dependencies(pkg_dir: &Path) -> Result<DependencyInfo> {
    let srcinfo_path = pkg_dir.join(".SRCINFO");

    if !srcinfo_path.exists() {
        // No .SRCINFO means we can't pre-check, fallback to makepkg
        return Ok(DependencyInfo {
            missing: Vec::new(),
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
    if let Some(arch) = super::aur::utils::current_arch()
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

    let dependency_count = all_deps.len();

    // Ask libalpm to evaluate the complete dependency expression. A package
    // with the right name but an older version is not a satisfier, while a
    // compatible virtual provider can be.
    let missing = crate::package_managers::alpm_direct::with_handle(|alpm| {
        let installed = alpm.localdb().pkgs();
        Ok(all_deps
            .into_iter()
            .filter(|dependency| installed.find_satisfier(dependency.clone()).is_none())
            .collect::<Vec<_>>())
    })?;

    debug_assert!(missing.len() <= dependency_count);
    Ok(DependencyInfo {
        missing,
        total: dependency_count,
    })
}
