//! Native Go runtime manager - PURE RUST
//!
//! Downloads and manages Go versions from go.dev.
//!
//! Features:
//! - Official binaries from go.dev
//! - Checksum verification (SHA256)
//! - GOROOT auto-configuration

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use super::common::{
    download_with_progress, extract_tar_gz, get_current_version, list_installed_versions,
    normalize_version, print_already_installed, print_installed, set_current_version,
};
use crate::core::http::download_client;

const GO_DOWNLOAD_URL: &str = "https://go.dev/dl";
const GO_VERSIONS_URL: &str = "https://go.dev/dl/?mode=json";

/// Go version info from go.dev
#[derive(Debug, Clone, Deserialize)]
pub struct GoVersion {
    pub version: String,
    pub stable: bool,
}

pub struct GoManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}

impl GoManager {
    pub fn new() -> Self {
        let data_dir = super::DATA_DIR.clone();

        Self {
            versions_dir: data_dir.join("versions").join("go"),
            current_link: data_dir.join("versions").join("go").join("current"),
            client: download_client().clone(),
        }
    }

    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.current_link.join("bin")
    }

    /// List available Go versions from go.dev
    pub async fn list_available(&self) -> Result<Vec<GoVersion>> {
        let versions: Vec<GoVersion> = self
            .client
            .get(GO_VERSIONS_URL)
            .send()
            .await
            .context("Failed to fetch Go version list. Check your internet connection.")?
            .json()
            .await
            .context("Failed to parse Go version list from go.dev")?;

        // Clean up version strings (remove "go" prefix)
        let versions: Vec<GoVersion> = versions
            .into_iter()
            .map(|mut v| {
                v.version = v.version.trim_start_matches("go").to_string();
                v
            })
            .collect();

        Ok(versions)
    }

    pub fn list_installed(&self) -> Result<Vec<String>> {
        list_installed_versions(&self.versions_dir)
    }

    #[must_use]
    pub fn current_version(&self) -> Option<String> {
        get_current_version(&self.versions_dir)
    }

    /// Install Go - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        let version_dir = self.versions_dir.join(&version);

        if version_dir.exists() {
            print_already_installed("Go", &version);
            return self.use_version(&version);
        }

        tracing::info!(
            "{} Installing Go {}...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        let arch = match std::env::consts::ARCH {
            "x86_64" => "amd64",
            "aarch64" => "arm64",
            arch => anyhow::bail!("Unsupported architecture: {arch}"),
        };

        let filename = format!("go{version}.linux-{arch}.tar.gz");
        let url = format!("{GO_DOWNLOAD_URL}/{filename}");

        fs::create_dir_all(&self.versions_dir)?;

        // Fetch checksum for verification
        let checksum = self.fetch_checksum(&version, &filename).await.ok();

        tracing::info!("{} Downloading {}...", "→".blue(), filename);
        let download_path = self.versions_dir.join(&filename);
        download_with_progress(&self.client, &url, &download_path, checksum.as_deref()).await?;

        tracing::info!("{} Extracting (pure Rust)...", "→".blue());
        extract_tar_gz(&download_path, &version_dir, 1).await?;

        let _ = fs::remove_file(&download_path);

        print_installed("Go", &version);
        self.use_version(&version)?;

        Ok(())
    }

    /// Fetch SHA256 checksum from go.dev
    async fn fetch_checksum(&self, _version: &str, filename: &str) -> Result<String> {
        let url = format!("{GO_DOWNLOAD_URL}/{filename}.sha256");
        let response = self.client.get(&url).send().await?;
        let text = response.text().await?;
        Ok(text.trim().to_string())
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        let version_dir = self.versions_dir.join(&version);
        set_current_version(&self.versions_dir, &version)?;

        tracing::info!("{} Now using Go {}", "✓".green(), version);
        tracing::info!(
            "  {} {}",
            "GOROOT:".dimmed(),
            version_dir.display().to_string().dimmed()
        );
        tracing::info!(
            "  {} {}",
            "PATH:".dimmed(),
            self.bin_dir().display().to_string().dimmed()
        );

        Ok(())
    }

    /// Uninstall a version
    pub fn uninstall(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        let version_dir = self.versions_dir.join(&version);

        if !version_dir.exists() {
            tracing::info!("{} Go {} is not installed", "→".dimmed(), version);
            return Ok(());
        }

        if let Some(current) = self.current_version()
            && current == version
        {
            let _ = fs::remove_file(&self.current_link);
        }

        fs::remove_dir_all(&version_dir)?;
        tracing::info!("{} Go {} uninstalled", "✓".green(), version);
        Ok(())
    }
}

impl Default for GoManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_go_manager_new() {
        let mgr = GoManager::new();
        assert!(mgr.versions_dir.ends_with("go"));
    }
}
