//! `omg migrate` - Cross-distro migration tools

use anyhow::Result;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

use crate::core::env::distro::detect_distro;
use crate::core::env::fingerprint::EnvironmentState;

#[derive(Debug, Serialize, Deserialize)]
pub struct MigrationManifest {
    pub version: String,
    pub source_distro: String,
    pub created_at: i64,
    pub runtimes: HashMap<String, String>,
    pub packages: Vec<PackageMapping>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageMapping {
    pub original_name: String,
    pub category: String,
    pub description: Option<String>,
    pub alternatives: Vec<String>,
}

/// Export current environment to portable manifest
pub async fn export(output: &str) -> Result<()> {
    println!("{} Exporting environment...\n", "OMG".cyan().bold());

    let state = EnvironmentState::capture().await?;
    let distro = format!("{:?}", detect_distro()).to_lowercase();

    let mut packages = Vec::new();
    for pkg_name in &state.packages {
        let mapping = create_package_mapping(pkg_name);
        packages.push(mapping);
    }

    let manifest = MigrationManifest {
        version: "1.0".to_string(),
        source_distro: distro.clone(),
        created_at: jiff::Timestamp::now().as_second(),
        runtimes: state.runtimes.clone(),
        packages,
    };

    let content = serde_json::to_string_pretty(&manifest)?;
    fs::write(output, &content)?;

    println!("  {} Exported to {}", "✓".green(), output.cyan());
    println!();
    println!("  Source distro: {}", distro.yellow());
    println!("  Runtimes: {}", state.runtimes.len());
    println!("  Packages: {}", state.packages.len());
    println!();
    println!("  {}", "To import on another machine:".bold());
    println!("    1. Copy {} to the target machine", output.cyan());
    println!(
        "    2. Run {}",
        format!("omg migrate import {output}").cyan()
    );

    Ok(())
}

/// Import environment from manifest with package mapping
pub async fn import(manifest_path: &str, dry_run: bool) -> Result<()> {
    println!(
        "{} {} manifest...\n",
        "OMG".cyan().bold(),
        if dry_run { "Previewing" } else { "Importing" }
    );

    let content = fs::read_to_string(manifest_path)?;
    let manifest: MigrationManifest = serde_json::from_str(&content)?;

    let target_distro = format!("{:?}", detect_distro()).to_lowercase();

    println!(
        "  Source: {} → Target: {}",
        manifest.source_distro.yellow(),
        target_distro.cyan()
    );
    println!();

    // Map packages
    println!("  {}", "Package mapping:".bold());

    let mut mapped = 0;
    let mut unmapped = Vec::new();
    let mut to_install = Vec::new();

    for pkg in &manifest.packages {
        let target_pkg = map_package(&pkg.original_name, &manifest.source_distro, &target_distro);

        if let Some(target) = target_pkg {
            if target != pkg.original_name {
                println!(
                    "    {} {} → {}",
                    "✓".green(),
                    pkg.original_name.dimmed(),
                    target.cyan()
                );
            }
            to_install.push(target);
            mapped += 1;
        } else {
            unmapped.push(&pkg.original_name);
        }
    }

    println!();
    println!(
        "  Mapped: {}/{} packages",
        mapped.to_string().green(),
        manifest.packages.len()
    );

    if !unmapped.is_empty() {
        println!();
        println!("  {} No direct mapping ({}):", "⚠".yellow(), unmapped.len());
        for pkg in unmapped.iter().take(10) {
            println!("    {} {}", "?".yellow(), pkg);
        }
        if unmapped.len() > 10 {
            println!("    ... and {} more", unmapped.len() - 10);
        }
    }

    // Runtimes
    println!();
    println!("  {}", "Runtimes:".bold());
    for (runtime, version) in &manifest.runtimes {
        println!("    {} {} @ {}", "→".blue(), runtime, version.cyan());
    }

    if dry_run {
        println!();
        println!("  {} Dry run complete - no changes made", "ℹ".blue());
        println!(
            "  Run without --dry-run to install: {}",
            format!("omg migrate import {manifest_path}").cyan()
        );
        return Ok(());
    }

    // Apply changes
    println!();
    println!("  {}", "Applying...".bold());

    // Install runtimes
    for (runtime, version) in &manifest.runtimes {
        println!("    Installing {runtime} {version}...");
        if let Err(e) = crate::cli::runtimes::use_version(runtime, Some(version)).await {
            println!("      {} Failed to install {runtime}: {e}", "✗".red());
        }
    }

    // Install packages
    if !to_install.is_empty() {
        println!();
        println!("    Installing {} packages...", to_install.len());
        if let Err(e) = crate::cli::packages::install(&to_install, true).await {
            println!("      {} Package installation failed: {e}", "✗".red());
        }
    }

    println!();
    println!("  {} Migration complete!", "✓".green());
    println!("  Some packages may need manual installation - check the unmapped list above.");

    Ok(())
}

fn create_package_mapping(name: &str) -> PackageMapping {
    // Categorize package
    let category = categorize_package(name);

    PackageMapping {
        original_name: name.to_string(),
        category,
        description: None,
        alternatives: get_alternatives(name),
    }
}

fn categorize_package(name: &str) -> String {
    if name.contains("lib") {
        "library".to_string()
    } else if name.contains("dev") || name.contains("devel") {
        "development".to_string()
    } else if name.ends_with("-doc") || name.ends_with("-docs") {
        "documentation".to_string()
    } else {
        "application".to_string()
    }
}

fn get_alternatives(name: &str) -> Vec<String> {
    // Common package name mappings between distros
    let mappings: HashMap<&str, Vec<&str>> = [
        ("vim", vec!["vim", "vim-nox", "neovim"]),
        ("gcc", vec!["gcc", "build-essential"]),
        ("make", vec!["make", "build-essential"]),
        ("git", vec!["git"]),
        ("curl", vec!["curl"]),
        ("wget", vec!["wget"]),
        ("python", vec!["python3", "python"]),
        ("nodejs", vec!["nodejs", "node"]),
    ]
    .into_iter()
    .collect();

    mappings
        .get(name)
        .map(|v| v.iter().map(std::string::ToString::to_string).collect())
        .unwrap_or_default()
}

fn map_package(name: &str, from: &str, to: &str) -> Option<String> {
    // Direct mappings between distros
    let arch_to_debian: HashMap<&str, &str> = [
        ("base-devel", "build-essential"),
        ("python", "python3"),
        ("python-pip", "python3-pip"),
        ("nodejs", "nodejs"),
        ("linux-headers", "linux-headers-generic"),
        ("lib32-glibc", "libc6-i386"),
    ]
    .into_iter()
    .collect();

    let debian_to_arch: HashMap<&str, &str> = [
        ("build-essential", "base-devel"),
        ("python3", "python"),
        ("python3-pip", "python-pip"),
        ("linux-headers-generic", "linux-headers"),
    ]
    .into_iter()
    .collect();

    match (from, to) {
        ("arch", "debian" | "ubuntu") => arch_to_debian
            .get(name)
            .map(std::string::ToString::to_string)
            .or_else(|| Some(name.to_string())),
        ("debian" | "ubuntu", "arch") => debian_to_arch
            .get(name)
            .map(std::string::ToString::to_string)
            .or_else(|| Some(name.to_string())),
        _ => Some(name.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_package() {
        assert_eq!(categorize_package("libc6"), "library");
        assert_eq!(categorize_package("libssl-dev"), "library"); // contains lib
        assert_eq!(categorize_package("python3-dev"), "development");
        assert_eq!(categorize_package("gcc-docs"), "documentation");
        assert_eq!(categorize_package("firefox"), "application");
    }

    #[test]
    fn test_get_alternatives() {
        let alts = get_alternatives("vim");
        assert!(alts.contains(&"neovim".to_string()));
        assert!(alts.contains(&"vim".to_string()));

        let empty = get_alternatives("nonexistent-pkg-123");
        assert!(empty.is_empty());
    }

    #[test]
    fn test_map_package() {
        // Arch to Debian
        assert_eq!(
            map_package("base-devel", "arch", "debian"),
            Some("build-essential".to_string())
        );
        assert_eq!(
            map_package("python", "arch", "ubuntu"),
            Some("python3".to_string())
        );

        // Debian to Arch
        assert_eq!(
            map_package("build-essential", "debian", "arch"),
            Some("base-devel".to_string())
        );
        assert_eq!(
            map_package("python3", "ubuntu", "arch"),
            Some("python".to_string())
        );

        // No mapping (identity)
        assert_eq!(
            map_package("my-custom-pkg", "arch", "debian"),
            Some("my-custom-pkg".to_string())
        );
    }
}
