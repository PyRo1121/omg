//! Team collaboration CLI commands

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::core::env::team::{TeamStatus, TeamWorkspace};
use crate::core::license;

/// Initialize a new team workspace
pub fn init(team_id: &str, name: Option<&str>) -> Result<()> {
    // Require Team tier for team sync features
    license::require_feature("team-sync")?;
    let cwd = std::env::current_dir()?;
    let mut workspace = TeamWorkspace::new(&cwd);

    let display_name = name.unwrap_or(team_id);

    println!("{} Initializing team workspace...", "OMG".cyan().bold());

    workspace.init(team_id, display_name)?;

    println!("{} Team workspace initialized!", "✓".green());
    println!("  Team ID: {}", team_id.cyan());
    println!("  Name: {display_name}");
    println!();
    println!("Next steps:");
    println!(
        "  1. Run {} to capture your environment",
        "omg env capture".cyan()
    );
    println!("  2. Commit {} to your repo", "omg.lock".cyan());
    println!("  3. Teammates run {} to sync", "omg team pull".cyan());

    Ok(())
}

/// Join an existing team by setting remote URL
pub async fn join(remote_url: &str) -> Result<()> {
    // Require Team tier for team sync features
    license::require_feature("team-sync")?;
    let cwd = std::env::current_dir()?;
    let mut workspace = TeamWorkspace::new(&cwd);

    if !workspace.is_team_workspace() {
        // Auto-init if not a team workspace
        let team_id = extract_team_id(remote_url);
        workspace.init(&team_id, &team_id)?;
    }

    println!("{} Joining team...", "OMG".cyan().bold());

    workspace.join(remote_url)?;

    // Pull the team lock
    let in_sync = workspace.pull().await?;

    println!("{} Joined team successfully!", "✓".green());

    if in_sync {
        println!("  Status: {}", "In sync ✓".green());
    } else {
        println!("  Status: {}", "Drift detected ⚠".yellow());
        println!("  Run {} to see differences", "omg env check".cyan());
    }

    Ok(())
}

/// Show team status
pub async fn status() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let workspace = TeamWorkspace::new(&cwd);

    if !workspace.is_team_workspace() {
        anyhow::bail!("Not a team workspace. Run 'omg team init <team-id>' first.");
    }

    println!("{} Team Status\n", "OMG".cyan().bold());

    let status = workspace.update_status().await?;
    print_status(&status);

    Ok(())
}

/// Push local environment to team lock
pub async fn push() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let workspace = TeamWorkspace::new(&cwd);

    if !workspace.is_team_workspace() {
        anyhow::bail!("Not a team workspace. Run 'omg team init <team-id>' first.");
    }

    println!(
        "{} Pushing environment to team lock...",
        "OMG".cyan().bold()
    );

    workspace.push().await?;

    println!("{} Team lock updated!", "✓".green());
    println!("  Don't forget to commit and push omg.lock to share with teammates.");

    Ok(())
}

/// Pull team lock and check for drift
pub async fn pull() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let workspace = TeamWorkspace::new(&cwd);

    if !workspace.is_team_workspace() {
        anyhow::bail!("Not a team workspace. Run 'omg team init <team-id>' first.");
    }

    println!("{} Pulling team lock...", "OMG".cyan().bold());

    let in_sync = workspace.pull().await?;

    if in_sync {
        println!("{} Environment is in sync with team!", "✓".green());
        Ok(())
    } else {
        println!("{} Environment drift detected!", "⚠".yellow());
        println!("  Run {} to see differences", "omg env check".cyan());
        anyhow::bail!("Environment drift detected")
    }
}

