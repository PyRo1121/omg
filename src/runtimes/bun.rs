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
    begin_staged_install, complete_staged_install, download_with_progress, extract_zip,
    normalize_version, parse_sha256_digest, print_already_installed, print_installed, print_using,
    remove_file_best_effort, set_current_version,
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
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    digest: Option<String>,
}

pub struct BunManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}

impl BunManager {
    pub fn new() -> Self {
        let versions_dir = super::DATA_DIR.join("versions").join("bun");
        Self {
            current_link: versions_dir.join("current"),
            versions_dir,
            client: download_client().clone(),
        }
    }

    #[must_use]
    pub fn bin_dir(&self) -> &PathBuf {
        &self.current_link
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

        Ok(releases
            .into_iter()
            .filter_map(|r| {
                // Tags are like "bun-v1.0.0"
                let version = r
                    .tag_name
                    .strip_prefix("bun-v")
                    .or_else(|| r.tag_name.strip_prefix('v'))
                    .unwrap_or(&r.tag_name);

                (!version.is_empty()).then(|| BunVersion {
                    version: version.to_owned(),
                    prerelease: r.prerelease,
                })
            })
            .collect())
    }

    /// Resolve Bun alias (latest) to a concrete version
    pub async fn resolve_alias(&self, alias: &str) -> Result<String> {
        let alias = normalize_version(alias);
        if alias == "latest" {
            let versions = self.list_available().await?;
            versions
                .first()
                .map(|v| v.version.clone())
                .context("No Bun versions found upstream")
        } else {
            Ok(alias)
        }
    }

    /// Install Bun - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = self.resolve_alias(version).await?;
        crate::core::security::validate_runtime_version(&version)?;
        let version_dir = self.versions_dir.join(&version);

        if crate::runtimes::common::is_valid_version_dir(&version_dir) {
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
        let checksum = self.fetch_checksum(&version, &filename).await?;

        fs::create_dir_all(&self.versions_dir)?;

        println!("{} Downloading Bun v{}...", "→".blue(), version);
        let download_path = self.versions_dir.join(&filename);
        download_with_progress(&self.client, &url, &download_path, Some(&checksum)).await?;

        println!("{} Extracting (pure Rust)...", "→".blue());
        let staging = begin_staged_install(&self.versions_dir)?;
        extract_zip(&download_path, staging.path(), 1).await?;
        complete_staged_install(&staging, &version_dir, &version)?;

        remove_file_best_effort(&download_path, "runtime archive");

        print_installed("Bun", &version);
        self.use_version(&version)?;

        Ok(())
    }

    async fn fetch_checksum(&self, version: &str, filename: &str) -> Result<String> {
        let release: GithubRelease = self
            .client
            .get(format!("{BUN_API_URL}/tags/bun-v{version}"))
            .header("User-Agent", "omg-package-manager")
            .send()
            .await
            .context("Failed to fetch Bun release metadata")?
            .error_for_status()
            .context("Bun release metadata request failed")?
            .json()
            .await
            .context("Failed to parse Bun release metadata")?;

        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == filename)
            .ok_or_else(|| anyhow::anyhow!("Bun release asset not found: {filename}"))?;
        let digest = asset.digest.as_deref().ok_or_else(|| {
            anyhow::anyhow!("Bun release asset has no SHA-256 digest: {filename}")
        })?;
        parse_sha256_digest(digest, "GitHub Bun release")
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        set_current_version(&self.versions_dir, &version)?;
        print_using("Bun", &version, self.bin_dir());
        Ok(())
    }
}

// Generate common runtime manager methods (list_installed, current_version, uninstall)
crate::impl_runtime_common!(BunManager, "Bun");

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
