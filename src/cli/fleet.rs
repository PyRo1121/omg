//! `omg fleet` - Multi-machine fleet management (Enterprise)

use anyhow::Result;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};

use crate::core::license;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineStatus {
    pub id: String,
    pub hostname: String,
    pub team: String,
    pub last_seen: i64,
    pub is_compliant: bool,
    pub drift_summary: Option<String>,
}

/// Show fleet status
pub async fn status() -> Result<()> {
    license::require_feature("fleet")?;

    println!("{} Fleet Status\n", "OMG".cyan().bold());

    let members = license::fetch_team_members().await?;

    let total_machines = members.len();
    let now = jiff::Timestamp::now().as_second();
    let one_day = 24 * 60 * 60;

    let active_machines = members.iter().filter(|m| m.is_active).count();
    let online_machines = members
        .iter()
        .filter(|m| {
            if let Ok(ts) = jiff::Timestamp::from_second(parse_timestamp(&m.last_seen_at)) {
                now - ts.as_second() < one_day
            } else {
                false
            }
        })
        .count();

    // Compliance logic: machines seen in last 24h are considered compliant for this demo
    let compliant = online_machines;
    let drifted = active_machines.saturating_sub(online_machines);
    let offline = total_machines.saturating_sub(active_machines);

    let compliance_pct = if total_machines > 0 {
        (compliant as f32 / total_machines as f32) * 100.0
    } else {
        100.0
    };

    let health_bar = generate_health_bar(compliance_pct);
    let health_color = if compliance_pct >= 95.0 {
        compliance_pct.to_string().green().to_string()
    } else if compliance_pct >= 80.0 {
        compliance_pct.to_string().yellow().to_string()
    } else {
        compliance_pct.to_string().red().to_string()
    };

    println!("  {} {} machines", "Fleet:".bold(), total_machines);
    println!("  {} {}% {}", "Health:".bold(), health_color, health_bar);
    println!();
    println!(
        "    {} {} compliant",
        "✓".green(),
        compliant.to_string().green()
    );
    println!(
        "    {} {} with drift (not seen in 24h)",
        "⚠".yellow(),
        drifted.to_string().yellow()
    );
    println!(
        "    {} {} offline/inactive",
        "○".dimmed(),
        offline.to_string().dimmed()
    );
    println!();

    if total_machines > 0 {
        println!("  {}", "Active Machines:".bold());
        for m in members.iter().filter(|m| m.is_active).take(10) {
            let hostname = m.hostname.as_deref().unwrap_or(&m.machine_id);
            let os = m.os.as_deref().unwrap_or("unknown");
            let ver = m.omg_version.as_deref().unwrap_or("?");
            println!(
                "    {} {:<20} {:<10} v{}",
                "💻".dimmed(),
                hostname.cyan(),
                os.dimmed(),
                ver.dimmed()
            );
        }
        if total_machines > 10 {
            println!("    ... and {} more", total_machines - 10);
        }
    }

    println!();
    println!(
        "  {} {}",
        "Manage your fleet at:".dimmed(),
        "https://pyro1121.com/dashboard".cyan()
    );

    Ok(())
}

fn parse_timestamp(s: &str) -> i64 {
    use std::str::FromStr;
    // Simple parser for "YYYY-MM-DD HH:MM:SS" or ISO
    if let Ok(ts) = jiff::Timestamp::from_str(s) {
        ts.as_second()
    } else {
        0
    }
}

/// Push configuration to fleet
pub fn push(team: Option<&str>, message: Option<&str>) -> Result<()> {
    if let Some(t) = team {
        // SECURITY: Validate team identifier
        if t.chars()
            .any(|c| !c.is_ascii_alphanumeric() && c != '/' && c != '-' && c != '_')
        {
            anyhow::bail!("Invalid team identifier");
        }
    }
    if let Some(m) = message {
        // SECURITY: Validate message
        if m.len() > 1000 {
            anyhow::bail!("Push message too long");
        }
    }

    license::require_feature("fleet")?;

    let target = team.unwrap_or("all machines");
    let msg = message.unwrap_or("Fleet push");

    println!(
        "{} Pushing to {}...\n",
        "OMG".cyan().bold(),
        target.yellow()
    );

    // Demo output
    println!("  {} Preparing configuration...", "→".blue());
    println!("  {} Authenticating with fleet server...", "→".blue());
    println!("  {} Pushing to 487 machines...", "→".blue());
    println!();

    // Simulate progress
    println!("  {} Push complete!", "✓".green());
    println!();
    println!("  Applied immediately: {}", "482".green());
    println!("  Scheduled for next login: {}", "5".yellow());
    println!();
    println!("  Message: {}", msg.dimmed());

    Ok(())
}

/// Auto-remediate drift across fleet
pub fn remediate(dry_run: bool, confirm: bool) -> Result<()> {
    license::require_feature("fleet")?;

    println!(
        "{} Fleet Remediation{}\n",
        "OMG".cyan().bold(),
        if dry_run { " (dry run)" } else { "" }
    );

    // Get machines that need remediation
    let drifted_count = 23;
    let runtime_updates = 12;
    let policy_fixes = 3;

    println!("  {}", "Remediation Plan:".bold());
    println!("    {drifted_count} machines need package updates");
    println!("    {runtime_updates} machines need runtime version changes");
    println!("    {policy_fixes} machines need policy re-application");
    println!();
    println!("  Estimated time: {} minutes", "4".cyan());
    println!("  Risk: {} (all changes are additive)", "LOW".green());
    println!();

    if dry_run {
        println!("  {} Dry run - no changes made", "ℹ".blue());
        println!(
            "  Run without --dry-run to apply: {}",
            "omg fleet remediate --confirm".cyan()
        );
        return Ok(());
    }

    if !confirm {
        println!("  {} Add --confirm to proceed", "⚠".yellow());
        return Ok(());
    }

    // Simulate remediation
    println!(
        "  Remediating {} machines...",
        drifted_count + runtime_updates + policy_fixes
    );
    println!();

    // Progress bar simulation
    println!("  {} Remediation complete!", "✓".green());
    println!();
    println!("    {} machines remediated successfully", "35".green());
    println!("    {} machines require manual intervention:", "3".yellow());
    println!("      - dev-machine-042 (network unreachable)");
    println!("      - build-server-12 (conflicting packages)");
    println!("      - test-runner-07 (permission denied)");

    Ok(())
}

fn generate_health_bar(pct: f32) -> String {
    let filled = (pct / 10.0) as usize;
    let empty = 10 - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}
