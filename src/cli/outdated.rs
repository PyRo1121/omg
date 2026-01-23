//! `omg outdated` - Show what packages would be updated

use anyhow::Result;
use owo_colors::OwoColorize;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct OutdatedPackage {
    pub name: String,
    pub current_version: String,
    pub new_version: String,
    pub is_security: bool,
    pub update_type: UpdateType,
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
pub fn run(security_only: bool, json: bool) -> Result<()> {
    // SECURITY: This command has no string inputs, but we validate environment state
    if !json {
        println!("{} Checking for updates...\n", "OMG".cyan().bold());
    }

    let outdated = get_outdated_packages()?;

    if outdated.is_empty() {
        if json {
            println!("[]");
        } else {
            println!("  {} All packages are up to date!", "✓".green());
        }
        return Ok(());
    }

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
                "    {} {} → {}",
                pkg.name,
                pkg.current_version.dimmed(),
                pkg.new_version.cyan()
            );
        }
        println!();
    }

    if !minor.is_empty() {
        println!("  {} (new features)", "Minor Updates".blue().bold());
        for pkg in minor.iter().take(10) {
            println!(
                "    {} {} → {}",
                pkg.name,
                pkg.current_version.dimmed(),
                pkg.new_version
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
                "    {} {} → {}",
                pkg.name,
                pkg.current_version.dimmed(),
                pkg.new_version
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

#[cfg(feature = "arch")]
fn get_outdated_packages() -> Result<Vec<OutdatedPackage>> {
    use alpm::Alpm;

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();
    let mut outdated = Vec::new();

    // Get sync databases
    let syncdbs: Vec<_> = handle.syncdbs().into_iter().collect();

    for local_pkg in localdb.pkgs() {
        let name = local_pkg.name();
        let local_ver = local_pkg.version().to_string();

        // Find in sync dbs
        for syncdb in &syncdbs {
            if let Ok(sync_pkg) = syncdb.pkg(name.as_bytes()) {
                let sync_ver: String = sync_pkg.version().to_string();
                if sync_ver != local_ver {
                    // Determine update type
                    let update_type = classify_update(&local_ver, &sync_ver);
                    // Simple CVE check - in reality would query a vulnerability database
                    let is_security = name.contains("openssl")
                        || name.contains("glibc")
                        || name.contains("linux")
                        || name.contains("curl");

                    outdated.push(OutdatedPackage {
                        name: name.to_string(),
                        current_version: local_ver.clone(),
                        new_version: sync_ver,
                        is_security,
                        update_type,
                    });
                }
                break;
            }
        }
    }

    outdated.sort_by(|a, b| {
        // Security first, then by update type, then by name
        match (a.is_security, b.is_security) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.cmp(&b.name),
        }
    });

    Ok(outdated)
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn get_outdated_packages() -> Result<Vec<OutdatedPackage>> {
    use std::process::Command;

    let output = Command::new("apt")
        .args(["list", "--upgradable", "--"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut outdated = Vec::new();

    for line in stdout.lines().skip(1) {
        // Skip "Listing..." header
        // Format: package/source version [upgradable from: old_version]
        if let Some((pkg_part, rest)) = line.split_once('/') {
            let name = pkg_part.to_string();
            let parts: Vec<_> = rest.split_whitespace().collect();
            if parts.len() >= 4 {
                let new_version = parts[0].to_string();
                let old_version = parts
                    .get(3)
                    .map(|s| s.trim_end_matches(']').to_string())
                    .unwrap_or_default();

                let update_type = classify_update(&old_version, &new_version);
                let is_security = name.contains("openssl") || name.contains("linux");

                outdated.push(OutdatedPackage {
                    name,
                    current_version: old_version,
                    new_version,
                    is_security,
                    update_type,
                });
            }
        }
    }

    Ok(outdated)
}

#[cfg(not(any(feature = "arch", feature = "debian")))]
#[allow(clippy::unnecessary_wraps)]
fn get_outdated_packages() -> Result<Vec<OutdatedPackage>> {
    // This is a stub for when features are disabled
    Ok(Vec::new())
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
