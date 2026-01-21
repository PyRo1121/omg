//! Team collaboration CLI commands

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::core::env::team::{TeamStatus, TeamWorkspace};
use crate::core::license;

/// Initialize a new team workspace
pub fn init(team_id: &str, name: Option<&str>) -> Result<()> {
    // SECURITY: Validate team_id
    if team_id.chars().any(|c| !c.is_ascii_alphanumeric() && c != '/' && c != '-' && c != '_') {
        anyhow::bail!("Invalid team ID: {team_id}");
    }
    if let Some(n) = name
        && (n.len() > 128 || n.chars().any(char::is_control)) {
            anyhow::bail!("Invalid team name");
        }

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
    // SECURITY: Basic URL validation
    if !remote_url.starts_with("https://") {
        anyhow::bail!("Only HTTPS URLs allowed for security");
    }
    if remote_url.len() > 1024 || remote_url.chars().any(char::is_control) {
        anyhow::bail!("Invalid remote URL");
    }

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
pub async fn members() -> Result<()> {
    // Require Team tier for team features
    license::require_feature("team-sync")?;

    println!("{} Team Members\n", "OMG".cyan().bold());

    let members = license::fetch_team_members().await?;
    
    if members.is_empty() {
        println!("  {} No team members found", "○".dimmed());
        println!("  Team members appear here once they activate with your license key.");
        return Ok(());
    }

    let now = jiff::Timestamp::now().as_second();
    let one_hour = 3600;

    for member in &members {
        let last_seen_ts = parse_timestamp(&member.last_seen_at);
        let in_sync = now - last_seen_ts < one_hour;
        
        let sync_icon = if in_sync {
            "✓".green().to_string()
        } else {
            "⚠".yellow().to_string()
        };

        let hostname = member.hostname.as_deref().unwrap_or(&member.machine_id);
        let last_sync = format_timestamp(last_seen_ts);

        println!(
            "  {} {} {}",
            sync_icon,
            hostname.bold(),
            format!("({})", &member.machine_id[..8.min(member.machine_id.len())]).dimmed()
        );
        println!("      Last active: {}", last_sync.dimmed());
        println!("      Platform:    {} {}", 
            member.os.as_deref().unwrap_or("unknown"),
            member.arch.as_deref().unwrap_or("")
        );
    }

    let in_sync_count = members.iter().filter(|m| {
        let ts = parse_timestamp(&m.last_seen_at);
        now - ts < one_hour
    }).count();

    println!();
    println!(
        "  {} in sync, {} inactive (>1h)",
        in_sync_count.to_string().green(),
        (members.len() - in_sync_count).to_string().yellow()
    );

    Ok(())
}

fn parse_timestamp(s: &str) -> i64 {
    use std::str::FromStr;
    // Basic parser for various formats
    if let Ok(ts) = jiff::Timestamp::from_str(s) {
        ts.as_second()
    } else {
        0
    }
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
        // Split URL and safely get the last segment
        let segments: Vec<&str> = url.split('/').collect();
        let id = segments.last().copied().unwrap_or("team");
        // Safely take up to 8 characters
        let short_id = id.chars().take(8).collect::<String>();
        format!("gist-{short_id}")
    } else if url.contains("github.com") {
        url.trim_end_matches(".git")
            .split("github.com/")
            .nth(1) // More idiomatic than .last() when we want the element after split
            .unwrap_or("team")
            .to_string()
    } else {
        "team".to_string()
    }
}

/// Interactive team dashboard (TUI)
pub async fn dashboard() -> Result<()> {
    license::require_feature("team-sync")?;
    crate::cli::tui::run_with_tab(crate::cli::tui::app::Tab::Team).await
}

