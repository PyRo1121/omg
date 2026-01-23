//! `omg size` - Show disk usage by packages

use anyhow::Result;
#[allow(unused_imports)]
use owo_colors::OwoColorize;

/// Show disk usage analysis
pub fn run(tree: Option<&str>, limit: usize) -> Result<()> {
    if let Some(package) = tree {
        // SECURITY: Validate package name
        crate::core::security::validate_package_name(package)?;
        show_package_tree(package)
    } else {
        show_top_packages(limit)
    }
}

#[cfg(feature = "arch")]
fn show_top_packages(limit: usize) -> Result<()> {
    use alpm::Alpm;

    println!("{} Disk Usage Analysis\n", "OMG".cyan().bold());

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
    let total_str = format_size(total);

    println!("  {} (by installed size)", "Top packages:".bold());
    println!();

    for (i, (name, size)) in packages.iter().take(limit).enumerate() {
        let size_str = format_size(*size);
        let bar = generate_bar(*size, packages[0].1, 20);
        println!(
            "  {:>3}. {} {:>10}  {}",
            i + 1,
            bar.blue(),
            size_str.cyan(),
            name
        );
    }

    println!();
    println!(
        "  {} {} in {} packages",
        "Total:".bold(),
        total_str.green(),
        packages.len()
    );

    // Show cache size
    if let Ok(cache_size) = get_cache_size() {
        println!(
            "  {} {} (run 'omg clean --cache' to clear)",
            "Cache:".bold(),
            format_size(cache_size).yellow()
        );
    }

    Ok(())
}

#[cfg(feature = "arch")]
fn show_package_tree(package: &str) -> Result<()> {
    use alpm::Alpm;
    use std::collections::HashSet;

    println!(
        "{} Size tree for {}\n",
        "OMG".cyan().bold(),
        package.yellow()
    );

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();

    let pkg = localdb
        .pkg(package)
        .map_err(|_| anyhow::anyhow!("Package '{package}' not installed"))?;

    let pkg_size = pkg.isize();
    println!(
        "  {} {} (package itself)",
        package.yellow().bold(),
        format_size(pkg_size).cyan()
    );

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

    if !dep_sizes.is_empty() {
        println!();
        println!("  {}", "Dependencies:".bold());
        for (name, size) in dep_sizes.iter().take(10) {
            println!("    ├─ {} {}", name, format_size(*size).dimmed());
        }
        if dep_sizes.len() > 10 {
            println!("    └─ ... and {} more", dep_sizes.len() - 10);
        }
    }

    println!();
    let total = pkg_size + total_dep_size;
    println!(
        "  {} {} (package: {}, deps: {})",
        "Total footprint:".bold(),
        format_size(total).green(),
        format_size(pkg_size),
        format_size(total_dep_size)
    );

    Ok(())
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn show_top_packages(limit: usize) -> Result<()> {
    use std::process::Command;

    println!("{} Disk Usage Analysis\n", "OMG".cyan().bold());

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

    println!("  {} (by installed size)", "Top packages:".bold());
    println!();

    for (i, (name, size)) in packages.iter().take(limit).enumerate() {
        let size_str = format_size(*size);
        let bar = generate_bar(*size, packages[0].1, 20);
        println!(
            "  {:>3}. {} {:>10}  {}",
            i + 1,
            bar.blue(),
            size_str.cyan(),
            name
        );
    }

    println!();
    println!(
        "  {} {} in {} packages",
        "Total:".bold(),
        format_size(total).green(),
        packages.len()
    );

    Ok(())
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn show_package_tree(package: &str) -> Result<()> {
    use std::process::Command;

    println!(
        "{} Size tree for {}\n",
        "OMG".cyan().bold(),
        package.yellow()
    );

    // Get package size
    let output = Command::new("dpkg-query")
        .args(["-W", "-f=${Installed-Size}", "--", package])
        .output()?;

    if !output.status.success() {
        anyhow::bail!("Package '{}' not installed", package);
    }

    let size: i64 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .unwrap_or(0)
        * 1024;

    println!(
        "  {} {} ({})",
        package.yellow().bold(),
        format_size(size).cyan(),
        "package itself"
    );

    // Get dependencies
    let deps_output = Command::new("apt-cache")
        .args(["depends", "--installed", "--", package])
        .output()?;

    let deps_str = String::from_utf8_lossy(&deps_output.stdout);
    let mut dep_sizes: Vec<(String, i64)> = Vec::new();

    for line in deps_str.lines() {
        if line.trim().starts_with("Depends:") {
            let dep_name = line.trim().strip_prefix("Depends:").unwrap_or("").trim();
            if let Ok(dep_out) = Command::new("dpkg-query")
                .args(["-W", "-f=${Installed-Size}", "--", dep_name])
                .output()
            {
                if dep_out.status.success() {
                    let dep_size: i64 = String::from_utf8_lossy(&dep_out.stdout)
                        .trim()
                        .parse()
                        .unwrap_or(0)
                        * 1024;
                    dep_sizes.push((dep_name.to_string(), dep_size));
                }
            }
        }
    }

    dep_sizes.sort_by(|a, b| b.1.cmp(&a.1));
    let total_deps: i64 = dep_sizes.iter().map(|(_, s)| s).sum();

    if !dep_sizes.is_empty() {
        println!();
        println!("  {}", "Dependencies:".bold());
        for (name, dep_size) in dep_sizes.iter().take(10) {
            println!("    ├─ {} {}", name, format_size(*dep_size).dimmed());
        }
    }

    println!();
    println!(
        "  {} {} (package: {}, deps: {})",
        "Total footprint:".bold(),
        format_size(size + total_deps).green(),
        format_size(size),
        format_size(total_deps)
    );

    Ok(())
}

#[cfg(not(any(feature = "arch", feature = "debian")))]
fn show_top_packages(_limit: usize) -> Result<()> {
    anyhow::bail!("Size analysis requires arch or debian feature")
}

#[cfg(not(any(feature = "arch", feature = "debian")))]
fn show_package_tree(_package: &str) -> Result<()> {
    anyhow::bail!("Size analysis requires arch or debian feature")
}

#[allow(dead_code)]
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

#[allow(dead_code)]
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
            total += meta.len().cast_signed();
        }
    }

    Ok(total)
}
