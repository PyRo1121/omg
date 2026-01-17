//! Team collaboration CLI commands

use anyhow::Result;
use owo_colors::OwoColorize;

use crate::core::env::team::{TeamStatus, TeamWorkspace};
use crate::core::license;

/// Initialize a new team workspace
pub async fn init(team_id: &str, name: Option<&str>) -> Result<()> {
    // Require Team tier for team sync features
    license::require_feature("team-sync")?;
    let cwd = std::env::current_dir()?;
    let mut workspace = TeamWorkspace::new(&cwd);

    let display_name = name.unwrap_or(team_id);

    println!("{} Initializing team workspace...", "OMG".cyan().bold());

    workspace.init(team_id, display_name)?;

    println!("{} Team workspace initialized!", "✓".green());
    println!("  Team ID: {}", team_id.cyan());
    println!("  Name: {}", display_name);
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
    } else {
        println!("{} Environment drift detected!", "⚠".yellow());
        println!("  Run {} to see differences", "omg env check".cyan());
        std::process::exit(1);
    }

    Ok(())
}

/// List team members
pub async fn members() -> Result<()> {
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
        let id = url.split('/').last().unwrap_or("team");
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
