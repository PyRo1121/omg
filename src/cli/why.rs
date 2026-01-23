//! `omg why` - Explain why a package is installed (dependency chain)

use anyhow::Result;
use owo_colors::OwoColorize;
#[cfg(feature = "arch")]
use std::collections::{HashMap, HashSet, VecDeque};

/// Explain why a package is installed
pub fn run(package: &str, reverse: bool) -> Result<()> {
    // SECURITY: Validate package name
    crate::core::security::validate_package_name(package)?;

    println!(
        "{} Analyzing dependencies for {}...\n",
        "OMG".cyan().bold(),
        package.yellow()
    );

    #[cfg(feature = "arch")]
    {
        if reverse {
            show_reverse_deps(package)?;
        } else {
            show_dependency_chain(package)?;
        }
    }

    #[cfg(all(feature = "debian", not(feature = "arch")))]
    {
        if reverse {
            show_reverse_deps_debian(package)?;
        } else {
            show_dependency_chain_debian(package)?;
        }
    }

    #[cfg(not(any(feature = "arch", feature = "debian")))]
    {
        let _ = reverse;
        anyhow::bail!("Package dependency analysis requires arch or debian feature");
    }

    #[cfg(any(feature = "arch", feature = "debian"))]
    Ok(())
}

#[cfg(feature = "arch")]
fn show_dependency_chain(package: &str) -> Result<()> {
    use alpm::Alpm;

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();

    // Check if package is installed
    let Ok(pkg) = localdb.pkg(package) else {
        println!("  {} Package '{}' is not installed", "✗".red(), package);
        return Ok(());
    };

    // Check install reason
    let reason = pkg.reason();
    let reason_str = match reason {
        alpm::PackageReason::Explicit => "explicitly installed".green().to_string(),
        alpm::PackageReason::Depend => "installed as a dependency".yellow().to_string(),
    };

    println!("  {} {}", "Package:".bold(), package.cyan());
    println!("  {} {}", "Version:".bold(), pkg.version());
    println!("  {} {}", "Reason:".bold(), reason_str);
    println!();

    if matches!(reason, alpm::PackageReason::Depend) {
        // Find what requires this package
        println!("  {} (what needs this package)", "Required by:".bold());

        let mut required_by = Vec::new();
        for db_pkg in localdb.pkgs() {
            for dep in db_pkg.depends() {
                if dep.name() == package {
                    required_by.push(db_pkg.name().to_string());
                }
            }
        }

        if required_by.is_empty() {
            println!("    {} (orphan - can be removed)", "Nothing".dimmed());
        } else {
            for req in &required_by {
                println!("    {} {}", "→".blue(), req);
            }
            println!();

            // Show one dependency chain
            if let Some(first_req) = required_by.first() {
                println!("  {} (example chain)", "Dependency path:".bold());
                print_dependency_path(&handle, first_req, package);
            }
        }
    } else {
        // Show what this package depends on
        println!("  {} (what this package needs)", "Dependencies:".bold());
        let deps: Vec<_> = pkg.depends().into_iter().collect();
        if deps.is_empty() {
            println!("    {} (no dependencies)", "None".dimmed());
        } else {
            for dep in deps.iter().take(10) {
                let installed = localdb.pkg(dep.name().as_bytes()).is_ok();
                let status = if installed {
                    "✓".green().to_string()
                } else {
                    "✗".red().to_string()
                };
                println!("    {} {}", status, dep.name());
            }
            if deps.len() > 10 {
                println!("    ... and {} more", deps.len() - 10);
            }
        }
    }

    // Safety assessment
    println!();
    let required_by_count = count_reverse_deps(&handle, package);
    if required_by_count == 0 && matches!(reason, alpm::PackageReason::Depend) {
        println!(
            "  {} {} (orphan dependency)",
            "Safe to remove:".bold(),
            "YES".green()
        );
    } else if required_by_count > 0 {
        println!(
            "  {} {} ({} packages depend on it)",
            "Safe to remove:".bold(),
            "NO".red(),
            required_by_count
        );
    } else {
        println!(
            "  {} {} (explicitly installed)",
            "Safe to remove:".bold(),
            "User decision".yellow()
        );
    }

    Ok(())
}

