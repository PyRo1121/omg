//! `omg snapshot` - Create and restore environment snapshots

use anyhow::Result;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::core::env::fingerprint::EnvironmentState;
use crate::core::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub message: Option<String>,
    pub created_at: i64,
    pub state: EnvironmentState,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SnapshotIndex {
    snapshots: Vec<SnapshotMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotMeta {
    id: String,
    message: Option<String>,
    created_at: i64,
    hash: String,
}

fn snapshots_dir() -> PathBuf {
    paths::data_dir().join("snapshots")
}

fn index_path() -> PathBuf {
    snapshots_dir().join("index.json")
}

fn load_index() -> Result<SnapshotIndex> {
    let path = index_path();
    if path.exists() {
        let content = fs::read_to_string(&path)?;
        Ok(serde_json::from_str(&content)?)
    } else {
        Ok(SnapshotIndex::default())
    }
}

fn save_index(index: &SnapshotIndex) -> Result<()> {
    let path = index_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let content = serde_json::to_string_pretty(index)?;
    fs::write(path, content)?;
    Ok(())
}

/// Create a new snapshot
pub async fn create(message: Option<String>) -> Result<()> {
    if let Some(ref msg) = message {
        // SECURITY: Validate message length
        if msg.len() > 1000 {
            anyhow::bail!("Snapshot message too long");
        }
    }

    println!("{} Creating snapshot...\n", "OMG".cyan().bold());

    let state = EnvironmentState::capture().await?;
    let id = generate_snapshot_id();

    let snapshot = Snapshot {
        id: id.clone(),
        message: message.clone(),
        created_at: jiff::Timestamp::now().as_second(),
        state: state.clone(),
    };

    // Save snapshot file
    let snapshot_path = snapshots_dir().join(format!("{id}.json"));
    fs::create_dir_all(snapshots_dir())?;
    let content = serde_json::to_string_pretty(&snapshot)?;
    fs::write(&snapshot_path, content)?;

    // Update index
    let mut index = load_index()?;
    index.snapshots.push(SnapshotMeta {
        id: id.clone(),
        message: message.clone(),
        created_at: snapshot.created_at,
        hash: state.hash.clone(),
    });
    save_index(&index)?;

    println!("  {} Snapshot created!", "✓".green());
    println!("  ID: {}", id.cyan());
    if let Some(msg) = &message {
        println!("  Message: {msg}");
    }
    println!("  Runtimes: {}", state.runtimes.len());
    println!("  Packages: {}", state.packages.len());
    println!();
    println!(
        "  Restore with: {}",
        format!("omg snapshot restore {id}").cyan()
    );

    Ok(())
}

/// List all snapshots
pub fn list() -> Result<()> {
    println!("{} Snapshots\n", "OMG".cyan().bold());

    let index = load_index()?;

    if index.snapshots.is_empty() {
        println!("  {} No snapshots found", "○".dimmed());
        println!();
        println!("  Create one with: {}", "omg snapshot create".cyan());
        return Ok(());
    }

    println!(
        "  {:12} {:20} {:12} {}",
        "ID".bold(),
        "Date".bold(),
        "Hash".bold(),
        "Message".bold()
    );
    println!("  {}", "─".repeat(60));

    for snap in index.snapshots.iter().rev() {
        let date = format_timestamp(snap.created_at);
        let msg = snap.message.as_deref().unwrap_or("-");
        let short_hash = &snap.hash[..8.min(snap.hash.len())];

        println!(
            "  {} {} {} {}",
            snap.id.cyan(),
            date.dimmed(),
            short_hash.dimmed(),
            msg
        );
    }

    println!();
    println!("  {} snapshots total", index.snapshots.len());

    Ok(())
}

/// Restore a snapshot
pub async fn restore(id: &str, dry_run: bool, yes: bool) -> Result<()> {
    // SECURITY: Validate snapshot ID
    if id.chars().any(|c| !c.is_ascii_alphanumeric() && c != '-') {
        anyhow::bail!("Invalid snapshot ID: {id}");
    }

    println!(
        "{} {} snapshot {}\n",
        "OMG".cyan().bold(),
        if dry_run {
            "Preview restore of"
        } else {
            "Restoring"
        },
        id.yellow()
    );

    // Load snapshot
    let snapshot_path = snapshots_dir().join(format!("{id}.json"));
    if !snapshot_path.exists() {
        anyhow::bail!("Snapshot '{id}' not found");
    }

    let content = fs::read_to_string(&snapshot_path)?;
    let snapshot: Snapshot = serde_json::from_str(&content)?;

    // Capture current state
    let current = EnvironmentState::capture().await?;

    // Calculate diff
    println!("  {}", "Changes to apply:".bold());
    println!();

    // Runtime changes
    let mut runtime_changes = Vec::new();
    for (runtime, target_ver) in &snapshot.state.runtimes {
        let current_ver = current.runtimes.get(runtime);
        if current_ver != Some(target_ver) {
            runtime_changes.push((runtime.clone(), current_ver.cloned(), target_ver.clone()));
        }
    }

    if !runtime_changes.is_empty() {
        println!("  Runtimes:");
        for (runtime, from, to) in &runtime_changes {
            let from_str = from.as_deref().unwrap_or("(none)");
            println!(
                "    {} {} → {}",
                runtime.yellow(),
                from_str.dimmed(),
                to.green()
            );
        }
        println!();
    }

    // Package changes
    let current_pkgs: std::collections::HashSet<_> = current.packages.iter().collect();
    let target_pkgs: std::collections::HashSet<_> = snapshot.state.packages.iter().collect();

    let to_install: Vec<_> = target_pkgs.difference(&current_pkgs).collect();
    let to_remove: Vec<_> = current_pkgs.difference(&target_pkgs).collect();

    if !to_install.is_empty() {
        println!("  Packages to install ({}):", to_install.len());
        for pkg in to_install.iter().take(10) {
            println!("    {} {}", "+".green(), pkg);
        }
        if to_install.len() > 10 {
            println!("    ... and {} more", to_install.len() - 10);
        }
        println!();
    }

    if !to_remove.is_empty() {
        println!("  Packages to remove ({}):", to_remove.len());
        for pkg in to_remove.iter().take(10) {
            println!("    {} {}", "-".red(), pkg);
        }
        if to_remove.len() > 10 {
            println!("    ... and {} more", to_remove.len() - 10);
        }
        println!();
    }

    if runtime_changes.is_empty() && to_install.is_empty() && to_remove.is_empty() {
        println!("  {} Environment already matches snapshot!", "✓".green());
        return Ok(());
    }

    if dry_run {
        println!("  {} Dry run - no changes made", "ℹ".blue());
        println!(
            "  Run without --dry-run to apply: {}",
            format!("omg snapshot restore {id}").cyan()
        );
        return Ok(());
    }

    // Apply changes
    println!("  {}", "Applying changes...".bold());

    // Switch runtimes
    for (runtime, _, target_ver) in &runtime_changes {
        println!("    Switching {runtime} to {target_ver}...");
        crate::cli::runtimes::use_version(runtime, Some(target_ver)).await?;
    }

    // Install/remove packages
    if !to_install.is_empty() || !to_remove.is_empty() {
        println!();
        if !yes {
            println!(
                "  {} Package changes found ({} to install, {} to remove):",
                "⚠".yellow(),
                to_install.len(),
                to_remove.len()
            );
            
            let confirm = dialoguer::Confirm::new()
                .with_prompt("Do you want to apply these package changes?")
                .default(false)
                .interact()?;
                
            if !confirm {
                println!("  {} Package changes skipped", "ℹ".blue());
                return Ok(());
            }
        }

        if !to_install.is_empty() {
            let pkgs: Vec<String> = to_install.iter().map(|s| (**s).clone()).collect();
            println!("    Installing {} packages...", pkgs.len());
            crate::cli::packages::install(&pkgs, true).await?;
        }

        if !to_remove.is_empty() {
            let pkgs: Vec<String> = to_remove.iter().map(|s| (**s).clone()).collect();
            println!("    Removing {} packages...", pkgs.len());
            crate::cli::packages::remove(&pkgs, false, true).await?;
        }
    }

    println!();
    println!("  {} Snapshot restore complete!", "✓".green());

    Ok(())
}

/// Delete a snapshot
pub fn delete(id: &str) -> Result<()> {
    // SECURITY: Validate snapshot ID
    if id.chars().any(|c| !c.is_ascii_alphanumeric() && c != '-') {
        anyhow::bail!("Invalid snapshot ID: {id}");
    }

    let snapshot_path = snapshots_dir().join(format!("{id}.json"));

    if !snapshot_path.exists() {
        anyhow::bail!("Snapshot '{id}' not found");
    }

    // Remove file
    fs::remove_file(&snapshot_path)?;

    // Update index
    let mut index = load_index()?;
    index.snapshots.retain(|s| s.id != id);
    save_index(&index)?;

    println!("{} Deleted snapshot {}", "✓".green(), id.yellow());

    Ok(())
}

fn generate_snapshot_id() -> String {
    let now = jiff::Timestamp::now();
    let date = format!("{now}").chars().take(10).collect::<String>();
    let random: String = (0..6)
        .map(|_| {
            let idx = rand_byte() % 36;
            if idx < 10 {
                (b'0' + idx) as char
            } else {
                (b'a' + idx - 10) as char
            }
        })
        .collect();
    format!("snap-{date}-{random}")
}

fn rand_byte() -> u8 {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .subsec_nanos();
    (nanos % 256) as u8
}

fn format_timestamp(ts: i64) -> String {
    use jiff::Timestamp;
    if let Ok(dt) = Timestamp::from_second(ts) {
        format!("{dt}")
            .chars()
            .take(16)
            .collect::<String>()
            .replace('T', " ")
    } else {
        "unknown".to_string()
    }
}
