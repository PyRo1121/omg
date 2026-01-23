//! `omg self-update` - Update OMG to the latest version

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::env;
use std::fs;
use std::path::PathBuf;

const UPDATE_URL: &str = "https://releases.pyro1121.com";

/// Update OMG to the latest version
pub async fn run(force: bool, version: Option<String>) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!(
        "{} Checking for updates... (current: v{})",
        "OMG".cyan().bold(),
        current_version
    );

    let target_version = if let Some(v) = version {
        v
    } else {
        // Fetch latest version
        let client = reqwest::Client::new();
        let resp = client
            .get(format!("{}/latest-version", UPDATE_URL))
            .send()
            .await
            .context("Failed to check for updates")?;
        
        if !resp.status().is_success() {
            anyhow::bail!("Failed to fetch version info: {}", resp.status());
        }

        resp.text().await?.trim().to_string()
    };

    if !force && target_version == current_version {
        println!("  {} You are already on the latest version.", "✓".green());
        return Ok(());
    }

    println!(
        "  {} Downloading version v{}...",
        "⬇".blue(),
        target_version
    );

    // Download binary
    let platform = "x86_64-unknown-linux-gnu"; // Auto-detect in real impl
    let download_url = format!("{}/download/omg-{}-{}.tar.gz", UPDATE_URL, target_version, platform);
    
    let response = reqwest::get(&download_url).await?;
    if !response.status().is_success() {
        anyhow::bail!("Failed to download update: {}", response.status());
    }

    let bytes = response.bytes().await?;
    let cursor = std::io::Cursor::new(bytes);
    let decoder = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(decoder);

    // Create temp directory for extraction
    let temp_dir = tempfile::tempdir()?;
    archive.unpack(temp_dir.path())?;

    // Find the new binary
    let new_binary = temp_dir.path().join("omg");
    
    // Perform blocking I/O operations in a separate thread
    tokio::task::spawn_blocking(move || -> Result<()> {
        archive.unpack(temp_dir.path())?;

        if !new_binary.exists() {
            anyhow::bail!("Update archive did not contain 'omg' binary");
        }

        // Replace current binary
        let current_exe = env::current_exe()?;
        
        // On Linux we can rename over the running executable
        // We rename the *current* exe to .old first to be safe, then move new one in
        let backup_path = current_exe.with_extension("old");
        fs::rename(&current_exe, &backup_path).context("Failed to backup current binary")?;
        
        match fs::rename(&new_binary, &current_exe) {
            Ok(_) => {
                // Fix permissions
                use std::os::unix::fs::PermissionsExt;
                let mut perms = fs::metadata(&current_exe)?.permissions();
                perms.set_mode(0o755);
                fs::set_permissions(&current_exe, perms)?;
                
                // Cleanup backup
                let _ = fs::remove_file(backup_path);
            }
            Err(e) => {
                // Restore backup
                let _ = fs::rename(&backup_path, &current_exe);
                return Err(anyhow::anyhow!("Failed to install update: {}", e));
            }
        }
        Ok(())
    }).await??;

    println!("  {} Update successful!", "✓".green());
    println!("  {} is now installed.", format!("v{}", target_version).cyan());

    Ok(())
}