#[cfg(feature = "arch")]
fn print_dependency_path(handle: &alpm::Alpm, from: &str, to: &str) {
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

            for (i, p) in path.iter().enumerate() {
                let indent = "  ".repeat(i + 2);
                if i == 0 {
                    println!("{}└─ {} (explicit)", indent, p.cyan());
                } else if i == path.len() - 1 {
                    println!("{}└─ {} (target)", indent, p.yellow());
                } else {
                    println!("{indent}└─ {p}");
                }
            }
            return;
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

    println!("    (could not trace path)");
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
fn show_reverse_deps(package: &str) -> Result<()> {
    use alpm::Alpm;

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();

    // Check if package is installed
    if localdb.pkg(package).is_err() {
        println!("  {} Package '{}' is not installed", "✗".red(), package);
        return Ok(());
    }

    println!(
        "  {} (packages that depend on {})",
        "Reverse dependencies:".bold(),
        package.yellow()
    );
    println!();

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

    if dependents.is_empty() {
        println!("    {} Nothing depends on this package", "✓".green());
        println!();
        println!(
            "  {} {}",
            "Safe to remove:".bold(),
            "YES (if not needed)".green()
        );
    } else {
        dependents.sort_by(|a, b| b.1.cmp(&a.1)); // Explicit first

        let explicit_count = dependents.iter().filter(|(_, e)| *e).count();
        let dep_count = dependents.len() - explicit_count;

        for (name, is_explicit) in &dependents {
            let marker = if *is_explicit {
                "[explicit]".green().to_string()
            } else {
                "[dependency]".dimmed().to_string()
            };
            println!("    {} {} {}", "→".blue(), name, marker);
        }

        println!();
        println!(
            "  {} {} ({} explicit, {} dependencies)",
            "Total dependents:".bold(),
            dependents.len(),
            explicit_count,
            dep_count
        );
        println!(
            "  {} {}",
            "Safe to remove:".bold(),
            "NO (would break dependents)".red()
        );
    }

    Ok(())
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn show_dependency_chain_debian(package: &str) -> Result<()> {
    use std::process::Command;

    // Use apt-cache to get dependency info
    let output = Command::new("apt-cache")
        .args(["depends", "--", package])
        .output()?;

    if !output.status.success() {
        println!("  {} Package '{}' not found", "✗".red(), package);
        return Ok(());
    }

    let deps_str = String::from_utf8_lossy(&output.stdout);
    println!("  {} {}", "Package:".bold(), package.cyan());
    println!();
    println!("  {}", "Dependencies:".bold());

    for line in deps_str.lines() {
        let line = line.trim();
        if line.starts_with("Depends:") {
            let dep = line.strip_prefix("Depends:").unwrap_or("").trim();
            println!("    {} {}", "→".blue(), dep);
        }
    }

    Ok(())
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn show_reverse_deps_debian(package: &str) -> Result<()> {
    use std::process::Command;

    let output = Command::new("apt-cache")
        .args(["rdepends", "--", package])
        .output()?;

    if !output.status.success() {
        println!("  {} Package '{}' not found", "✗".red(), package);
        return Ok(());
    }

    let rdeps_str = String::from_utf8_lossy(&output.stdout);
    println!(
        "  {} (packages that depend on {})",
        "Reverse dependencies:".bold(),
        package.yellow()
    );
    println!();

    let mut count = 0;
    for line in rdeps_str.lines().skip(2) {
        // Skip header lines
        let dep = line.trim();
        if !dep.is_empty() && !dep.starts_with("Reverse") {
            println!("    {} {}", "→".blue(), dep);
            count += 1;
        }
    }

    println!();
    println!("  {} {}", "Total dependents:".bold(), count);

    Ok(())
}
