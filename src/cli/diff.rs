//! `omg diff` - Compare two environment lock files

use anyhow::Result;
use owo_colors::OwoColorize;
use std::collections::{HashMap, HashSet};

use crate::cli::style;
use crate::core::env::fingerprint::EnvironmentState;

/// Compare two environment states
pub async fn run(from: Option<&str>, to: &str) -> Result<()> {
    // SECURITY: Validate paths/names
    if let Some(f) = from {
        crate::core::security::validate_relative_path(f)?;
    }
    crate::core::security::validate_relative_path(to)?;

    println!("{} Environment Comparison\n", style::runtime("OMG"));

    // Load the "from" state (current env or specified file)
    let from_state = if let Some(from_path) = from {
        println!(
            "  From: {}",
            style::maybe_color(from_path, |t| t.cyan().to_string())
        );
        EnvironmentState::load(from_path)?
    } else {
        println!(
            "  From: {} (current)",
            style::maybe_color("live environment", |t| t.cyan().to_string())
        );
        EnvironmentState::capture().await?
    };

    // Load the "to" state
    println!(
        "  To:   {}",
        style::maybe_color(to, |t| t.cyan().to_string())
    );
    let to_state = EnvironmentState::load(to)?;

    println!();

    if from_state.hash == to_state.hash {
        println!(
            "  {} Environments are identical!",
            style::maybe_color("✓", |t| t.green().to_string())
        );
        return Ok(());
    }

    // Compare runtimes
    let runtime_diff = diff_runtimes(&from_state.runtimes, &to_state.runtimes);
    if !runtime_diff.is_empty() {
        println!(
            "  {}",
            style::maybe_color("Runtime differences:", |t| t.bold().to_string())
        );
        for change in &runtime_diff {
            println!("    {change}");
        }
        println!();
    }

    // Compare packages
    let package_diff = diff_packages(&from_state.packages, &to_state.packages);
    if !package_diff.added.is_empty()
        || !package_diff.removed.is_empty()
        || !package_diff.changed.is_empty()
    {
        println!(
            "  {}",
            style::maybe_color("Package differences:", |t| t.bold().to_string())
        );

        if !package_diff.added.is_empty() {
            println!(
                "    {} {} packages added:",
                style::maybe_color("+", |t| t.green().to_string()),
                package_diff.added.len()
            );
            for pkg in package_diff.added.iter().take(10) {
                println!(
                    "      {} {}",
                    style::maybe_color("+", |t| t.green().to_string()),
                    pkg
                );
            }
            if package_diff.added.len() > 10 {
                println!("      ... and {} more", package_diff.added.len() - 10);
            }
        }

        if !package_diff.removed.is_empty() {
            println!(
                "    {} {} packages removed:",
                style::maybe_color("-", |t| t.red().to_string()),
                package_diff.removed.len()
            );
            for pkg in package_diff.removed.iter().take(10) {
                println!(
                    "      {} {}",
                    style::maybe_color("-", |t| t.red().to_string()),
                    pkg
                );
            }
            if package_diff.removed.len() > 10 {
                println!("      ... and {} more", package_diff.removed.len() - 10);
            }
        }

        println!();
    }

    // Summary
    let total_changes = runtime_diff.len()
        + package_diff.added.len()
        + package_diff.removed.len()
        + package_diff.changed.len();

    println!(
        "  {}",
        style::maybe_color("Summary:", |t| t.bold().to_string())
    );
    println!(
        "    Runtimes:  {} changes",
        if runtime_diff.is_empty() {
            style::version("0")
        } else {
            style::maybe_color(&runtime_diff.len().to_string(), |t| t.yellow().to_string())
        }
    );
    println!(
        "    Packages:  +{} -{} ~{}",
        style::version(&package_diff.added.len().to_string()),
        style::maybe_color(&package_diff.removed.len().to_string(), |t| {
            t.red().to_string()
        }),
        style::maybe_color(&package_diff.changed.len().to_string(), |t| {
            t.yellow().to_string()
        })
    );
    println!();

    if total_changes > 0 {
        println!(
            "  {} To sync to the target environment:",
            style::dim("Hint:")
        );
        println!("       {}", style::command(&format!("omg env sync {to}")));
    }

    Ok(())
}

fn diff_runtimes(from: &HashMap<String, String>, to: &HashMap<String, String>) -> Vec<String> {
    let mut changes = Vec::new();

    let all_runtimes: HashSet<_> = from.keys().chain(to.keys()).collect();

    for runtime in all_runtimes {
        match (from.get(runtime), to.get(runtime)) {
            (Some(from_ver), Some(to_ver)) if from_ver != to_ver => {
                changes.push(format!(
                    "{} {} → {}",
                    style::path(runtime),
                    style::dim(from_ver),
                    style::version(to_ver)
                ));
            }
            (Some(from_ver), None) => {
                changes.push(format!(
                    "{} {} → {}",
                    style::maybe_color("-", |t| t.red().to_string()),
                    runtime,
                    style::dim(&format!("(removed, was {from_ver})"))
                ));
            }
            (None, Some(to_ver)) => {
                changes.push(format!(
                    "{} {} {}",
                    style::maybe_color("+", |t| t.green().to_string()),
                    runtime,
                    style::version(&format!("(added @ {to_ver})"))
                ));
            }
            _ => {}
        }
    }

    changes
}

struct PackageDiff {
    added: Vec<String>,
    removed: Vec<String>,
    changed: Vec<String>,
}

fn diff_packages(from: &[String], to: &[String]) -> PackageDiff {
    let from_set: HashSet<&str> = from.iter().map(String::as_str).collect();
    let to_set: HashSet<&str> = to.iter().map(String::as_str).collect();

    let added: Vec<String> = to_set
        .difference(&from_set)
        .map(std::string::ToString::to_string)
        .collect();

    let removed: Vec<String> = from_set
        .difference(&to_set)
        .map(std::string::ToString::to_string)
        .collect();

    PackageDiff {
        added,
        removed,
        changed: Vec::new(), // Version changes would need more data
    }
}