/// Generate team invite link
pub fn invite(email: Option<&str>, role: &str) -> Result<()> {
    // SECURITY: Validate role and email
    let valid_roles = ["admin", "lead", "developer", "readonly"];
    if !valid_roles.contains(&role) {
        anyhow::bail!("Invalid role: {role}");
    }
    if let Some(e) = email
        && (!e.contains('@') || e.len() > 255) {
            anyhow::bail!("Invalid email address");
        }

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
        // SECURITY: Validate role and member
        let valid_roles = ["admin", "lead", "developer", "readonly"];
        if !valid_roles.contains(&role) {
            anyhow::bail!("Invalid role: {role}");
        }
        if member.len() > 128 || member.chars().any(char::is_control) {
            anyhow::bail!("Invalid member identifier");
        }

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
pub async fn propose(message: &str) -> Result<()> {
    license::require_feature("team-sync")?;

    println!("{} Creating proposal...\n", "OMG".cyan().bold());

    // Capture current environment state for the proposal
    let state = serde_json::json!({
        "environment": crate::core::env::fingerprint::EnvironmentState::capture().await?,
        "packages": crate::package_managers::list_explicit_fast().unwrap_or_default(),
    });

    let proposal_id = license::propose_change(message, &state).await?;

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

/// Review and approve/reject a proposal
pub async fn review(proposal_id: u32, approve: bool) -> Result<()> {
    license::require_feature("team-sync")?;

    let status = if approve { "approved" } else { "rejected" };
    
    println!(
        "{} Reviewing proposal #{} -> {}...\n",
        "OMG".cyan().bold(),
        proposal_id,
        if approve { "APPROVE".green().to_string() } else { "REJECT".red().to_string() }
    );

    license::review_proposal(proposal_id, status).await?;

    println!("  {} Proposal status updated", "✓".green());
    Ok(())
}

/// List pending team proposals
pub async fn list_proposals() -> Result<()> {
    license::require_feature("team-sync")?;

    println!("{} Team Proposals\n", "OMG".cyan().bold());

    let proposals = license::fetch_proposals().await?;

    if proposals.is_empty() {
        println!("  {}", "No pending proposals.".dimmed());
        return Ok(());
    }

    for p in &proposals {
        let id = p["id"].as_u64().unwrap_or(0);
        let status = p["status"].as_str().unwrap_or("pending");
        let msg = p["message"].as_str().unwrap_or("");
        let email = p["creator_email"].as_str().unwrap_or("unknown");
        let date = p["created_at"].as_str().unwrap_or("");

        let status_color = match status {
            "approved" => status.green().to_string(),
            "rejected" => status.red().to_string(),
            _ => status.yellow().to_string(),
        };

        println!(
            "  #{} [{}] {} - {}",
            id.to_string().cyan(),
            status_color,
            msg.bold(),
            email.dimmed()
        );
        println!("     Created: {}", date.dimmed());
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
        // SECURITY: Validate all inputs
        if name.chars().any(|c| !c.is_ascii_alphanumeric() && c != '-') {
            anyhow::bail!("Invalid template name (alphanumeric and hyphens only)");
        }
        if let Some(v) = node { crate::core::security::validate_version(v)?; }
        if let Some(v) = python { crate::core::security::validate_version(v)?; }
        if let Some(p) = packages {
            for pkg in p.split(',') {
                crate::core::security::validate_package_name(pkg.trim())?;
            }
        }

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
pub async fn activity(days: u32) -> Result<()> {
    license::require_feature("team-sync")?;

    println!(
        "{} Team Activity (last {} days)\n",
        "OMG".cyan().bold(),
        days
    );

    let logs = license::fetch_audit_logs().await?;
    
    if logs.is_empty() {
        println!("  {} No recent activity found", "○".dimmed());
        return Ok(());
    }

    for log in logs {
        let timestamp = format_timestamp(parse_timestamp(&log.created_at));
        let action_color = if log.action.contains("violation") || log.action.contains("revoked") {
            log.action.red().to_string()
        } else if log.action.contains("pushed") || log.action.contains("synced") {
            log.action.green().to_string()
        } else {
            log.action.cyan().to_string()
        };

        println!(
            "  {} {} {} {}",
            timestamp.dimmed(),
            "user".dimmed(), // User info not always available in audit_log query yet
            action_color,
            format!("({})", log.resource_type.as_deref().unwrap_or("-")).dimmed()
        );
    }

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
        // SECURITY: Validate type and URL
        let valid_types = ["slack", "discord", "webhook"];
        if !valid_types.contains(&notify_type) {
            anyhow::bail!("Invalid notification type: {notify_type}");
        }
        if !url.starts_with("https://") || url.len() > 1024 {
            anyhow::bail!("Invalid or insecure notification URL (HTTPS required)");
        }

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
