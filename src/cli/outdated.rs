//! `omg outdated` - Show what packages would be updated

use anyhow::Result;
use serde::Serialize;

use crate::cli::tea::{Cmd, UpdateType};
use crate::core::packages::PackageService;
use crate::package_managers::get_package_manager;

#[derive(Debug, Serialize)]
pub struct OutdatedPackage {
    pub name: String,
    pub current_version: String,
    pub new_version: String,
    pub update_type: UpdateType,
    pub repo: String,
}

/// Show outdated packages
pub async fn run(json: bool) -> Result<()> {
    use crate::cli::components::Components;

    // SECURITY: This command has no string inputs, but we validate environment state
    if !json {
        crate::cli::packages::execute_cmd(Components::loading("Checking for updates"))?;
    }

    let pm = get_package_manager()?;
    let service = PackageService::new(pm)?;
    let updates = service.list_updates().await?;

    if updates.is_empty() {
        if json {
            println!("[]");
        } else {
            crate::cli::packages::execute_cmd(Components::up_to_date())?;
        }
        return Ok(());
    }

    let mut outdated: Vec<OutdatedPackage> = updates
        .into_iter()
        .map(|u| OutdatedPackage {
            update_type: classify_update(&u.old_version, &u.new_version),
            name: u.name,
            current_version: u.old_version,
            new_version: u.new_version,
            repo: u.repo,
        })
        .collect();

    outdated.sort_by(|a, b| a.name.cmp(&b.name));

    if json {
        println!("{}", serde_json::to_string_pretty(&outdated)?);
        return Ok(());
    }

    let major: Vec<_> = outdated
        .iter()
        .filter(|p| matches!(p.update_type, UpdateType::Major))
        .collect();
    let minor: Vec<_> = outdated
        .iter()
        .filter(|p| matches!(p.update_type, UpdateType::Minor))
        .collect();
    let patch: Vec<_> = outdated
        .iter()
        .filter(|p| matches!(p.update_type, UpdateType::Patch))
        .collect();

    let mut commands = vec![
        Cmd::spacer(),
        Cmd::header(
            "Available Updates",
            format!("{} packages total", outdated.len()),
        ),
        Cmd::spacer(),
    ];

    if !major.is_empty() {
        commands.push(Cmd::card(
            "Major Updates (may have breaking changes)".to_string(),
            major
                .iter()
                .map(|p| {
                    format!(
                        "{} {} → {} ({})",
                        p.name, p.current_version, p.new_version, p.repo
                    )
                })
                .collect(),
        ));
        commands.push(Cmd::spacer());
    }

    // Minor updates
    if !minor.is_empty() {
        let minor_count = minor.len().min(10);
        commands.push(Cmd::card(
            "Minor Updates (new features)".to_string(),
            minor
                .iter()
                .take(minor_count)
                .map(|p| format!("{} {} → {}", p.name, p.current_version, p.new_version))
                .collect(),
        ));

        if minor.len() > 10 {
            use crate::cli::tea::{StyledTextConfig, TextStyle};
            commands.push(Cmd::styled_text(StyledTextConfig {
                text: format!("... and {} more minor updates", minor.len() - 10),
                style: TextStyle::Muted,
            }));
        }
        commands.push(Cmd::spacer());
    }

    // Patch updates
    if !patch.is_empty() {
        let patch_count = patch.len().min(5);
        commands.push(Cmd::card(
            "Patch Updates (bug fixes)".to_string(),
            patch
                .iter()
                .take(patch_count)
                .map(|p| format!("{} {} → {}", p.name, p.current_version, p.new_version))
                .collect(),
        ));

        if patch.len() > 5 {
            use crate::cli::tea::{StyledTextConfig, TextStyle};
            commands.push(Cmd::styled_text(StyledTextConfig {
                text: format!("... and {} more patch updates", patch.len() - 5),
                style: TextStyle::Muted,
            }));
        }
        commands.push(Cmd::spacer());
    }

    // Summary
    commands.push(Components::kv_list(
        Some("Summary"),
        vec![
            ("Major Updates", &major.len().to_string()),
            ("Minor Updates", &minor.len().to_string()),
            ("Patch Updates", &patch.len().to_string()),
        ],
    ));
    commands.push(Cmd::spacer());

    // Actions
    commands.push(Cmd::info("Run 'omg update' to update all packages"));

    crate::cli::packages::execute_cmd(Cmd::batch(commands))?;

    Ok(())
}

/// Classify an update by comparing the parsed versions; falls back to
/// [`UpdateType::Unknown`] when either side is not valid semver.
fn classify_update(old: &str, new: &str) -> UpdateType {
    UpdateType::from_versions(old, new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_update_uses_version_change_not_cve_status() {
        assert!(matches!(
            classify_update("1.0.0", "2.0.0"),
            UpdateType::Major
        ));
        assert!(matches!(
            classify_update("1.0.0", "1.1.0"),
            UpdateType::Minor
        ));
        assert!(matches!(
            classify_update("1.0.0", "1.0.1"),
            UpdateType::Patch
        ));
    }
}
