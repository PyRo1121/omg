//! Native Node.js runtime manager
//!
//! Downloads and manages Node.js versions - PURE RUST, NO SUBPROCESS.
//!
//! Features:
//! - Automatic LTS detection
//! - Checksum verification (SHASUMS256.txt)
//! - Pure Rust XZ extraction
//! - Version aliasing (latest, lts, lts/iron, etc.)

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use super::common::{
    download_with_progress, extract_tar_xz, get_current_version, list_installed_versions,
    normalize_version, print_already_installed, print_installed, print_using, set_current_version,
};
use crate::core::http::download_client;

const NODE_DIST_URL: &str = "https://nodejs.org/dist";

/// Node.js version info from nodejs.org
#[derive(Debug, Deserialize)]
pub struct NodeVersion {
    pub version: String,
    pub date: String,
    pub lts: serde_json::Value,
}

/// Node.js runtime manager
pub struct NodeManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}

impl NodeManager {
    pub fn new() -> Self {
        let data_dir = super::DATA_DIR.clone();

        Self {
            versions_dir: data_dir.join("versions").join("node"),
            current_link: data_dir.join("versions").join("node").join("current"),
            client: download_client().clone(),
        }
    }

    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.current_link.join("bin")
    }

    pub async fn list_available(&self) -> Result<Vec<NodeVersion>> {
        let url = format!("{NODE_DIST_URL}/index.json");

        let versions: Vec<NodeVersion> = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch Node.js version list. Check your internet connection.")?
            .json()
            .await
            .context("Failed to parse Node.js version list from nodejs.org")?;
        Ok(versions)
    }

    pub fn list_installed(&self) -> Result<Vec<String>> {
        list_installed_versions(&self.versions_dir)
    }

    #[must_use]
    pub fn current_version(&self) -> Option<String> {
        get_current_version(&self.versions_dir)
    }

    /// Resolve version alias (latest, lts) to actual version number
    pub async fn resolve_alias(&self, alias: &str) -> Result<String> {
        let alias = normalize_version(alias);

        if alias == "latest" {
            let versions = self.list_available().await?;
            if let Some(v) = versions.first() {
                return Ok(v.version.trim_start_matches('v').to_string());
            }
            anyhow::bail!("No Node.js versions found upstream");
        }

        if alias == "lts" {
            let versions = self.list_available().await?;
            for v in versions {
                if v.lts.is_string() {
                    return Ok(v.version.trim_start_matches('v').to_string());
                }
            }
            anyhow::bail!("No LTS version found");
        }

        Ok(alias)
    }

    /// Install Node.js - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = self.resolve_alias(version).await?;
        let version_dir = self.versions_dir.join(&version);

        if version_dir.exists() {
            print_already_installed("Node.js", &version);
            return self.use_version(&version);
        }

        println!(
            "{} Installing Node.js {}...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            arch => anyhow::bail!("Unsupported architecture: {arch}"),
        };

        let filename = format!("node-v{version}-linux-{arch}.tar.xz");
        let url = format!("{NODE_DIST_URL}/v{version}/{filename}");

        fs::create_dir_all(&self.versions_dir)?;

        // Fetch checksum for verification
        let checksum = self.fetch_checksum(&version, &filename).await.ok();

        println!("{} Downloading {}...", "→".blue(), filename);
        let download_path = self.versions_dir.join(&filename);
        download_with_progress(&self.client, &url, &download_path, checksum.as_deref()).await?;

        println!("{} Extracting (pure Rust)...", "→".blue());
        extract_tar_xz(&download_path, &version_dir, 1)?;

        let _ = fs::remove_file(&download_path);

        print_installed("Node.js", &version);
        self.use_version(&version)?;

        Ok(())
    }

    /// Fetch SHA256 checksum from nodejs.org
    async fn fetch_checksum(&self, version: &str, filename: &str) -> Result<String> {
        let url = format!("{NODE_DIST_URL}/v{version}/SHASUMS256.txt");
        let response = self.client.get(&url).send().await?;
        let text = response.text().await?;

        for line in text.lines() {
            if line.ends_with(filename)
                && let Some(hash) = line.split_whitespace().next()
            {
                return Ok(hash.to_string());
            }
        }

        anyhow::bail!("Checksum not found for {filename}")
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        set_current_version(&self.versions_dir, &version)?;
        print_using("Node.js", &version, &self.bin_dir());
        Ok(())
    }

    /// Uninstall a version
    pub fn uninstall(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        let version_dir = self.versions_dir.join(&version);

        if !version_dir.exists() {
            println!("{} Node.js {} is not installed", "→".dimmed(), version);
            return Ok(());
        }

        // Check if this is the current version
        if let Some(current) = self.current_version()
            && current == version
        {
            // Remove the symlink
            let _ = std::fs::remove_file(&self.current_link);
        }

        std::fs::remove_dir_all(&version_dir)?;
        println!("{} Node.js {} uninstalled", "✓".green(), version);

        Ok(())
    }
}

impl Default for NodeManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Get LTS version name if applicable
#[must_use]
pub fn get_lts_name(version: &NodeVersion) -> Option<String> {
    match &version.lts {
        serde_json::Value::String(s) => Some(s.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_manager_new() {
        let mgr = NodeManager::new();
        assert!(mgr.versions_dir.ends_with("node"));
    }

    #[test]
    fn test_get_lts_name() {
        let lts_version = NodeVersion {
            version: "v20.0.0".to_string(),
            date: "2024-01-01".to_string(),
            lts: serde_json::Value::String("Iron".to_string()),
        };
        assert_eq!(get_lts_name(&lts_version), Some("Iron".to_string()));

        let non_lts = NodeVersion {
            version: "v21.0.0".to_string(),
            date: "2024-01-01".to_string(),
            lts: serde_json::Value::Bool(false),
        };
        assert_eq!(get_lts_name(&non_lts), None);
    }
}
