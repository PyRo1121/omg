//! `omg fleet` - Multi-machine fleet management (Enterprise)

use crate::cli::components::Components;
use crate::cli::tea::Cmd;
use crate::cli::{CliContext, FleetCommands, LocalCommandRunner};
use crate::core::license;
use anyhow::Result;

impl LocalCommandRunner for FleetCommands {
    async fn execute(&self, ctx: &CliContext) -> Result<()> {
        match self {
            FleetCommands::Status => status(ctx).await,
        }
    }
}

/// Show fleet status
pub async fn status(_ctx: &CliContext) -> Result<()> {
    use crate::cli::packages::execute_cmd;

    let members = license::fetch_team_members().await?;

    let total_machines = members.len();
    let now = jiff::Timestamp::now().as_second();
    let one_day: i64 = 24 * 60 * 60;

    let active_machines = members.iter().filter(|m| m.is_active).count();
    let online_machines = members
        .iter()
        .filter(|m| {
            crate::cli::parse_timestamp_opt(&m.last_seen_at)
                .is_some_and(|ts| now.saturating_sub(ts) < one_day)
        })
        .count();

    let online_pct = if total_machines > 0 {
        (online_machines as f32 / total_machines as f32) * 100.0
    } else {
        0.0
    };

    let health_bar = generate_health_bar(online_pct);

    // "inactive" is a roster flag, not an offline state.
    let status_items = vec![
        ("Total Machines", total_machines.to_string()),
        (
            "Online (24h)",
            format!("{}% {}", online_pct as u32, health_bar),
        ),
        ("Active", active_machines.to_string()),
        (
            "Inactive",
            total_machines.saturating_sub(active_machines).to_string(),
        ),
    ];

    let mut machine_list = vec![];
    for m in members.iter().filter(|m| m.is_active).take(10) {
        let hostname = m.hostname.as_deref().unwrap_or(&m.machine_id);
        let os = m.os.as_deref().unwrap_or("unknown");
        let ver = m.omg_version.as_deref().unwrap_or("?");
        machine_list.push(format!("{} {:<20} {:<10} v{}", "💻", hostname, os, ver));
    }

    let remaining_active = remaining_active_machine_count(active_machines, machine_list.len());
    if remaining_active > 0 {
        machine_list.push(format!("... and {remaining_active} more"));
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
        Cmd::println("Manage your fleet at: https://omg.latham.cloud"),
    ]))?;

    Ok(())
}

fn remaining_active_machine_count(active: usize, shown: usize) -> usize {
    active.saturating_sub(shown)
}

fn generate_health_bar(pct: f32) -> String {
    let filled = ((pct / 10.0).round() as usize).min(10);
    let empty = 10 - filled;
    format!("{}{}", "█".repeat(filled), "░".repeat(empty))
}

#[cfg(test)]
mod tests {
    #[test]
    fn active_machine_overflow_excludes_inactive_members() {
        let active_machines = 12usize;
        let shown_active_machines = 10usize;
        assert_eq!(
            super::remaining_active_machine_count(active_machines, shown_active_machines),
            2
        );
    }
}
