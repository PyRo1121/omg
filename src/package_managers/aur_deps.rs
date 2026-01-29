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
    /// Dependencies already installed
    pub satisfied: Vec<String>,
    /// Total dependency count
    pub total: usize,
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

    // Get base package info from srcinfo
    let base = &srcinfo.base;
    let mut all_deps = Vec::new();

    // Collect runtime dependencies (RelationOrSoname type - use to_string())
    for dep in &base.dependencies {
        all_deps.push(dep.to_string());
    }

    // Collect make dependencies (PackageRelation type - use name field)
    for dep in &base.make_dependencies {
        all_deps.push(dep.name.to_string());
    }

    // Collect check dependencies (PackageRelation type - use name field)
    for dep in &base.check_dependencies {
        all_deps.push(dep.name.to_string());
    }

    // Remove duplicates
    all_deps.sort();
    all_deps.dedup();

    let total = all_deps.len();

    // Check which ones are installed using alpm
    let (satisfied, missing) = crate::package_managers::alpm_direct::with_handle(|alpm| {
        let localdb = alpm.localdb();
        let mut satisfied = Vec::new();
        let mut missing = Vec::new();

        for dep in &all_deps {
            // Extract package name (strip version constraints)
            let pkg_name: &str = dep.split(['>', '<', '=']).next().unwrap_or(dep);

            if localdb.pkg(pkg_name).is_ok() {
                satisfied.push(dep.clone());
            } else {
                missing.push(dep.clone());
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
