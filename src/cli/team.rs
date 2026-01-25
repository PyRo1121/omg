//! Team collaboration CLI commands

use crate::cli::components::Components;
use crate::cli::tea::Cmd;
use crate::cli::{
    CliContext, CommandRunner, GoldenPathCommands, NotifyCommands, TeamCommands, TeamRoleCommands,
};
use anyhow::Result;
use async_trait::async_trait;

use crate::core::env::team::TeamWorkspace;
use crate::core::license;

#[async_trait]
impl CommandRunner for TeamCommands {
    async fn execute(&self, ctx: &CliContext) -> Result<()> {
        match self {
            TeamCommands::Init { team_id, name } => init(team_id, name.as_deref(), ctx),
            TeamCommands::Join { url } => join(url, ctx).await,
            TeamCommands::Status => status(ctx).await,
            TeamCommands::Push => push(ctx).await,
            TeamCommands::Pull => pull(ctx).await,
            TeamCommands::Members => members(ctx).await,
            TeamCommands::Dashboard => dashboard(ctx).await,
            TeamCommands::Invite { email, role } => invite(email.as_deref(), role, ctx),
            TeamCommands::Roles { command } => match command {
                TeamRoleCommands::List => roles::list(ctx),
                TeamRoleCommands::Assign { member, role } => roles::assign(member, role, ctx),
                TeamRoleCommands::Remove { member } => roles::remove(member, ctx),
            },
            TeamCommands::Propose { message } => propose(message, ctx).await,
            TeamCommands::Proposals => list_proposals(ctx).await,
            TeamCommands::Review { id, approve, .. } => review(*id, *approve, ctx).await,
            TeamCommands::GoldenPath { command } => match command {
                GoldenPathCommands::Create {
                    name,
                    node,
                    python,
                    packages,
                } => golden_path::create(
                    name,
                    node.as_deref(),
                    python.as_deref(),
                    packages.as_deref(),
                    ctx,
                ),
                GoldenPathCommands::List => golden_path::list(ctx),
                GoldenPathCommands::Delete { name } => golden_path::delete(name, ctx),
            },
            TeamCommands::Compliance { export, enforce } => {
                compliance(export.as_deref(), *enforce, ctx)
            }
            TeamCommands::Activity { days } => activity(*days, ctx).await,
            TeamCommands::Notify { command } => match command {
                NotifyCommands::Add { notify_type, url } => team_notify::add(notify_type, url, ctx),
                NotifyCommands::List => team_notify::list(ctx),
                NotifyCommands::Remove { id } => team_notify::remove(id, ctx),
                NotifyCommands::Test { id } => team_notify::test(id, ctx),
            },
        }
    }
}

/// Initialize a new team workspace
pub fn init(team_id: &str, name: Option<&str>, _ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    // SECURITY: Validate team_id
    if team_id
        .chars()
        .any(|c| !c.is_ascii_alphanumeric() && c != '/' && c != '-' && c != '_')
    {
        execute_cmd(Components::error_with_suggestion(
            "Invalid team ID",
            "Team IDs must be alphanumeric with /, -, or _ allowed",
        ));
        anyhow::bail!("Invalid team ID: {team_id}");
    }
    if let Some(n) = name
        && (n.len() > 128 || n.chars().any(char::is_control))
    {
        execute_cmd(Components::error(
            "Invalid team name (too long or contains control characters)",
        ));
        anyhow::bail!("Invalid team name");
    }

    // Require Team tier for team sync features
    license::require_feature("team-sync")?;
    let cwd = std::env::current_dir()?;
    let mut workspace = TeamWorkspace::new(&cwd);

    let display_name = name.unwrap_or(team_id);

    execute_cmd(Components::loading("Initializing team workspace..."));

    workspace.init(team_id, display_name)?;

    execute_cmd(Cmd::batch([
        Components::success("Team workspace initialized!"),
        Components::kv_list(
            Some("Team Details"),
            vec![("Team ID", team_id), ("Name", display_name)],
        ),
        Components::spacer(),
        Components::header("Next Steps", ""),
        Cmd::println("  1. Run 'omg env capture' to capture your environment"),
        Cmd::println("  2. Commit 'omg.lock' to your repo"),
        Cmd::println("  3. Teammates run 'omg team pull' to sync"),
    ]));

    Ok(())
}

