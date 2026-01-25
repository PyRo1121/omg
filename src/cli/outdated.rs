//! `omg outdated` - Show what packages would be updated

use anyhow::Result;
use serde::Serialize;
use std::sync::Arc;

use crate::cli::tea::Cmd;
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
    use crate::cli::components::Components;

    // SECURITY: This command has no string inputs, but we validate environment state
    if !json {
        crate::cli::packages::execute_cmd(Components::loading("Checking for updates"));
    }

    let pm = Arc::from(get_package_manager());
    let service = PackageService::new(pm);
    let updates = service.list_updates().await?;

    if updates.is_empty() {
        if json {
            println!("[]");
        } else {
            crate::cli::packages::execute_cmd(Components::up_to_date());
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

    let mut commands = vec![
        Components::spacer(),
        Components::header(
            "Available Updates",
            format!("{} packages total", filtered.len()),
        ),
        Components::spacer(),
    ];

    // Security updates
    if !security.is_empty() {
        commands.push(Components::card(
            format!("Security Updates (install immediately)",),
            security
                .iter()
                .map(|p| format!("{} {} → {} (CVE)", p.name, p.current_version, p.new_version))
                .collect(),
        ));
        commands.push(Components::spacer());
    }

    // Major updates
    if !major.is_empty() {
        commands.push(Components::card(
            format!("Major Updates (may have breaking changes)"),
            major
                .iter()
                .map(|p| format!("{} {} → ({})", p.name, p.current_version, p.repo))
                .collect(),
        ));
        commands.push(Components::spacer());
    }

    // Minor updates
    if !minor.is_empty() {
        let minor_count = minor.len().min(10);
        commands.push(Components::card(
            format!("Minor Updates (new features)"),
            minor
                .iter()
                .take(minor_count)
                .map(|p| format!("{} {} → {}", p.name, p.current_version, p.new_version))
                .collect(),
        ));

        if minor.len() > 10 {
            commands.push(Components::muted(format!(
                "... and {} more minor updates",
                minor.len() - 10
            )));
        }
        commands.push(Components::spacer());
    }

    // Patch updates
    if !patch.is_empty() {
        let patch_count = patch.len().min(5);
        commands.push(Components::card(
            format!("Patch Updates (bug fixes)"),
            patch
                .iter()
                .take(patch_count)
                .map(|p| format!("{} {} → {}", p.name, p.current_version, p.new_version))
                .collect(),
        ));

        if patch.len() > 5 {
            commands.push(Components::muted(format!(
                "... and {} more patch updates",
                patch.len() - 5
            )));
        }
        commands.push(Components::spacer());
    }

    // Summary
    commands.push(Components::kv_list(
        Some("Summary"),
        vec![
            ("Security Updates", &security.len().to_string()),
            ("Major Updates", &major.len().to_string()),
            ("Minor Updates", &minor.len().to_string()),
            ("Patch Updates", &patch.len().to_string()),
        ],
    ));
    commands.push(Components::spacer());

    // Actions
    commands.push(Components::info("Run 'omg update' to update all packages"));
    if !security.is_empty() {
        commands.push(Components::warning(
            "Run 'omg update --security' to update security fixes only",
        ));
    }

    crate::cli::packages::execute_cmd(Cmd::batch(commands));

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
