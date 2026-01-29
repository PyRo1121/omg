//! `omg size` - Show disk usage by packages

use anyhow::Result;

use crate::cli::tea::Cmd;

/// Show disk usage analysis
pub fn run(tree: Option<&str>, limit: usize) -> Result<()> {
    let cmd = if let Some(package) = tree {
        // SECURITY: Validate package name
        crate::core::security::validate_package_name(package)?;
        show_package_tree(package)?
    } else {
        show_top_packages(limit)?
    };
    crate::cli::packages::execute_cmd(cmd);
    Ok(())
}

#[cfg(feature = "arch")]
fn show_top_packages(limit: usize) -> Result<Cmd<()>> {
    use crate::cli::components::Components;
    use alpm::Alpm;

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();
    let mut packages: Vec<(String, i64)> = localdb
        .pkgs()
        .into_iter()
        .map(|p: &alpm::Package| (p.name().to_string(), p.isize()))
        .collect();

    packages.sort_by(|a, b| b.1.cmp(&a.1));

    let total: i64 = packages.iter().map(|(_, s)| s).sum();

    let mut content = Vec::new();
    for (i, (name, size)) in packages.iter().take(limit).enumerate() {
        let size_str = format_size(*size);
        let bar = generate_bar(*size, packages[0].1, 20);
        content.push(format!("{:>3}. {} {:>10}  {}", i + 1, bar, size_str, name));
    }

    let mut commands = vec![
        Cmd::header("Disk Usage Analysis", "by installed size"),
        Cmd::spacer(),
        Cmd::card(format!("Top {limit} Packages"), content),
        Cmd::spacer(),
        Components::kv_list(
            Some("Summary"),
            vec![
                ("Total Disk Usage", &format_size(total)),
                ("Number of Packages", &packages.len().to_string()),
            ],
        ),
    ];

    // Show cache size
    if let Ok(cache_size) = get_cache_size() {
        commands.push(Cmd::spacer());
        commands.push(Cmd::info(format!(
            "Cache: {} (run 'omg clean --cache' to clear)",
            format_size(cache_size)
        )));
    }

    Ok(Cmd::batch(commands))
}

