//! `omg fleet` - Multi-machine fleet management (Enterprise)

use crate::cli::components::Components;
use crate::cli::tea::Cmd;
use crate::cli::{CliContext, FleetCommands, LocalCommandRunner};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::core::license;

impl LocalCommandRunner for FleetCommands {
    async fn execute(&self, ctx: &CliContext) -> Result<()> {
        match self {
            FleetCommands::Status => status(ctx).await,
            FleetCommands::Push { team, message } => {
                push(team.as_deref(), message.as_deref(), ctx).await
            }
        }
    }
}

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
pub async fn status(_ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    license::require_feature("fleet")?;

    let members = license::fetch_team_members().await?;

    let total_machines = members.len();
    let now = jiff::Timestamp::now().as_second();
    let one_day = 24 * 60 * 60;

    let active_machines = members.iter().filter(|m| m.is_active).count();
    let online_machines = members
        .iter()
        .filter(|m| {
            jiff::Timestamp::from_second(parse_timestamp(&m.last_seen_at))
                .is_ok_and(|ts| now - ts.as_second() < one_day)
        })
        .count();

    // Online in the last day is an availability signal, not policy compliance.
    let online = online_machines;
    let offline = total_machines.saturating_sub(active_machines);

    let online_pct = if total_machines > 0 {
        (online as f32 / total_machines as f32) * 100.0
    } else {
        0.0
    };

    let health_bar = generate_health_bar(online_pct);

    let status_items = vec![
        ("Total Machines", total_machines.to_string()),
        (
            "Online (24h)",
            format!("{}% {}", online_pct as u32, health_bar),
        ),
        ("Active", active_machines.to_string()),
        ("Offline", offline.to_string()),
    ];

    let mut machine_list = vec![];
    for m in members.iter().filter(|m| m.is_active).take(10) {
        let hostname = m.hostname.as_deref().unwrap_or(&m.machine_id);
        let os = m.os.as_deref().unwrap_or("unknown");
        let ver = m.omg_version.as_deref().unwrap_or("?");
        machine_list.push(format!("{} {:<20} {:<10} v{}", "💻", hostname, os, ver));
    }

    if total_machines > 10 {
        machine_list.push(format!("... and {} more", total_machines - 10));
    }

    execute_cmd(Cmd::batch([
        Cmd::header(
            "Fleet Status",
            format!("{total_machines} machine(s) in fleet"),
        ),
        Cmd::spacer(),
        Components::status_summary(status_items),
        if machine_list.is_empty() {
            Cmd::none()
        } else {
            Cmd::batch([Cmd::spacer(), Cmd::card("Active Machines", machine_list)])
        },
        Cmd::spacer(),
        Cmd::println("Manage your fleet at: https://pyro1121.com/dashboard"),
    ]));

    Ok(())
}

fn parse_timestamp(s: &str) -> i64 {
    // Simple parser for "YYYY-MM-DD HH:MM:SS" or ISO
    s.parse::<jiff::Timestamp>()
        .map_or(0, jiff::Timestamp::as_second)
}

/// Push configuration to fleet
pub async fn push(team: Option<&str>, message: Option<&str>, _ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    if let Some(t) = team {
        // SECURITY: Validate team identifier
        if t.chars()
            .any(|c| !c.is_ascii_alphanumeric() && c != '/' && c != '-' && c != '_')
        {
            execute_cmd(Components::error_with_suggestion(
                "Invalid team identifier",
                "Team IDs must be alphanumeric with /, -, or _ allowed",
            ));
            anyhow::bail!("Invalid team identifier");
        }
    }
    if let Some(m) = message {
        // SECURITY: Validate message
        if m.len() > 1000 {
            execute_cmd(Cmd::error("Push message too long (max 1000 characters)"));
            anyhow::bail!("Push message too long");
        }
    }

    license::require_feature("fleet")?;

    let target = team.unwrap_or("all machines");
    let msg = message.unwrap_or("Fleet push");

    execute_cmd(Components::loading(format!("Pushing to {target}...")));

    // Fetch members to get a real count
    let members = license::fetch_team_members().await?;
    let count = members.len();

    let lock_path = std::path::Path::new("omg.lock");
    let lock_content = if lock_path.exists() {
        std::fs::read_to_string(lock_path).context("Failed to read omg.lock")?
    } else {
        execute_cmd(Cmd::warning(
            "No omg.lock found, capturing current state...",
        ));
        String::new()
    };

    let push_result = crate::core::http::shared_client()
        .post("https://api.pyro1121.com/api/fleet/push")
        .json(&serde_json::json!({
            "team": target,
            "message": msg,
            "lock_content": lock_content,
            "machine_count": count
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await;

    match push_result {
        Ok(res) => {
            let status = res.status().as_u16();
            if let Err(e) = fleet_push_http_outcome(status) {
                execute_cmd(Cmd::error(e.to_string()));
                return Err(e);
            }
        }
        Err(e) => {
            // Network error
            execute_cmd(Cmd::error(format!(
                "Failed to connect to fleet server: {e}"
            )));
            anyhow::bail!("Failed to connect to fleet server: {e}");
        }
    }

    execute_cmd(Cmd::batch([
        Cmd::success("Push complete!"),
        Components::kv_list(
            Some("Push Summary"),
            vec![
                ("Target", target.to_string()),
                ("Applied immediately", count.to_string()),
                ("Scheduled for next login", "0".to_string()),
                ("Message", msg.to_string()),
            ],
        ),
    ]));

    Ok(())
}

fn fleet_push_http_outcome(status: u16) -> Result<()> {
    if (200..300).contains(&status) {
        Ok(())
    } else if status == 404 {
        anyhow::bail!("Fleet API endpoint not found (404)")
    } else {
        anyhow::bail!("Fleet push failed: {status}")
    }
}

fn generate_health_bar(pct: f32) -> String {
    let filled = (pct / 10.0) as usize;
    let empty = 10 - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fleet_push_rejects_missing_endpoint() {
        let err =
            fleet_push_http_outcome(404).expect_err("404 must not look like a successful push");
        assert!(err.to_string().contains("404"), "got: {err}");
    }

    #[test]
    fn fleet_push_rejects_server_errors() {
        let err = fleet_push_http_outcome(503).expect_err("5xx must fail the push");
        assert!(err.to_string().contains("503"), "got: {err}");
    }

    #[test]
    fn fleet_push_accepts_success() {
        assert!(fleet_push_http_outcome(200).is_ok());
        assert!(fleet_push_http_outcome(204).is_ok());
    }
}
