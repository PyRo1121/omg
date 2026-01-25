//! `omg why` - Explain why a package is installed (dependency chain)

use anyhow::Result;
#[cfg(feature = "arch")]
use std::collections::{HashMap, HashSet, VecDeque};

use crate::cli::tea::Cmd;

/// Explain why a package is installed
pub fn run(package: &str, reverse: bool) -> Result<()> {
    // SECURITY: Validate package name
    crate::core::security::validate_package_name(package)?;

    #[cfg(feature = "arch")]
    {
        let cmd = if reverse {
            show_reverse_deps(package)?
        } else {
            show_dependency_chain(package)?
        };
        crate::cli::packages::execute_cmd(cmd);
        Ok(())
    }

    #[cfg(all(feature = "debian", not(feature = "arch")))]
    {
        let cmd = if reverse {
            show_reverse_deps_debian(package)?
        } else {
            show_dependency_chain_debian(package)?
        };
        crate::cli::packages::execute_cmd(cmd);
        Ok(())
    }

    #[cfg(not(any(feature = "arch", feature = "debian")))]
    {
        let _ = reverse;
        anyhow::bail!("Package dependency analysis requires arch or debian feature");
    }
}

#[cfg(feature = "arch")]
fn show_dependency_chain(package: &str) -> Result<Cmd<()>> {
    use alpm::Alpm;
    use crate::cli::components::Components;

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();

    // Check if package is installed
    let Ok(pkg) = localdb.pkg(package) else {
        return Ok(Components::error_with_suggestion(
            format!("Package '{}' is not installed", package),
            "Try 'omg search' to find available packages",
        ));
    };

    // Check install reason
    let reason = pkg.reason();
    let reason_str = match reason {
        alpm::PackageReason::Explicit => "explicitly installed",
        alpm::PackageReason::Depend => "installed as a dependency",
    };

    let mut commands = vec![
        Components::header("Package Analysis", format!("for {}", package)),
        Components::spacer(),
        Components::kv_list(
            Some("Package Information"),
            vec![
                ("Name", package),
                ("Version", pkg.version().as_str()),
                ("Reason", reason_str),
            ],
        ),
        Components::spacer(),
    ];

    if matches!(reason, alpm::PackageReason::Depend) {
        // Find what requires this package
        let mut required_by = Vec::new();
        for db_pkg in localdb.pkgs() {
            for dep in db_pkg.depends() {
                if dep.name() == package {
                    required_by.push(db_pkg.name().to_string());
                }
            }
        }

        if required_by.is_empty() {
            commands.push(Components::info("Required by: (orphan - can be removed)"));
            commands.push(Components::success("This package is safe to remove"));
        } else {
            commands.push(Components::card(
                format!("Required by ({} packages)", required_by.len()),
                required_by.clone(),
            ));

            // Show one dependency chain
            if let Some(first_req) = required_by.first() {
                if let Some(path) = build_dependency_path(&handle, first_req, package) {
                    commands.push(Components::spacer());
                    commands.push(Components::kv_list(Some("Dependency Path Example"), path));
                }
            }
        }
    } else {
        // Show what this package depends on
        let deps: Vec<_> = pkg.depends().into_iter().collect();
        if deps.is_empty() {
            commands.push(Components::info("Dependencies: (no dependencies)"));
        } else {
            let dep_list: Vec<(String, String)> = deps
                .iter()
                .take(10)
                .map(|dep| {
                    let installed = localdb.pkg(dep.name().as_bytes()).is_ok();
                    let status = if installed { "✓ installed" } else { "✗ not installed" };
                    (dep.name().to_string(), status.to_string())
                })
                .collect();

            commands.push(Components::kv_list(Some("Dependencies"), dep_list));

            if deps.len() > 10 {
                commands.push(Components::muted(format!("... and {} more dependencies", deps.len() - 10)));
            }
        }
    }

    // Safety assessment
    commands.push(Components::spacer());
    let required_by_count = count_reverse_deps(&handle, package);
    let (safety_msg, safety_type) = if required_by_count == 0 && matches!(reason, alpm::PackageReason::Depend) {
        ("YES - orphan dependency".to_string(), "safe")
    } else if required_by_count > 0 {
        (format!("NO - {} packages depend on it", required_by_count), "unsafe")
    } else {
        ("User decision - explicitly installed".to_string(), "decision")
    };

    match safety_type {
        "safe" => commands.push(Components::success(format!("Safe to remove: {}", safety_msg))),
        "unsafe" => commands.push(Components::warning(format!("Safe to remove: {}", safety_msg))),
        _ => commands.push(Components::info(format!("Safe to remove: {}", safety_msg))),
    }

    Ok(Cmd::batch(commands))
}