#[cfg(feature = "arch")]
fn show_package_tree(package: &str) -> Result<Cmd<()>> {
    use crate::cli::components::Components;
    use alpm::Alpm;
    use std::collections::HashSet;

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();

    let pkg = localdb
        .pkg(package)
        .map_err(|_| anyhow::anyhow!("Package '{package}' not installed"))?;

    let pkg_size = pkg.isize();

    // Get dependencies and their sizes
    let mut visited = HashSet::new();
    visited.insert(package.to_string());

    let mut dep_sizes: Vec<(String, i64)> = Vec::new();
    let mut total_dep_size: i64 = 0;

    for dep in pkg.depends() {
        let dep_name = dep.name();
        if visited.contains(dep_name) {
            continue;
        }
        visited.insert(dep_name.to_string());

        if let Ok(dep_pkg) = localdb.pkg(dep_name) {
            let size = dep_pkg.isize();
            dep_sizes.push((dep_name.to_string(), size));
            total_dep_size += size;
        }
    }

    dep_sizes.sort_by(|a, b| b.1.cmp(&a.1));

    let mut commands = vec![Cmd::header("Package Size Tree", package), Cmd::spacer()];

    // Package info
    commands.push(Components::kv_list(
        Some("Package Size"),
        vec![
            (package, format_size(pkg_size)),
            ("Type", "installed package".to_string()),
        ],
    ));

    // Dependencies
    if !dep_sizes.is_empty() {
        let dep_content: Vec<String> = dep_sizes
            .iter()
            .take(10)
            .map(|(name, size)| format!("├─ {} {}", name, format_size(*size)))
            .collect();

        commands.push(Cmd::spacer());
        commands.push(Cmd::card(
            format!("Dependencies ({} total)", dep_sizes.len()),
            dep_content,
        ));

        if dep_sizes.len() > 10 {
            use crate::cli::tea::{StyledTextConfig, TextStyle};
            commands.push(Cmd::styled_text(StyledTextConfig {
                text: format!("... and {} more dependencies", dep_sizes.len() - 10),
                style: TextStyle::Muted,
            }));
        }
    }

    // Total footprint
    let total = pkg_size + total_dep_size;
    commands.push(Cmd::spacer());
    commands.push(Components::kv_list(
        Some("Total Footprint"),
        vec![
            ("Combined Total", &format_size(total)),
            ("Package Size", &format_size(pkg_size)),
            ("Dependencies", &format_size(total_dep_size)),
        ],
    ));

    Ok(Cmd::batch(commands))
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn show_top_packages(limit: usize) -> Result<Cmd<()>> {
    use crate::cli::components::Components;
    use std::process::Command;

    let output = Command::new("dpkg-query")
        .args(["-W", "-f=${Installed-Size}\t${Package}\n"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut packages: Vec<(String, i64)> = stdout
        .lines()
        .filter_map(|line| {
            let parts: Vec<_> = line.split('\t').collect();
            if parts.len() == 2 {
                let size: i64 = parts[0].parse().unwrap_or(0) * 1024; // KB to bytes
                Some((parts[1].to_string(), size))
            } else {
                None
            }
        })
        .collect();

    packages.sort_by(|a, b| b.1.cmp(&a.1));

    let total: i64 = packages.iter().map(|(_, s)| s).sum();

    let mut content = Vec::new();
    for (i, (name, size)) in packages.iter().take(limit).enumerate() {
        let size_str = format_size(*size);
        let bar = generate_bar(*size, packages[0].1, 20);
        content.push(format!("{:>3}. {} {:>10}  {}", i + 1, bar, size_str, name));
    }

    Ok(Cmd::batch(vec![
        Cmd::header("Disk Usage Analysis", "by installed size"),
        Cmd::spacer(),
        Cmd::card(format!("Top {limit} Packages"), content),
        Cmd::spacer(),
        Components::kv_list(
            Some("Summary"),
            vec![
                ("Total Disk Usage", &format_size(total)),
                ("Number of Packages", &packages.len().to_string()),
            ],
        ),
    ]))
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn show_package_tree(package: &str) -> Result<Cmd<()>> {
    use crate::cli::components::Components;
    use std::process::Command;

    // Get package size
    let output = Command::new("dpkg-query")
        .args(["-W", "-f=${Installed-Size}", "--", package])
        .output()?;

    if !output.status.success() {
        return Ok(Components::error_with_suggestion(
            format!("Package '{package}' not installed"),
            "Try 'omg search' to find available packages",
        ));
    }

    let size: i64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
        * 1024;

    // Get dependencies
    let deps_output = Command::new("apt-cache")
        .args(["depends", "--installed", "--", package])
        .output()?;

    let deps_str = String::from_utf8_lossy(&deps_output.stdout);
    let mut dep_sizes: Vec<(String, i64)> = Vec::new();

    for line in deps_str.lines() {
        if !line.trim().starts_with("Depends:") {
            continue;
        }
        let dep_name = match line.trim().strip_prefix("Depends:") {
            Some(suffix) => suffix.trim(),
            None => continue,
        };

        let Ok(dep_out) = Command::new("dpkg-query")
            .args(["-W", "-f=${Installed-Size}", "--", dep_name])
            .output()
        else {
            continue;
        };

        if dep_out.status.success() {
            let dep_size: i64 = String::from_utf8_lossy(&dep_out.stdout)
                .trim()
                .parse()
                .unwrap_or(0)
                * 1024;
            dep_sizes.push((dep_name.to_string(), dep_size));
        }
    }

    dep_sizes.sort_by(|a, b| b.1.cmp(&a.1));
    let total_deps: i64 = dep_sizes.iter().map(|(_, s)| s).sum();

    let mut commands = vec![
        Cmd::header("Package Size Tree", package),
        Cmd::spacer(),
        Components::kv_list(
            Some("Package Size"),
            vec![
                (package, format_size(size)),
                ("Type", "installed package".to_string()),
            ],
        ),
    ];

    if !dep_sizes.is_empty() {
        let dep_content: Vec<String> = dep_sizes
            .iter()
            .take(10)
            .map(|(name, dep_size)| format!("├─ {} {}", name, format_size(*dep_size)))
            .collect();

        commands.push(Cmd::spacer());
        commands.push(Cmd::card(
            format!("Dependencies ({} total)", dep_sizes.len()),
            dep_content,
        ));
    }

    let total = size + total_deps;
    commands.push(Cmd::spacer());
    commands.push(Components::kv_list(
        Some("Total Footprint"),
        vec![
            ("Combined Total", &format_size(total)),
            ("Package Size", &format_size(size)),
            ("Dependencies", &format_size(total_deps)),
        ],
    ));

    Ok(Cmd::batch(commands))
}

#[cfg(not(any(feature = "arch", feature = "debian")))]
fn show_top_packages(_limit: usize) -> Result<Cmd<()>> {
    anyhow::bail!("Size analysis requires arch or debian feature")
}

#[cfg(not(any(feature = "arch", feature = "debian")))]
fn show_package_tree(_package: &str) -> Result<Cmd<()>> {
    anyhow::bail!("Size analysis requires arch or debian feature")
}

fn format_size(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn generate_bar(value: i64, max: i64, width: usize) -> String {
    let ratio = if max > 0 {
        (value as f64 / max as f64).min(1.0)
    } else {
        0.0
    };
    let filled = (ratio * width as f64) as usize;
    let empty = width - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

#[cfg(feature = "arch")]
fn get_cache_size() -> Result<i64> {
    use std::fs;

    let cache_dir = std::path::Path::new("/var/cache/pacman/pkg");
    if !cache_dir.exists() {
        return Ok(0);
    }

    let mut total: i64 = 0;
    for entry in fs::read_dir(cache_dir)? {
        if let Ok(entry) = entry
            && let Ok(meta) = entry.metadata()
        {
            // Use saturating_add to prevent overflow on extremely large caches
            total = total.saturating_add(meta.len().try_into().unwrap_or(i64::MAX));
        }
    }

    Ok(total)
}
