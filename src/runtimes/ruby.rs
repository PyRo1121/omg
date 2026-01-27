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
    download_with_progress, extract_tar_gz, get_current_version, list_installed_versions,
    normalize_version, print_already_installed, print_installed, print_using, set_current_version,
    version_cmp,
};
use crate::core::http::download_client;

const RUBY_PREBUILT_URL: &str = "https://github.com/ruby/ruby-builder/releases/download";
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

    pub fn list_installed(&self) -> Result<Vec<String>> {
        list_installed_versions(&self.versions_dir)
    }

    #[must_use]
    pub fn current_version(&self) -> Option<String> {
        get_current_version(&self.versions_dir)
    }

    /// Install Ruby - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        let version_dir = self.versions_dir.join(&version);

        if version_dir.exists() {
            print_already_installed("Ruby", &version);
            return self.use_version(&version);
        }

        println!(
            "{} Installing Ruby {}...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        // Use pre-built Ruby from GitHub ruby-builder
        const OS: &str = "ubuntu-22.04"; // Most compatible glibc version
        let filename = format!("ruby-{version}.tar.gz");
        let url = format!("{RUBY_PREBUILT_URL}/toolcache/{OS}-{filename}");

        fs::create_dir_all(&self.versions_dir)?;

        println!("{} Downloading pre-built Ruby {version}...", "→".blue());
        let download_path = self.versions_dir.join(&filename);

        download_with_progress(&self.client, &url, &download_path, None)
            .await
            .with_context(|| {
                eprintln!(
                    "{} Pre-built Ruby {version} not available",
                    "!".yellow()
                );
                eprintln!("  Try: omg list ruby --available");
                format!("Failed to download Ruby {version}")
            })?;

        println!("{} Extracting (pure Rust)...", "→".blue());
        extract_tar_gz(&download_path, &version_dir, 1).await?;

        let _ = fs::remove_file(&download_path);

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

    /// Uninstall a version
    pub fn uninstall(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        let version_dir = self.versions_dir.join(&version);

        if !version_dir.exists() {
            println!("{} Ruby {version} is not installed", "→".dimmed());
            return Ok(());
        }

        if self.current_version().is_some_and(|current| current == version) {
            let _ = fs::remove_file(&self.current_link);
        }

        fs::remove_dir_all(&version_dir)?;
        println!("{} Ruby {version} uninstalled", "✓".green());
        Ok(())
    }
}

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
