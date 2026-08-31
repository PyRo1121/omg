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

    let all_deps = dependency_expressions(&srcinfo);
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

fn dependency_expressions(srcinfo: &SourceInfoV1) -> Vec<String> {
    let base = &srcinfo.base;
    let estimated_deps =
        base.dependencies.len() + base.make_dependencies.len() + base.check_dependencies.len();
    let mut dependencies = Vec::with_capacity(estimated_deps + 16);

    dependencies.extend(base.dependencies.iter().map(ToString::to_string));
    dependencies.extend(base.make_dependencies.iter().map(ToString::to_string));
    dependencies.extend(base.check_dependencies.iter().map(ToString::to_string));

    if let Some(arch) = super::aur::utils::current_arch()
        && let Some(arch_props) = base.architecture_properties.get(&arch)
    {
        dependencies.extend(arch_props.dependencies.iter().map(ToString::to_string));
        dependencies.extend(arch_props.make_dependencies.iter().map(ToString::to_string));
        dependencies.extend(
            arch_props
                .check_dependencies
                .iter()
                .map(ToString::to_string),
        );
    }

    // Split packages have Override<Vec> dependencies which may replace or
    // clear base dependencies. makepkg remains the final authority for those
    // package-specific overrides.
    dependencies.sort();
    dependencies.dedup();
    dependencies
}

#[cfg(test)]
mod tests {
    use super::{SourceInfoV1, dependency_expressions};

    #[test]
    fn preserves_version_constraints_for_all_dependency_kinds() {
        let srcinfo = SourceInfoV1::from_string(
            "pkgbase = example\n\
             \tpkgdesc = dependency constraint fixture\n\
             \tpkgver = 1.0.0\n\
             \tpkgrel = 1\n\
             \tarch = any\n\
             \tdepends = runtime>=1.2\n\
             \tmakedepends = compiler=3.4\n\
             \tcheckdepends = test-runner<5\n\
             \n\
             pkgname = example\n",
        )
        .expect("valid .SRCINFO fixture");

        assert_eq!(
            dependency_expressions(&srcinfo),
            ["compiler=3.4", "runtime>=1.2", "test-runner<5"]
        );
    }
}