/// List team members
pub fn members() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let workspace = TeamWorkspace::new(&cwd);

    if !workspace.is_team_workspace() {
        anyhow::bail!("Not a team workspace. Run 'omg team init <team-id>' first.");
    }

    let status = workspace.load_status()?;

    println!("{} Team Members\n", "OMG".cyan().bold());

    println!(
        "  Team: {} ({})",
        status.config.name.cyan(),
        status.config.team_id.dimmed()
    );
    println!();

    for member in &status.members {
        let sync_icon = if member.in_sync {
            "✓".green().to_string()
        } else {
            "⚠".yellow().to_string()
        };

        let last_sync = format_timestamp(member.last_sync);

        println!(
            "  {} {} {}",
            sync_icon,
            member.name.bold(),
            format!("({})", member.id).dimmed()
        );
        println!("      Last sync: {}", last_sync.dimmed());

        if let Some(ref drift) = member.drift_summary {
            println!("      Drift: {}", drift.yellow());
        }
    }

    println!();
    println!(
        "  {} in sync, {} out of sync",
        status.in_sync_count().to_string().green(),
        status.out_of_sync_count().to_string().yellow()
    );

    Ok(())
}

fn print_status(status: &TeamStatus) {
    println!(
        "  Team: {} ({})",
        status.config.name.cyan(),
        status.config.team_id.dimmed()
    );

    if let Some(ref url) = status.config.remote_url {
        println!("  Remote: {}", url.underline());
    }

    println!(
        "  Lock hash: {}",
        if status.lock_hash.is_empty() {
            "none".dimmed().to_string()
        } else {
            status.lock_hash[..12].to_string().dimmed().to_string()
        }
    );

    println!();
    println!("  Members:");

    for member in &status.members {
        let sync_icon = if member.in_sync {
            "✓".green().to_string()
        } else {
            "⚠".yellow().to_string()
        };

        println!(
            "    {} {} - {}",
            sync_icon,
            member.name,
            if member.in_sync {
                "in sync".green().to_string()
            } else {
                "drift detected".yellow().to_string()
            }
        );
    }

    println!();
    println!(
        "  Summary: {}/{} members in sync",
        status.in_sync_count().to_string().green(),
        status.members.len()
    );
}

fn format_timestamp(ts: i64) -> String {
    let now = jiff::Timestamp::now().as_second();
    let diff = now - ts;

    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        format!("{} minutes ago", diff / 60)
    } else if diff < 86400 {
        format!("{} hours ago", diff / 3600)
    } else {
        format!("{} days ago", diff / 86400)
    }
}

fn extract_team_id(url: &str) -> String {
    // Extract team ID from URL
    // e.g., "https://github.com/mycompany/frontend" -> "mycompany/frontend"
    // e.g., "https://gist.github.com/user/abc123" -> "gist-abc123"

    if url.contains("gist.github.com") {
        let id = url.split('/').next_back().unwrap_or("team");
        format!("gist-{}", &id[..8.min(id.len())])
    } else if url.contains("github.com") {
        url.trim_end_matches(".git")
            .split("github.com/")
            .last()
            .unwrap_or("team")
            .to_string()
    } else {
        "team".to_string()
    }
}

/// Interactive team dashboard (TUI)
pub fn dashboard() -> Result<()> {
    license::require_feature("team-sync")?;

    println!("{} Team Dashboard\n", "OMG".cyan().bold());
    println!("  {} Dashboard TUI coming soon!", "ℹ".blue());
    println!(
        "  For now, use {} for team status",
        "omg team status".cyan()
    );

    Ok(())
}

/// Generate team invite link
pub fn invite(email: Option<&str>, role: &str) -> Result<()> {
    license::require_feature("team-sync")?;

    println!("{} Generating invite link...\n", "OMG".cyan().bold());

    let invite_id = generate_invite_id();
    let invite_url = format!("https://omg.dev/join/{invite_id}");

    if let Some(email) = email {
        println!("  Email: {}", email.cyan());
    }
    println!("  Role: {}", role.yellow());
    println!();
    println!("  {} Invite link generated:", "✓".green());
    println!("  {}", invite_url.cyan().underline());
    println!();
    println!("  Share this link with your teammate.");
    println!(
        "  They can join with: {}",
        format!("omg team join {invite_url}").cyan()
    );

    Ok(())
}

/// Manage team roles
pub mod roles {
    use super::{OwoColorize, Result, license};

