//! Native Ruby runtime manager - PURE RUST
//!
//! Downloads pre-built Ruby binaries from ruby-builder.
//!
//! Features:
//! - Pre-built binaries (no compilation required)
//! - Compatible with Ubuntu/Debian glibc

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use super::common::{
    begin_staged_install, complete_staged_install, download_with_progress, extract_tar_gz,
    normalize_version, parse_sha256_digest, print_already_installed, print_installed, print_using,
    remove_file_best_effort, set_current_version, version_cmp,
};
use crate::core::http::download_client;

const RUBY_VERSIONS_URL: &str = "https://api.github.com/repos/ruby/ruby-builder/releases";

/// Ruby version info
#[derive(Debug, Clone)]
pub struct RubyVersion {
    pub version: String,
    pub prebuilt: bool,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    #[serde(default)]
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

pub struct RubyManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}

impl RubyManager {
    pub fn new() -> Self {
        let versions_dir = super::DATA_DIR.join("versions").join("ruby");
        Self {
            current_link: versions_dir.join("current"),
            versions_dir,
            client: download_client().clone(),
        }
    }

    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.current_link.join("bin")
    }

    /// List available Ruby versions from ruby-builder releases
    pub async fn list_available(&self) -> Result<Vec<RubyVersion>> {
        let releases: Vec<GithubRelease> = self
            .client
            .get(format!("{RUBY_VERSIONS_URL}?per_page=20"))
            .send()
            .await
            .context("Failed to fetch Ruby releases from GitHub")?
            .json()
            .await
            .context("Failed to parse Ruby release data")?;

        // Extract unique Ruby versions from release tags
        // Tags are like "toolcache" or version-specific
        let re = regex::Regex::new(r"^(\d+\.\d+\.\d+)$")?;

        let mut versions: std::collections::HashSet<_> = releases
            .iter()
            .filter_map(|release| {
                re.captures(&release.tag_name)
                    .and_then(|caps| caps.get(1))
                    .map(|m| m.as_str().to_owned())
            })
            .collect();

        // If no version tags found, return common stable versions
        if versions.is_empty() {
            versions.extend([
                "3.3.0".to_owned(),
                "3.2.2".to_owned(),
                "3.1.4".to_owned(),
                "3.0.6".to_owned(),
            ]);
        }

        let mut result: Vec<_> = versions
            .into_iter()
            .map(|version| RubyVersion {
                version,
                prebuilt: true,
            })
            .collect();

        result.sort_by(|a, b| version_cmp(&b.version, &a.version));
        Ok(result)
    }

    /// Install Ruby - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        crate::core::security::validate_runtime_version(&version)?;
        let version_dir = self.versions_dir.join(&version);

        if crate::runtimes::common::is_valid_version_dir(&version_dir) {
            print_already_installed("Ruby", &version);
            return self.use_version(&version);
        }

        println!(
            "{} Installing Ruby {}...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        // Use the release-specific, pre-built Ruby from GitHub ruby-builder.
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            arch => anyhow::bail!("Unsupported architecture: {arch}"),
        };
        let release: GithubRelease = self
            .client
            .get(format!("{RUBY_VERSIONS_URL}/tags/ruby-{version}"))
            .header("User-Agent", "omg-package-manager")
            .send()
            .await
            .context("Failed to fetch Ruby release metadata")?
            .error_for_status()
            .context("Ruby release metadata request failed")?
            .json()
            .await
            .context("Failed to parse Ruby release metadata")?;
        let expected_name = format!("ruby-{version}-ubuntu-22.04-{arch}.tar.gz");
        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == expected_name)
            .ok_or_else(|| anyhow::anyhow!("Ruby release asset not found: {expected_name}"))?;
        let checksum = asset
            .digest
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Ruby release asset has no SHA-256 digest"))
            .and_then(|digest| parse_sha256_digest(digest, "GitHub Ruby release"))?;

        fs::create_dir_all(&self.versions_dir)?;

        println!("{} Downloading pre-built Ruby {version}...", "→".blue());
        let download_path = self.versions_dir.join(&asset.name);

        download_with_progress(
            &self.client,
            &asset.browser_download_url,
            &download_path,
            Some(&checksum),
        )
        .await
        .with_context(|| {
            eprintln!("{} Pre-built Ruby {version} not available", "!".yellow());
            eprintln!("  Try: omg list ruby --available");
            format!("Failed to download Ruby {version}")
        })?;

        println!("{} Extracting (pure Rust)...", "→".blue());
        let staging = begin_staged_install(&self.versions_dir)?;
        extract_tar_gz(&download_path, staging.path(), 1).await?;
        complete_staged_install(&staging, &version_dir, &version)?;

        remove_file_best_effort(&download_path, "runtime archive");

        print_installed("Ruby", &version);
        self.use_version(&version)?;

        Ok(())
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        set_current_version(&self.versions_dir, &version)?;
        print_using("Ruby", &version, &self.bin_dir());
        Ok(())
    }
}

// Generate common runtime manager methods (list_installed, current_version, uninstall)
crate::impl_runtime_common!(RubyManager, "Ruby");

impl Default for RubyManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ruby_manager_new() {
        let mgr = RubyManager::new();
        assert!(mgr.versions_dir.ends_with("ruby"));
    }
}
