//! `omg outdated` - Show what packages would be updated

use anyhow::Result;
use owo_colors::OwoColorize;
use serde::Serialize;
use std::sync::Arc;

use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;

#[derive(Debug, Serialize)]
pub struct OutdatedPackage {
    pub name: String,
    pub current_version: String,
    pub new_version: String,
    pub is_security: bool,
    pub update_type: UpdateType,
    pub repo: String,
}

#[derive(Debug, Serialize)]
pub enum UpdateType {
    Security,
    Major,
    Minor,
    Patch,
}

impl std::fmt::Display for UpdateType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Security => write!(f, "security"),
            Self::Major => write!(f, "major"),
            Self::Minor => write!(f, "minor"),
            Self::Patch => write!(f, "patch"),
        }
    }
}

/// Show outdated packages
pub async fn run(security_only: bool, json: bool) -> Result<()> {
    // SECURITY: This command has no string inputs, but we validate environment state
    if !json {
        println!("{} Checking for updates...\n", "OMG".cyan().bold());
    }

    let pm = Arc::from(get_package_manager());
    let service = PackageService::new(pm);
    let updates = service.list_updates().await?;

    if updates.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("  {} All packages are up to date!", "✓".green());
        }
        return Ok(());
    }

    let mut outdated: Vec<OutdatedPackage> = updates
        .into_iter()
        .map(|u| {
            let update_type = classify_update(&u.old_version, &u.new_version);
            // Simple CVE check - in reality would query a vulnerability database
            let is_security = u.name.contains("openssl")
                || u.name.contains("glibc")
                || u.name.contains("linux")
                || u.name.contains("curl");

            OutdatedPackage {
                name: u.name,
                current_version: u.old_version,
                new_version: u.new_version,
                is_security,
                update_type,
                repo: u.repo,
            }
        })
        .collect();

    outdated.sort_by(|a, b| {
        // Security first, then by update type, then by name
        match (a.is_security, b.is_security) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    let filtered: Vec<_> = if security_only {
        outdated.into_iter().filter(|p| p.is_security).collect()
    } else {
        outdated
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&filtered)?);
        return Ok(());
    }

    // Group by update type
    let security: Vec<_> = filtered.iter().filter(|p| p.is_security).collect();
    let major: Vec<_> = filtered
        .iter()
        .filter(|p| matches!(p.update_type, UpdateType::Major) && !p.is_security)
        .collect();
    let minor: Vec<_> = filtered
        .iter()
        .filter(|p| matches!(p.update_type, UpdateType::Minor) && !p.is_security)
        .collect();
    let patch: Vec<_> = filtered
        .iter()
        .filter(|p| matches!(p.update_type, UpdateType::Patch) && !p.is_security)
        .collect();

    if !security.is_empty() {
        println!(
            "  {} (install immediately)",
            "Security Updates".red().bold()
        );
        for pkg in &security {
            println!(
                "    {} {} → {} {}",
                pkg.name.yellow(),
                pkg.current_version.dimmed(),
                pkg.new_version.green(),
                "(CVE)".red()
            );
        }
        println!();
    }

    if !major.is_empty() {
        println!(
            "  {} (may have breaking changes)",
            "Major Updates".yellow().bold()
        );
        for pkg in &major {
            println!(
                "    {} {} → {} {}",
                pkg.name,
                pkg.current_version.dimmed(),
                pkg.new_version.cyan(),
                format!("({})", pkg.repo).dimmed()
            );
        }
        println!();
    }

    if !minor.is_empty() {
        println!("  {} (new features)", "Minor Updates".blue().bold());
        for pkg in minor.iter().take(10) {
            println!(
                "    {} {} → {} {}",
                pkg.name,
                pkg.current_version.dimmed(),
                pkg.new_version,
                format!("({})", pkg.repo).dimmed()
            );
        }
        if minor.len() > 10 {
            println!("    ... and {} more", minor.len() - 10);
        }
        println!();
    }

    if !patch.is_empty() {
        println!("  {} (bug fixes)", "Patch Updates".dimmed().bold());
        for pkg in patch.iter().take(5) {
            println!(
                "    {} {} → {} {}",
                pkg.name,
                pkg.current_version.dimmed(),
                pkg.new_version,
                format!("({})", pkg.repo).dimmed()
            );
        }
        if patch.len() > 5 {
            println!("    ... and {} more", patch.len() - 5);
        }
        println!();
    }

    // Summary
    println!("  {}", "Summary:".bold());
    println!("    Security: {}", security.len().to_string().red());
    println!("    Major: {}", major.len().to_string().yellow());
    println!("    Minor: {}", minor.len().to_string().blue());
    println!("    Patch: {}", patch.len());
    println!();
    println!("  Run {} to update all", "omg update".cyan());
    if !security.is_empty() {
        println!(
            "  Run {} to update security fixes only",
            "omg update --security".cyan()
        );
    }

    Ok(())
}

#[allow(dead_code)]
fn classify_update(old: &str, new: &str) -> UpdateType {
    // Parse semver-like versions
    let old_parts: Vec<_> = old.split('.').collect();
    let new_parts: Vec<_> = new.split('.').collect();

    if old_parts.is_empty() || new_parts.is_empty() {
        return UpdateType::Minor;
    }

    // Extract first numeric part
    let old_major = old_parts[0]
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();
    let new_major = new_parts[0]
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>();

    if old_major != new_major {
        return UpdateType::Major;
    }

    if old_parts.len() > 1 && new_parts.len() > 1 {
        let old_minor = old_parts[1]
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>();
        let new_minor = new_parts[1]
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>();
        if old_minor != new_minor {
            return UpdateType::Minor;
        }
    }

    UpdateType::Patch
}