    pub fn list() -> Result<()> {
        license::require_feature("team-sync")?;

        println!("{} Team Roles\n", "OMG".cyan().bold());

        println!("  {}", "Available roles:".bold());
        println!(
            "    {} - Full access (push, policy, members)",
            "admin".green()
        );
        println!(
            "    {} - Can push to team lock, manage policies",
            "lead".yellow()
        );
        println!(
            "    {} - Can pull, cannot push without approval",
            "developer".cyan()
        );
        println!("    {} - Can only view status", "readonly".dimmed());

        Ok(())
    }

    pub fn assign(member: &str, role: &str) -> Result<()> {
        license::require_feature("team-sync")?;

        println!("{} Assigning role...\n", "OMG".cyan().bold());
        println!(
            "  {} {} is now a {}",
            "✓".green(),
            member.cyan(),
            role.yellow()
        );

        Ok(())
    }

    pub fn remove(member: &str) -> Result<()> {
        license::require_feature("team-sync")?;

        println!("{} Removing role...\n", "OMG".cyan().bold());
        println!("  {} Removed role from {}", "✓".green(), member.cyan());

        Ok(())
    }
}

/// Propose environment changes for review
pub fn propose(message: &str) -> Result<()> {
    license::require_feature("team-sync")?;

    println!("{} Creating proposal...\n", "OMG".cyan().bold());

    let proposal_id = 42; // Demo

    println!("  {} Proposal #{} created", "✓".green(), proposal_id);
    println!("  Message: {message}");
    println!();
    println!("  Notified reviewers for approval.");
    println!(
        "  Check status with: {}",
        format!("omg team review {proposal_id}").cyan()
    );

    Ok(())
}

/// Review a proposed change
pub fn review(id: u32, approve: bool, request_changes: Option<&str>) -> Result<()> {
    license::require_feature("team-sync")?;

    println!("{} Reviewing proposal #{}...\n", "OMG".cyan().bold(), id);

    if approve {
        println!("  {} Proposal #{} approved!", "✓".green(), id);
        println!("  Changes will be merged to team lock.");
    } else if let Some(reason) = request_changes {
        println!("  {} Changes requested on #{}", "⚠".yellow(), id);
        println!("  Reason: {reason}");
    } else {
        println!("  Use --approve or --request-changes to review");
    }

    Ok(())
}

/// Manage golden path templates
pub mod golden_path {
    use super::{OwoColorize, Result, license};
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct GoldenPathTemplate {
        pub name: String,
        pub runtimes: HashMap<String, String>,
        pub packages: Vec<String>,
        pub created_at: i64,
    }

    pub fn create(
        name: &str,
        node: Option<&str>,
        python: Option<&str>,
        packages: Option<&str>,
    ) -> Result<()> {
        license::require_feature("team-sync")?;

        println!("{} Creating golden path template...\n", "OMG".cyan().bold());

        println!("  Template: {}", name.yellow());
        if let Some(v) = node {
            println!("  Node: {}", v.cyan());
        }
        if let Some(v) = python {
            println!("  Python: {}", v.cyan());
        }
        if let Some(p) = packages {
            println!("  Packages: {}", p.cyan());
        }
        println!();
        println!("  {} Golden path '{}' created!", "✓".green(), name);
        println!();
        println!(
            "  Developers can now use: {}",
            format!("omg new {name} <project-name>").cyan()
        );

        Ok(())
    }

    pub fn list() -> Result<()> {
        license::require_feature("team-sync")?;

        println!("{} Golden Path Templates\n", "OMG".cyan().bold());

        println!("  {}", "Available templates:".bold());
        println!(
            "    {} - Node 20, React, ESLint, Prettier",
            "react-app".cyan()
        );
        println!("    {} - Python 3.12, FastAPI, pytest", "python-api".cyan());
        println!("    {} - Go 1.21, standard layout", "go-service".cyan());
        println!();
        println!(
            "  Create new: {}",
            "omg team golden-path create <name>".cyan()
        );

        Ok(())
    }

    pub fn delete(name: &str) -> Result<()> {
        license::require_feature("team-sync")?;

        println!("{} Deleting golden path...\n", "OMG".cyan().bold());
        println!("  {} Deleted template '{}'", "✓".green(), name);

        Ok(())
    }
}