#[cfg(feature = "arch")]
fn build_dependency_path(handle: &alpm::Alpm, from: &str, to: &str) -> Option<Vec<(String, String)>> {
    // BFS to find shortest path
    let localdb = handle.localdb();
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut parent: HashMap<String, String> = HashMap::new();

    queue.push_back(from.to_string());
    visited.insert(from.to_string());

    while let Some(current) = queue.pop_front() {
        if current == to {
            // Reconstruct path
            let mut path = vec![to.to_string()];
            let mut current_node = to;
            while let Some(p) = parent.get(current_node) {
                path.push(p.clone());
                current_node = p;
            }
            path.reverse();

            let mut result = Vec::new();
            for (i, p) in path.iter().enumerate() {
                if i == 0 {
                    result.push((format!("└─ {}", p), "explicit".to_string()));
                } else if i == path.len() - 1 {
                    result.push((format!("└─ {}", p), "target package".to_string()));
                } else {
                    result.push((format!("└─ {}", p), "dependency".to_string()));
                }
            }
            return Some(result);
        }

        if let Ok(pkg) = localdb.pkg(current.as_bytes()) {
            for dep in pkg.depends() {
                let dep_name = dep.name().to_string();
                if !visited.contains(&dep_name) {
                    visited.insert(dep_name.clone());
                    parent.insert(dep_name.clone(), current.clone());
                    queue.push_back(dep_name);
                }
            }
        }
    }

    None
}

#[cfg(feature = "arch")]
fn count_reverse_deps(handle: &alpm::Alpm, package: &str) -> usize {
    let localdb = handle.localdb();
    let mut count = 0;

    for pkg in localdb.pkgs() {
        for dep in pkg.depends() {
            if dep.name() == package {
                count += 1;
                break;
            }
        }
    }

    count
}

#[cfg(feature = "arch")]
fn show_reverse_deps(package: &str) -> Result<Cmd<()>> {
    use alpm::Alpm;
    use crate::cli::components::Components;

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();

    // Check if package is installed
    if localdb.pkg(package).is_err() {
        return Ok(Components::error_with_suggestion(
            format!("Package '{}' is not installed", package),
            "Try 'omg search' to find available packages",
        ));
    }

    let mut dependents: Vec<(String, bool)> = Vec::new();

    for pkg in localdb.pkgs() {
        for dep in pkg.depends() {
            if dep.name() == package {
                let is_explicit = matches!(pkg.reason(), alpm::PackageReason::Explicit);
                dependents.push((pkg.name().to_string(), is_explicit));
                break;
            }
        }
    }

    let mut commands = vec![
        Components::header("Reverse Dependencies", format!("packages that depend on {}", package)),
        Components::spacer(),
    ];

    if dependents.is_empty() {
        commands.push(Components::success("Nothing depends on this package"));
        commands.push(Components::info("Safe to remove: YES (if not needed)"));
    } else {
        dependents.sort_by(|a, b| b.1.cmp(&a.1)); // Explicit first

        let explicit_count = dependents.iter().filter(|(_, e)| *e).count();
        let dep_count = dependents.len() - explicit_count;

        let dep_list: Vec<(String, String)> = dependents
            .iter()
            .map(|(name, is_explicit)| {
                let marker = if *is_explicit {
                    "explicit"
                } else {
                    "dependency"
                };
                (name.clone(), marker.to_string())
            })
            .collect();

        commands.push(Components::kv_list(
            Some(format!("Dependents ({} total)", dependents.len())),
            dep_list,
        ));

        commands.push(Components::spacer());
        commands.push(Components::warning(format!(
            "Safe to remove: NO (would break {} dependents: {} explicit, {} dependencies)",
            dependents.len(),
            explicit_count,
            dep_count
        )));
    }

    Ok(Cmd::batch(commands))
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn show_dependency_chain_debian(package: &str) -> Result<Cmd<()>> {
    use std::process::Command;
    use crate::cli::components::Components;

    // Use apt-cache to get dependency info
    let output = Command::new("apt-cache")
        .args(["depends", "--", package])
        .output()?;

    if !output.status.success() {
        return Ok(Components::error_with_suggestion(
            format!("Package '{}' not found", package),
            "Try 'omg search' to find available packages",
        ));
    }

    let deps_str = String::from_utf8_lossy(&output.stdout);
    let mut deps = Vec::new();

    for line in deps_str.lines() {
        let line = line.trim();
        if line.starts_with("Depends:") {
            if let Some(dep) = line.strip_prefix("Depends:") {
                deps.push(dep.trim().to_string());
            }
        }
    }

    if deps.is_empty() {
        Ok(Cmd::batch(vec![
            Components::header("Package Analysis", package),
            Components::spacer(),
            Components::info("No dependencies found"),
        ]))
    } else {
        Ok(Cmd::batch(vec![
            Components::header("Package Analysis", package),
            Components::spacer(),
            Components::card(format!("Dependencies ({})", deps.len()), deps),
        ]))
    }
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn show_reverse_deps_debian(package: &str) -> Result<Cmd<()>> {
    use std::process::Command;
    use crate::cli::components::Components;

    let output = Command::new("apt-cache")
        .args(["rdepends", "--", package])
        .output()?;

    if !output.status.success() {
        return Ok(Components::error_with_suggestion(
            format!("Package '{}' not found", package),
            "Try 'omg search' to find available packages",
        ));
    }

    let rdeps_str = String::from_utf8_lossy(&output.stdout);
    let mut deps = Vec::new();

    for line in rdeps_str.lines().skip(2) {
        // Skip header lines
        let dep = line.trim();
        if !dep.is_empty() && !dep.starts_with("Reverse") {
            deps.push(dep.to_string());
        }
    }

    Ok(Cmd::batch(vec![
        Components::header("Reverse Dependencies", format!("packages that depend on {}", package)),
        Components::spacer(),
        Components::kv_list(
            Some(format!("Dependents ({})", deps.len())),
            deps.into_iter().map(|d| (d.clone(), String::new())).collect(),
        ),
    ]))
}
