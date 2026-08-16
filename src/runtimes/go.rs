//! Native Go runtime manager - PURE RUST
//!
//! Downloads and manages Go versions from go.dev.
//!
//! Features:
//! - Official binaries from go.dev
//! - Checksum verification (SHA256)
//! - GOROOT auto-configuration

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Deserialize;

use super::common::{
    begin_staged_install, complete_staged_install, download_with_progress, extract_tar_gz,
    normalize_version, parse_sha256_digest, print_already_installed, print_installed,
    remove_file_best_effort, set_current_version,
};
use crate::core::http::download_client;

const GO_DOWNLOAD_URL: &str = "https://go.dev/dl";
const GO_VERSIONS_URL: &str = "https://go.dev/dl/?mode=json";

/// Go version info from go.dev
#[derive(Debug, Clone, Deserialize)]
pub struct GoVersion {
    version: String,
    stable: bool,
}

impl GoVersion {
    /// Get the version string without the "go" prefix
    #[must_use]
    pub fn version(&self) -> &str {
        self.version.trim_start_matches("go")
    }

    #[must_use]
    pub const fn stable(&self) -> bool {
        self.stable
    }
}

pub struct GoManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: &'static reqwest::Client,
}

impl GoManager {
    pub fn new() -> Self {
        let versions_dir = super::DATA_DIR.join("versions/go");

        Self {
            current_link: versions_dir.join("current"),
            versions_dir,
            client: download_client(),
        }
    }

    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.current_link.join("bin")
    }

    /// List available Go versions from go.dev
    pub async fn list_available(&self) -> Result<Vec<GoVersion>> {
        self.client
            .get(GO_VERSIONS_URL)
            .send()
            .await
            .context("Failed to fetch Go version list. Check your internet connection.")?
            .json()
            .await
            .context("Failed to parse Go version list from go.dev")
    }

    /// Install Go - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        crate::core::security::validate_runtime_version(&version)?;
        let version_dir = self.versions_dir.join(&version);

        if crate::runtimes::common::is_valid_version_dir(&version_dir) {
            print_already_installed("Go", &version);
            return self.use_version(&version);
        }

        println!(
            "{} Installing Go {}...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        let arch = Self::detect_architecture()?;
        let filename = format!("go{version}.linux-{arch}.tar.gz");
        let url = format!("{GO_DOWNLOAD_URL}/{filename}");

        fs::create_dir_all(&self.versions_dir)?;

        // A vendor checksum is required before installing a downloaded runtime.
        let checksum = self.fetch_checksum(&filename).await?;

        println!("{} Downloading {filename}...", "→".blue());
        let download_path = self.versions_dir.join(&filename);
        download_with_progress(self.client, &url, &download_path, Some(&checksum)).await?;

        println!("{} Extracting (pure Rust)...", "→".blue());
        let staging = begin_staged_install(&self.versions_dir)?;
        extract_tar_gz(&download_path, staging.path(), 1).await?;
        complete_staged_install(&staging, &version_dir, &version)?;

        remove_file_best_effort(&download_path, "runtime archive");

        print_installed("Go", &version);
        self.use_version(&version)
    }

    fn detect_architecture() -> Result<&'static str> {
        match std::env::consts::ARCH {
            "x86_64" => Ok("amd64"),
            "aarch64" => Ok("arm64"),
            arch => anyhow::bail!("Unsupported architecture: {arch}"),
        }
    }

    /// Fetch SHA256 checksum from go.dev
    async fn fetch_checksum(&self, filename: &str) -> Result<String> {
        let url = format!("{GO_DOWNLOAD_URL}/{filename}.sha256");
        let text = self
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()
            .context("Failed to fetch Go checksum")?
            .text()
            .await?;
        parse_sha256_digest(&text, &url)
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        let version_dir = self.versions_dir.join(&version);
        set_current_version(&self.versions_dir, &version)?;

        Self::print_version_info(&version, &version_dir, &self.bin_dir());
        Ok(())
    }

    fn print_version_info(version: &str, goroot: &Path, bin_dir: &Path) {
        println!("{} Now using Go {version}", "✓".green());
        println!("  {} {}", "GOROOT:".dimmed(), goroot.display().dimmed());
        println!("  {} {}", "PATH:".dimmed(), bin_dir.display().dimmed());
    }
}

// Generate common runtime manager methods (list_installed, current_version, uninstall)
crate::impl_runtime_common!(GoManager, "Go");

impl Default for GoManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_go_manager_new() {
        let mgr = GoManager::new();
        assert!(mgr.versions_dir.ends_with("go"));
    }

    #[test]
    fn uninstall_rejects_parent_directory_versions_before_deletion() -> Result<()> {
        let temp = TempDir::new()?;
        let versions_dir = temp.path().join("versions");
        fs::create_dir(&versions_dir)?;
        let sentinel = temp.path().join("sentinel");
        fs::write(&sentinel, "preserve")?;

        let mut manager = GoManager::new();
        manager.versions_dir = versions_dir.clone();
        manager.current_link = versions_dir.join("current");

        assert!(manager.uninstall("..").is_err());
        assert_eq!(fs::read_to_string(sentinel)?, "preserve");
        assert!(versions_dir.is_dir());
        Ok(())
    }

    #[tokio::test]
    async fn install_rejects_parent_directory_versions_before_network_access() -> Result<()> {
        let manager = GoManager::new();
        assert!(manager.install("..").await.is_err());
        Ok(())
    }
}