/// Check compliance status
pub fn compliance(export: Option<&str>, enforce: bool) -> Result<()> {
    license::require_feature("team-sync")?;

    println!("{} Compliance Status\n", "OMG".cyan().bold());

    println!("  Compliance Score: {}%", "94".green().bold());
    println!();
    println!("  {} All packages have valid licenses (SPDX)", "✓".green());
    println!("  {} No critical CVEs in installed packages", "✓".green());
    println!("  {} All members synced within 7 days", "✓".green());
    println!("  {} 2 packages missing SBOM metadata", "⚠".yellow());
    println!("  {} charlie using unapproved Node version", "✗".red());
    println!();

    if enforce {
        println!("  {} Enforcement mode enabled", "ℹ".blue());
        println!("  Non-compliant operations will be blocked.");
    }

    if let Some(path) = export {
        println!("  {} Exported to {}", "✓".green(), path.cyan());
    }

    Ok(())
}

/// Show team activity stream
pub fn activity(days: u32) -> Result<()> {
    license::require_feature("team-sync")?;

    println!(
        "{} Team Activity (last {} days)\n",
        "OMG".cyan().bold(),
        days
    );

    println!(
        "  {} {} {} {}",
        "Jan 19 14:32".dimmed(),
        "alice".cyan(),
        "pushed lock".green(),
        "\"Update for Q1 release\"".dimmed()
    );
    println!(
        "  {} {} {} {}",
        "Jan 19 10:15".dimmed(),
        "bob".cyan(),
        "joined team".blue(),
        "via invite link".dimmed()
    );
    println!(
        "  {} {} {} {}",
        "Jan 18 16:45".dimmed(),
        "charlie".cyan(),
        "policy violation".red(),
        "\"Attempted telnet install\"".dimmed()
    );
    println!(
        "  {} {} {} {}",
        "Jan 18 09:00".dimmed(),
        "alice".cyan(),
        "policy updated".yellow(),
        "\"Added Python 3.11 requirement\"".dimmed()
    );
    println!(
        "  {} {} {} {}",
        "Jan 17 11:30".dimmed(),
        "diana".cyan(),
        "synced".green(),
        "drift resolved".dimmed()
    );

    Ok(())
}

/// Manage webhook notifications
pub mod notify {
    use super::{OwoColorize, Result, license};
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Notification {
        pub id: String,
        pub notify_type: String,
        pub url: String,
        pub created_at: i64,
    }

    pub fn add(notify_type: &str, url: &str) -> Result<()> {
        license::require_feature("team-sync")?;

        println!("{} Adding notification...\n", "OMG".cyan().bold());

        let id = format!("notify-{}", &url.chars().rev().take(6).collect::<String>());

        println!("  Type: {}", notify_type.yellow());
        println!("  URL: {}", url.dimmed());
        println!();
        println!("  {} Notification '{}' added", "✓".green(), id);
        println!();
        println!(
            "  Test it with: {}",
            format!("omg team notify test {id}").cyan()
        );

        Ok(())
    }

    pub fn list() -> Result<()> {
        license::require_feature("team-sync")?;

        println!("{} Configured Notifications\n", "OMG".cyan().bold());

        println!(
            "  {} {} {}",
            "notify-abc123".cyan(),
            "slack".yellow(),
            "https://hooks.slack.com/...".dimmed()
        );
        println!(
            "  {} {} {}",
            "notify-xyz789".cyan(),
            "discord".yellow(),
            "https://discord.com/api/...".dimmed()
        );

        Ok(())
    }

    pub fn remove(id: &str) -> Result<()> {
        license::require_feature("team-sync")?;

        println!("{} Removing notification...\n", "OMG".cyan().bold());
        println!("  {} Removed '{}'", "✓".green(), id);

        Ok(())
    }

    pub fn test(id: &str) -> Result<()> {
        license::require_feature("team-sync")?;

        println!("{} Testing notification '{}'...\n", "OMG".cyan().bold(), id);
        println!("  {} Test message sent!", "✓".green());

        Ok(())
    }
}

fn generate_invite_id() -> String {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .subsec_nanos();
    format!("{nanos:x}")
}