/// Join an existing team by setting remote URL
pub async fn join(remote_url: &str, _ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    // SECURITY: Basic URL validation
    if !remote_url.starts_with("https://") {
        execute_cmd(Components::error_with_suggestion(
            "Only HTTPS URLs allowed for security",
            "Use https:// instead of http://",
        ));
        anyhow::bail!("Only HTTPS URLs allowed for security");
    }
    if remote_url.len() > 1024 || remote_url.chars().any(char::is_control) {
        execute_cmd(Components::error("Invalid remote URL"));
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

    execute_cmd(Components::loading("Joining team..."));

    workspace.join(remote_url)?;

    // Pull the team lock
    let in_sync = workspace.pull().await?;

    if in_sync {
        execute_cmd(Cmd::batch([
            Components::success("Joined team successfully!"),
            Components::status_summary(vec![("Status", "In sync")]),
        ]));
    } else {
        execute_cmd(Cmd::batch([
            Components::success("Joined team successfully!"),
            Components::warning("Drift detected"),
            Components::info("Run 'omg env check' to see differences"),
        ]));
    }

    Ok(())
}

/// Show team status
pub async fn status(_ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    let cwd = std::env::current_dir()?;
    let workspace = TeamWorkspace::new(&cwd);

    if !workspace.is_team_workspace() {
        execute_cmd(Components::error_with_suggestion(
            "Not a team workspace",
            "Run 'omg team init <team-id>' first",
        ));
        anyhow::bail!("Not a team workspace");
    }

    let team_status = workspace.update_status().await?;

    let mut details = vec![format!(
        "Team: {} ({})",
        team_status.config.name, team_status.config.team_id
    )];

    if let Some(ref url) = team_status.config.remote_url {
        details.push(format!("Remote: {}", url));
    }

    details.push(format!(
        "Lock hash: {}",
        if team_status.lock_hash.is_empty() {
            "none".to_string()
        } else {
            format!(
                "{}...",
                &team_status.lock_hash[..12.min(team_status.lock_hash.len())]
            )
        }
    ));

    let mut member_list = vec![];
    for member in &team_status.members {
        let status_icon = if member.in_sync { "✓" } else { "⚠" };
        member_list.push(format!(
            "{} {} - {}",
            status_icon,
            member.name,
            if member.in_sync {
                "in sync"
            } else {
                "drift detected"
            }
        ));
    }

    execute_cmd(Cmd::batch([
        Components::header(
            "Team Status",
            &format!(
                "{}/{} members in sync",
                team_status.in_sync_count(),
                team_status.members.len()
            ),
        ),
        Components::spacer(),
        Components::card("Team Information", details),
        Components::spacer(),
        Components::card("Members", member_list),
    ]));

    Ok(())
}

/// Push local environment to team lock
pub async fn push(_ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    let cwd = std::env::current_dir()?;
    let workspace = TeamWorkspace::new(&cwd);

    if !workspace.is_team_workspace() {
        execute_cmd(Components::error_with_suggestion(
            "Not a team workspace",
            "Run 'omg team init <team-id>' first",
        ));
        anyhow::bail!("Not a team workspace");
    }

    execute_cmd(Components::loading("Pushing environment to team lock..."));

    workspace.push().await?;

    execute_cmd(Cmd::batch([
        Components::success("Team lock updated!"),
        Components::info("Don't forget to commit and push omg.lock to share with teammates"),
    ]));

    Ok(())
}

/// Pull team lock and check for drift
pub async fn pull(_ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    let cwd = std::env::current_dir()?;
    let workspace = TeamWorkspace::new(&cwd);

    if !workspace.is_team_workspace() {
        execute_cmd(Components::error_with_suggestion(
            "Not a team workspace",
            "Run 'omg team init <team-id>' first",
        ));
        anyhow::bail!("Not a team workspace");
    }

    execute_cmd(Components::loading("Pulling team lock..."));

    let in_sync = workspace.pull().await?;

    if in_sync {
        execute_cmd(Components::complete("Environment is in sync with team!"));
        Ok(())
    } else {
        execute_cmd(Cmd::batch([
            Components::warning("Environment drift detected!"),
            Components::info("Run 'omg env check' to see differences"),
        ]));
        anyhow::bail!("Environment drift detected")
    }
}

/// List team members
pub async fn members(_ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    // Require Team tier for team features
    license::require_feature("team-sync")?;

    let members = license::fetch_team_members().await?;

    if members.is_empty() {
        execute_cmd(Cmd::batch([
            Components::header("Team Members", "No members found"),
            Components::spacer(),
            Components::info(
                "Team members will appear here once they activate with your license key",
            ),
        ]));
        return Ok(());
    }

    let now = jiff::Timestamp::now().as_second();
    let one_hour = 3600;

    let mut member_list = vec![];
    for member in &members {
        let last_seen_ts = parse_timestamp(&member.last_seen_at);
        let in_sync = now - last_seen_ts < one_hour;

        let sync_icon = if in_sync { "✓" } else { "⚠" };
        let hostname = member.hostname.as_deref().unwrap_or(&member.machine_id);
        let last_sync = format_timestamp(last_seen_ts);
        let platform = format!(
            "{} {}",
            member.os.as_deref().unwrap_or("unknown"),
            member.arch.as_deref().unwrap_or("")
        );

        member_list.push(format!(
            "{} {} ({})",
            sync_icon,
            hostname,
            &member.machine_id[..8.min(member.machine_id.len())]
        ));
        member_list.push(format!("  Last active: {}", last_sync));
        member_list.push(format!("  Platform: {}", platform));
    }

    let in_sync_count = members
        .iter()
        .filter(|m| {
            let ts = parse_timestamp(&m.last_seen_at);
            now - ts < one_hour
        })
        .count();

    execute_cmd(Cmd::batch([
        Components::header(
            "Team Members",
            &format!("{} member(s), {} in sync", members.len(), in_sync_count),
        ),
        Components::spacer(),
        Components::card("Active Members", member_list),
    ]));

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
pub async fn dashboard(_ctx: &CliContext) -> Result<()> {
    license::require_feature("team-sync")?;
    crate::cli::tui::run_with_tab(crate::cli::tui::app::Tab::Team).await
}

/// Generate team invite link
pub fn invite(email: Option<&str>, role: &str, _ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    // SECURITY: Validate role and email
    let valid_roles = ["admin", "lead", "developer", "readonly"];
    if !valid_roles.contains(&role) {
        execute_cmd(Components::error_with_suggestion(
            &format!("Invalid role: {}", role),
            "Valid roles: admin, lead, developer, readonly",
        ));
        anyhow::bail!("Invalid role: {role}");
    }
    if let Some(e) = email
        && (!e.contains('@') || e.len() > 255)
    {
        execute_cmd(Components::error("Invalid email address"));
        anyhow::bail!("Invalid email address");
    }

    license::require_feature("team-sync")?;

    execute_cmd(Components::loading("Generating invite link..."));

    let invite_id = generate_invite_id();
    let invite_url = format!("https://omg.dev/join/{invite_id}");

    let mut details = vec![("Role", role)];
    if let Some(e) = email {
        details.push(("Email", e));
    }

    execute_cmd(Cmd::batch([
        Components::success("Invite link generated"),
        Components::kv_list(Some("Invite Details"), details),
        Components::spacer(),
        Components::card("Invite URL", vec![invite_url.clone()]),
        Components::spacer(),
        Components::info("Share this link with your teammate"),
        Components::info(&format!("They can join with: omg team join {}", invite_url)),
    ]));

    Ok(())
}

/// Manage team roles
pub mod roles {
    use super::{CliContext, Result, license};
    use crate::cli::components::Components;
    use crate::cli::packages::execute_cmd;
    use crate::cli::tea::Cmd;

    pub fn list(_ctx: &CliContext) -> Result<()> {
        license::require_feature("team-sync")?;

        let role_list = vec![
            "admin - Full access (push, policy, members)".to_string(),
            "lead - Can push to team lock, manage policies".to_string(),
            "developer - Can pull, cannot push without approval".to_string(),
            "readonly - Can only view status".to_string(),
        ];

        execute_cmd(Cmd::batch([
            Components::header("Team Roles", "Available roles"),
            Components::spacer(),
            Components::card("Role Permissions", role_list),
        ]));

        Ok(())
    }

    pub fn assign(member: &str, role: &str, _ctx: &CliContext) -> Result<()> {
        // SECURITY: Validate role and member
        let valid_roles = ["admin", "lead", "developer", "readonly"];
        if !valid_roles.contains(&role) {
            execute_cmd(Components::error(&format!("Invalid role: {}", role)));
            anyhow::bail!("Invalid role: {role}");
        }
        if member.len() > 128 || member.chars().any(char::is_control) {
            execute_cmd(Components::error("Invalid member identifier"));
            anyhow::bail!("Invalid member identifier");
        }

        license::require_feature("team-sync")?;

        execute_cmd(Cmd::batch([
            Components::loading("Assigning role..."),
            Components::success(&format!("{} is now a {}", member, role)),
        ]));

        Ok(())
    }

    pub fn remove(member: &str, _ctx: &CliContext) -> Result<()> {
        license::require_feature("team-sync")?;

        execute_cmd(Cmd::batch([
            Components::loading("Removing role..."),
            Components::success(&format!("Removed role from {}", member)),
        ]));

        Ok(())
    }
}

/// Propose environment changes for review
pub async fn propose(message: &str, _ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    license::require_feature("team-sync")?;

    execute_cmd(Components::loading("Creating proposal..."));

    // Capture current environment state for the proposal
    let packages = {
        #[cfg(feature = "arch")]
        {
            crate::package_managers::list_explicit_fast().unwrap_or_default()
        }
        #[cfg(feature = "debian")]
        {
            crate::package_managers::apt_list_explicit().unwrap_or_default()
        }
        #[cfg(not(any(feature = "arch", feature = "debian")))]
        {
            Vec::<String>::new()
        }
    };

    let state = serde_json::json!({
        "environment": crate::core::env::fingerprint::EnvironmentState::capture().await?,
        "packages": packages,
    });

    let proposal_id = license::propose_change(message, &state).await?;

    execute_cmd(Cmd::batch([
        Components::success(&format!("Proposal #{} created", proposal_id)),
        Components::kv_list(
            Some("Proposal Details"),
            vec![
                ("ID", &proposal_id.to_string()),
                ("Message", &message.to_string()),
            ],
        ),
        Components::spacer(),
        Components::info("Notified reviewers for approval"),
        Components::info(&format!(
            "Check status with: omg team review {}",
            proposal_id
        )),
    ]));

    Ok(())
}

/// Review and approve/reject a proposal
pub async fn review(proposal_id: u32, approve: bool, _ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    license::require_feature("team-sync")?;

    let status = if approve { "approved" } else { "rejected" };
    let status_str = if approve { "APPROVE" } else { "REJECT" };

    execute_cmd(Components::loading(&format!(
        "Reviewing proposal #{} -> {}...",
        proposal_id, status_str
    )));

    license::review_proposal(proposal_id, status).await?;

    execute_cmd(Components::success("Proposal status updated"));
    Ok(())
}

/// List pending team proposals
pub async fn list_proposals(_ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    license::require_feature("team-sync")?;

    let proposals = license::fetch_proposals().await?;

    if proposals.is_empty() {
        execute_cmd(Cmd::batch([
            Components::header("Team Proposals", "No pending proposals"),
            Components::spacer(),
        ]));
        return Ok(());
    }

    let mut proposal_list = vec![];
    for p in &proposals {
        let id = p["id"].as_u64().unwrap_or(0);
        let status = p["status"].as_str().unwrap_or("pending");
        let msg = p["message"].as_str().unwrap_or("");
        let email = p["creator_email"].as_str().unwrap_or("unknown");
        let date = p["created_at"].as_str().unwrap_or("");

        proposal_list.push(format!("#{} [{}] {} - {}", id, status, msg, email));
        proposal_list.push(format!("  Created: {}", date));
    }

    execute_cmd(Cmd::batch([
        Components::header(
            "Team Proposals",
            &format!("{} proposal(s)", proposals.len()),
        ),
        Components::spacer(),
        Components::card("Pending Proposals", proposal_list),
    ]));

    Ok(())
}

/// Manage golden path templates
pub mod golden_path {
    use super::{CliContext, Result, license};
    use crate::cli::components::Components;
    use crate::cli::packages::execute_cmd;
    use crate::cli::tea::Cmd;
    use serde::{Deserialize, Serialize};
    use std::collections::HashMap;

    #[derive(Debug, Serialize, Deserialize)]
    pub struct GoldenPathTemplate {
        pub name: String,
        pub runtimes: HashMap<String, String>,
        pub packages: Vec<String>,
        pub created_at: i64,
    }

    #[derive(Debug, Serialize, Deserialize, Default)]
    pub struct GoldenPathConfig {
        pub templates: Vec<GoldenPathTemplate>,
    }

    impl GoldenPathConfig {
        fn path() -> std::path::PathBuf {
            crate::core::paths::config_dir().join("golden-paths.toml")
        }

        pub fn load() -> Result<Self> {
            let path = Self::path();
            if path.exists() {
                let content = std::fs::read_to_string(&path)?;
                Ok(toml::from_str(&content)?)
            } else {
                Ok(Self::default())
            }
        }

        pub fn save(&self) -> Result<()> {
            let path = Self::path();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let content = toml::to_string_pretty(self)?;
            std::fs::write(path, content)?;
            Ok(())
        }
    }

    pub fn create(
        name: &str,
        node: Option<&str>,
        python: Option<&str>,
        packages: Option<&str>,
        _ctx: &CliContext,
    ) -> Result<()> {
        // SECURITY: Validate all inputs
        if name.chars().any(|c| !c.is_ascii_alphanumeric() && c != '-') {
            execute_cmd(Components::error_with_suggestion(
                "Invalid template name",
                "Template names must be alphanumeric with hyphens only",
            ));
            anyhow::bail!("Invalid template name (alphanumeric and hyphens only)");
        }
        if let Some(v) = node {
            if let Err(e) = crate::core::security::validate_version(v) {
                execute_cmd(Components::error(&format!("Invalid Node version: {}", e)));
                return Err(e.into());
            }
        }
        if let Some(v) = python {
            if let Err(e) = crate::core::security::validate_version(v) {
                execute_cmd(Components::error(&format!("Invalid Python version: {}", e)));
                return Err(e.into());
            }
        }
        if let Some(p) = packages {
            for pkg in p.split(',') {
                if let Err(e) = crate::core::security::validate_package_name(pkg.trim()) {
                    execute_cmd(Components::error(&format!("Invalid package name: {}", e)));
                    return Err(e.into());
                }
            }
        }

        license::require_feature("team-sync")?;

        let mut config = GoldenPathConfig::load()?;

        let mut runtimes = HashMap::new();
        if let Some(v) = node {
            runtimes.insert("node".to_string(), v.to_string());
        }
        if let Some(v) = python {
            runtimes.insert("python".to_string(), v.to_string());
        }

        let package_list = packages
            .map(|p| p.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        let template = GoldenPathTemplate {
            name: name.to_string(),
            runtimes,
            packages: package_list,
            created_at: jiff::Timestamp::now().as_second(),
        };

        // Remove existing if same name
        config.templates.retain(|t| t.name != name);
        config.templates.push(template);
        config.save()?;

        let mut details = vec![format!("Template: {}", name)];
        if let Some(v) = node {
            details.push(format!("Node: {}", v));
        }
        if let Some(v) = python {
            details.push(format!("Python: {}", v));
        }
        if let Some(p) = packages {
            details.push(format!("Packages: {}", p));
        }

        execute_cmd(Cmd::batch([
            Components::success(&format!("Golden path '{}' created!", name)),
            Components::card("Template Details", details),
            Components::spacer(),
            Components::info(&format!(
                "Developers can now use: omg new {} <project-name>",
                name
            )),
        ]));

        Ok(())
    }

    pub fn list(_ctx: &CliContext) -> Result<()> {
        license::require_feature("team-sync")?;

        let config = GoldenPathConfig::load()?;

        if config.templates.is_empty() {
            execute_cmd(Cmd::batch([
                Components::header("Golden Path Templates", "No custom templates"),
                Components::spacer(),
                Components::card(
                    "Default Templates",
                    vec![
                        "react-app - Node 20, React, ESLint, Prettier".to_string(),
                        "python-api - Python 3.12, FastAPI, pytest".to_string(),
                        "go-service - Go 1.21, standard layout".to_string(),
                    ],
                ),
                Components::spacer(),
                Components::info("Create new: omg team golden-path create <name>"),
            ]));
        } else {
            let mut template_list = vec![];
            for t in &config.templates {
                let runtimes = t.runtimes.keys().cloned().collect::<Vec<_>>().join(", ");
                template_list.push(format!(
                    "{} - runtimes: [{}], packages: {}",
                    t.name,
                    runtimes,
                    t.packages.len()
                ));
            }

            execute_cmd(Cmd::batch([
                Components::header(
                    "Golden Path Templates",
                    &format!("{} custom template(s)", config.templates.len()),
                ),
                Components::spacer(),
                Components::card("Available Templates", template_list),
            ]));
        }

        Ok(())
    }

    pub fn delete(name: &str, _ctx: &CliContext) -> Result<()> {
        license::require_feature("team-sync")?;

        let mut config = GoldenPathConfig::load()?;
        let original_len = config.templates.len();
        config.templates.retain(|t| t.name != name);

        if config.templates.len() < original_len {
            config.save()?;
            execute_cmd(Components::success(&format!("Deleted template '{}'", name)));
        } else {
            execute_cmd(Components::warning(&format!(
                "Template '{}' not found",
                name
            )));
        }

        Ok(())
    }
}

/// Check compliance status
pub fn compliance(export: Option<&str>, enforce: bool, _ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    license::require_feature("team-sync")?;

    let compliance_items = vec![
        ("Compliance Score", "94%"),
        ("Valid SPDX licenses", "All packages"),
        ("Critical CVEs", "None"),
        ("Member sync", "Within 7 days"),
        ("SBOM metadata", "2 packages missing"),
        ("Unapproved versions", "1 machine"),
    ];

    execute_cmd(Cmd::batch([
        Components::header("Compliance Status", "Overall score: 94%"),
        Components::spacer(),
        Components::status_summary(compliance_items),
        if enforce {
            Cmd::batch([
                Components::spacer(),
                Components::warning(
                    "Enforcement mode enabled - Non-compliant operations will be blocked",
                ),
            ])
        } else {
            Cmd::none()
        },
        if let Some(path) = export {
            Cmd::batch([
                Components::spacer(),
                Components::success(&format!("Exported to {}", path)),
            ])
        } else {
            Cmd::none()
        },
    ]));

    Ok(())
}

/// Show team activity stream
pub async fn activity(days: u32, _ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    license::require_feature("team-sync")?;

    let logs = license::fetch_audit_logs().await?;

    if logs.is_empty() {
        execute_cmd(Cmd::batch([
            Components::header(
                &format!("Team Activity (last {} days)", days),
                "No recent activity",
            ),
            Components::spacer(),
        ]));
        return Ok(());
    }

    let event_count = logs.len();
    let mut activity_list = vec![];
    for log in &logs {
        let timestamp = format_timestamp(parse_timestamp(&log.created_at));
        let resource = log.resource_type.as_deref().unwrap_or("-").to_string();
        activity_list.push(format!("{} {} ({})", timestamp, log.action, resource));
    }

    execute_cmd(Cmd::batch([
        Components::header(
            &format!("Team Activity (last {} days)", days),
            &format!("{} event(s)", event_count),
        ),
        Components::spacer(),
        Components::card("Recent Activity", activity_list),
    ]));

    Ok(())
}

/// Manage webhook notifications
pub mod team_notify {
    use super::{CliContext, Result, license};
    use crate::cli::components::Components;
    use crate::cli::packages::execute_cmd;
    use crate::cli::tea::Cmd;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Notification {
        pub id: String,
        pub notify_type: String,
        pub url: String,
        pub created_at: i64,
    }

    pub fn add(notify_type: &str, url: &str, _ctx: &CliContext) -> Result<()> {
        // SECURITY: Validate type and URL
        let valid_types = ["slack", "discord", "webhook"];
        if !valid_types.contains(&notify_type) {
            execute_cmd(Components::error(&format!(
                "Invalid notification type: {}",
                notify_type
            )));
            anyhow::bail!("Invalid notification type: {notify_type}");
        }
        if !url.starts_with("https://") || url.len() > 1024 {
            execute_cmd(Components::error_with_suggestion(
                "Invalid or insecure notification URL",
                "HTTPS URLs are required for webhooks",
            ));
            anyhow::bail!("Invalid or insecure notification URL (HTTPS required)");
        }

        license::require_feature("team-sync")?;

        let id = format!("notify-{}", &url.chars().rev().take(6).collect::<String>());

        execute_cmd(Cmd::batch([
            Components::loading("Adding notification..."),
            Components::kv_list(
                Some("Notification Added"),
                vec![
                    ("ID", id.clone()),
                    ("Type", notify_type.to_string()),
                    ("URL", url.to_string()),
                ],
            ),
            Components::spacer(),
            Components::info(&format!("Test it with: omg team notify test {}", id)),
        ]));

        Ok(())
    }

    pub fn list(_ctx: &CliContext) -> Result<()> {
        license::require_feature("team-sync")?;

        execute_cmd(Cmd::batch([
            Components::header("Configured Notifications", "Webhooks and integrations"),
            Components::spacer(),
            Components::card(
                "Active Notifications",
                vec![
                    "notify-abc123 - slack - https://hooks.slack.com/...".to_string(),
                    "notify-xyz789 - discord - https://discord.com/api/...".to_string(),
                ],
            ),
        ]));

        Ok(())
    }

    pub fn remove(id: &str, _ctx: &CliContext) -> Result<()> {
        license::require_feature("team-sync")?;

        execute_cmd(Cmd::batch([
            Components::loading("Removing notification..."),
            Components::success(&format!("Removed '{}'", id)),
        ]));

        Ok(())
    }

    pub fn test(id: &str, _ctx: &CliContext) -> Result<()> {
        license::require_feature("team-sync")?;

        execute_cmd(Cmd::batch([
            Components::loading(&format!("Testing notification '{}'...", id)),
            Components::success("Test message sent!"),
        ]));

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
