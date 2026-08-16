//! `omg self-update` - Update OMG to the latest version

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::env;
use std::fs;

use crate::cli::style;

const UPDATE_URL: &str = "https://releases.pyro1121.com";

/// Update OMG to the latest version
pub async fn run(force: bool, version: Option<String>) -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!(
        "{} Checking for updates... (current: v{})",
        style::runtime("OMG"),
        current_version
    );

    let target_version = if let Some(v) = version {
        v
    } else {
        // Fetch latest version
        let resp = crate::core::http::shared_client()
            .get(format!("{UPDATE_URL}/latest-version"))
            .send()
            .await
            .context("Failed to check for updates")?;

        if !resp.status().is_success() {
            anyhow::bail!("Failed to fetch version info: {}", resp.status());
        }

        resp.text().await?.trim().to_string()
    };

    if !force && target_version == current_version {
        println!(
            "  {} You are already on the latest version.",
            style::maybe_color("✓", |t| t.green().to_string())
        );
        return Ok(());
    }

    println!(
        "  {} Downloading version v{}...",
        style::maybe_color("⬇", |t| t.blue().to_string()),
        target_version
    );

    // Download binary
    let platform = "x86_64-unknown-linux-gnu"; // Auto-detect in real impl
    let download_url = format!("{UPDATE_URL}/download/omg-{target_version}-{platform}.tar.gz");

    let response = crate::core::http::download_client()
        .get(&download_url)
        .send()
        .await?;
    if !response.status().is_success() {
        anyhow::bail!("Failed to download update: {}", response.status());
    }

    let total_size = response
        .content_length()
        .ok_or_else(|| anyhow::anyhow!("Failed to get content length"))?;

    use indicatif::{ProgressBar, ProgressStyle};
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")?
        .progress_chars("#>-"));

    let capacity =
        usize::try_from(total_size).context("Update file size exceeds platform address space")?;
    let mut bytes = Vec::with_capacity(capacity);
    let mut stream = response.bytes_stream();

    use futures::StreamExt;
    while let Some(item) = stream.next().await {
        let chunk = item.context("Failed to read update download chunk")?;
        bytes.extend_from_slice(&chunk);
        pb.inc(chunk.len() as u64);
    }
    pb.finish_with_message("Download complete");

    // Perform blocking extraction and binary replacement in a separate thread
    // to avoid blocking the tokio async runtime
    tokio::task::spawn_blocking(move || -> Result<()> {
        let cursor = std::io::Cursor::new(bytes);
        let decoder = flate2::read::GzDecoder::new(cursor);
        let mut archive = tar::Archive::new(decoder);

        let temp_dir = tempfile::tempdir().context("Failed to create temp directory for update")?;
        archive
            .unpack(temp_dir.path())
            .context("Failed to extract update archive")?;

        let new_binary = temp_dir.path().join("omg");
        if !new_binary.exists() {
            anyhow::bail!("Update archive did not contain 'omg' binary");
        }

        // Replace current binary
        let current_exe = env::current_exe().context("Failed to find current executable path")?;

        // On Linux we can rename over the running executable
        // We rename the *current* exe to .old first to be safe, then move new one in
        let backup_path = current_exe.with_extension("old");
        fs::rename(&current_exe, &backup_path).context("Failed to backup current binary")?;

        match fs::rename(&new_binary, &current_exe) {
            Ok(()) => {
                // Fix permissions on Unix
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mut perms = fs::metadata(&current_exe)
                        .context("Failed to read updated binary metadata")?
                        .permissions();
                    perms.set_mode(0o755);
                    fs::set_permissions(&current_exe, perms)
                        .context("Failed to set updated binary permissions")?;
                }

                // Cleanup backup; a leftover .old file is harmless.
                if let Err(error) = fs::remove_file(&backup_path) {
                    tracing::debug!(
                        "Failed to remove self-update backup {}: {error}",
                        backup_path.display()
                    );
                }
            }
            Err(e) => {
                // Restore backup, and surface a restore failure: otherwise the
                // binary is left missing with only the install error reported.
                if let Err(restore_error) = fs::rename(&backup_path, &current_exe) {
                    return Err(anyhow::anyhow!(
                        "Failed to install update: {e}; additionally failed to restore the previous binary: {restore_error}"
                    ));
                }
                return Err(anyhow::anyhow!("Failed to install update: {e}"));
            }
        }
        Ok(())
    })
    .await??;

    println!(
        "  {} Update successful!",
        style::maybe_color("✓", |t| t.green().to_string())
    );
    println!(
        "  {} is now installed.",
        style::maybe_color(&format!("v{target_version}"), |t| t.cyan().to_string())
    );

    Ok(())
}
