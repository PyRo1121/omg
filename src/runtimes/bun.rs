//! Native Bun runtime manager - PURE RUST
//!
//! Downloads and manages Bun versions from GitHub.
//!
//! Features:
//! - Fast JavaScript/TypeScript runtime
//! - Pre-built binaries from GitHub releases
//! - Version aliasing (latest)

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use super::common::{
    download_with_progress, extract_zip, get_current_version, list_installed_versions,
    normalize_version, print_already_installed, print_installed, print_using, set_current_version,
};
use crate::core::http::download_client;

const BUN_RELEASES_URL: &str = "https://github.com/oven-sh/bun/releases/download";
const BUN_API_URL: &str = "https://api.github.com/repos/oven-sh/bun/releases";

/// Bun version info
#[derive(Debug, Clone)]
pub struct BunVersion {
    pub version: String,
    pub prerelease: bool,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    prerelease: bool,
}

pub struct BunManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}

impl BunManager {
    pub fn new() -> Self {
        let data_dir = super::DATA_DIR.clone();

        let client = download_client().clone();

        Self {
            versions_dir: data_dir.join("versions").join("bun"),
            current_link: data_dir.join("versions").join("bun").join("current"),
            client,
        }
    }

    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.current_link.clone()
    }

    /// List available Bun versions from GitHub releases
    pub async fn list_available(&self) -> Result<Vec<BunVersion>> {
        let releases: Vec<GithubRelease> = self
            .client
            .get(format!("{BUN_API_URL}?per_page=20"))
            .send()
            .await
            .context("Failed to fetch Bun releases from GitHub")?
            .json()
            .await
            .context("Failed to parse Bun release data")?;

        let versions: Vec<BunVersion> = releases
            .into_iter()
            .filter_map(|r| {
                // Tags are like "bun-v1.0.0"
                let version = r
                    .tag_name
                    .trim_start_matches("bun-v")
                    .trim_start_matches('v')
                    .to_string();

                if version.is_empty() {
                    None
                } else {
                    Some(BunVersion {
                        version,
                        prerelease: r.prerelease,
                    })
                }
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

    /// Resolve Bun alias (latest) to a concrete version
    pub async fn resolve_alias(&self, alias: &str) -> Result<String> {
        let alias = normalize_version(alias);
        if alias == "latest" {
            let versions = self.list_available().await?;
            if let Some(v) = versions.first() {
                return Ok(v.version.clone());
            }
            anyhow::bail!("No Bun versions found upstream");
        }
        Ok(alias)
    }

    /// Install Bun - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = self.resolve_alias(version).await?;
        let version_dir = self.versions_dir.join(&version);

        if version_dir.exists() {
            print_already_installed("Bun", &version);
            return self.use_version(&version);
        }

        println!(
            "{} Installing Bun {}...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        let arch = match std::env::consts::ARCH {
            "x86_64" => "linux-x64",
            "aarch64" => "linux-aarch64",
            arch => anyhow::bail!("Unsupported architecture: {arch}"),
        };

        let filename = format!("bun-{arch}.zip");
        let url = format!("{BUN_RELEASES_URL}/bun-v{version}/{filename}");

        fs::create_dir_all(&self.versions_dir)?;

        println!("{} Downloading Bun v{}...", "→".blue(), version);
        let download_path = self.versions_dir.join(&filename);
        download_with_progress(&self.client, &url, &download_path, None).await?;

        println!("{} Extracting (pure Rust)...", "→".blue());
        extract_zip(&download_path, &version_dir, 1)?;

        let _ = fs::remove_file(&download_path);

        print_installed("Bun", &version);
        self.use_version(&version)?;

        Ok(())
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        set_current_version(&self.versions_dir, &version)?;
        print_using("Bun", &version, &self.bin_dir());
        Ok(())
    }

    /// Uninstall a version
    pub fn uninstall(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        let version_dir = self.versions_dir.join(&version);

        if !version_dir.exists() {
            println!("{} Bun {} is not installed", "→".dimmed(), version);
            return Ok(());
        }

        if let Some(current) = self.current_version()
            && current == version
        {
            let _ = fs::remove_file(&self.current_link);
        }

        fs::remove_dir_all(&version_dir)?;
        println!("{} Bun {} uninstalled", "✓".green(), version);
        Ok(())
    }
}

impl Default for BunManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bun_manager_new() {
        let mgr = BunManager::new();
        assert!(mgr.versions_dir.ends_with("bun"));
    }
}
