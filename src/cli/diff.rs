//! `omg diff` - Compare two environment lock files

use anyhow::Result;
use owo_colors::OwoColorize;
use std::collections::{HashMap, HashSet};

use crate::core::env::fingerprint::EnvironmentState;

/// Compare two environment states
pub async fn run(from: Option<&str>, to: &str) -> Result<()> {
    println!("{} Environment Comparison\n", "OMG".cyan().bold());

    // Load the "from" state (current env or specified file)
    let from_state = if let Some(from_path) = from {
        println!("  From: {}", from_path.cyan());
        EnvironmentState::load(from_path)?
    } else {
        println!("  From: {} (current)", "live environment".cyan());
        EnvironmentState::capture().await?
    };

    // Load the "to" state
    println!("  To:   {}", to.cyan());
    let to_state = EnvironmentState::load(to)?;

    println!();

    if from_state.hash == to_state.hash {
        println!("  {} Environments are identical!", "✓".green());
        return Ok(());
    }

    // Compare runtimes
    let runtime_diff = diff_runtimes(&from_state.runtimes, &to_state.runtimes);
    if !runtime_diff.is_empty() {
        println!("  {}", "Runtime differences:".bold());
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
        println!("  {}", "Package differences:".bold());

        if !package_diff.added.is_empty() {
            println!(
                "    {} {} packages added:",
                "+".green(),
                package_diff.added.len()
            );
            for pkg in package_diff.added.iter().take(10) {
                println!("      {} {}", "+".green(), pkg);
            }
            if package_diff.added.len() > 10 {
                println!("      ... and {} more", package_diff.added.len() - 10);
            }
        }

        if !package_diff.removed.is_empty() {
            println!(
                "    {} {} packages removed:",
                "-".red(),
                package_diff.removed.len()
            );
            for pkg in package_diff.removed.iter().take(10) {
                println!("      {} {}", "-".red(), pkg);
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

    println!("  {}", "Summary:".bold());
    println!(
        "    Runtimes:  {} changes",
        if runtime_diff.is_empty() {
            "0".green().to_string()
        } else {
            runtime_diff.len().to_string().yellow().to_string()
        }
    );
    println!(
        "    Packages:  +{} -{} ~{}",
        package_diff.added.len().to_string().green(),
        package_diff.removed.len().to_string().red(),
        package_diff.changed.len().to_string().yellow()
    );
    println!();

    if total_changes > 0 {
        println!("  {} To sync to the target environment:", "Hint:".dimmed());
        println!("       {}", format!("omg env sync {to}").cyan());
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
                    runtime.yellow(),
                    from_ver.dimmed(),
                    to_ver.green()
                ));
            }
            (Some(from_ver), None) => {
                changes.push(format!(
                    "{} {} → {}",
                    "-".red(),
                    runtime,
                    format!("(removed, was {from_ver})").dimmed()
                ));
            }
            (None, Some(to_ver)) => {
                changes.push(format!(
                    "{} {} {}",
                    "+".green(),
                    runtime,
                    format!("(added @ {to_ver})").green()
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
    let from_set: HashSet<_> = from.iter().collect();
    let to_set: HashSet<_> = to.iter().collect();

    let added: Vec<_> = to_set.difference(&from_set).map(|s| (*s).clone()).collect();

    let removed: Vec<_> = from_set.difference(&to_set).map(|s| (*s).clone()).collect();

    PackageDiff {
        added,
        removed,
        changed: Vec::new(), // Version changes would need more data
    }
}
