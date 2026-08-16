//! Native Python runtime manager - PURE RUST
//!
//! Downloads pre-built Python binaries from python-build-standalone.
//!
//! Features:
//! - Pre-built binaries (no compilation required)
//! - Automatic version detection
//! - Virtual environment support

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use super::common::{
    activate_version, begin_staged_install, complete_staged_install, download_with_progress,
    extract_tar_gz, normalize_version, parse_sha256_digest, print_already_installed,
    print_installed, print_using, remove_file_best_effort, version_cmp,
};
use crate::core::http::download_client;

const PBS_RELEASES_URL: &str =
    "https://api.github.com/repos/indygreg/python-build-standalone/releases";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    digest: Option<String>,
}

/// Python version info for available versions
#[derive(Debug, Clone)]
pub struct PythonVersion {
    pub version: String,
    pub prebuilt: bool,
}

pub struct PythonManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}

impl PythonManager {
    pub fn new() -> Self {
        let data_dir = &*super::DATA_DIR;
        let versions_dir = data_dir.join("versions").join("python");

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

    /// List available Python versions from python-build-standalone
    pub async fn list_available(&self) -> Result<Vec<PythonVersion>> {
        if crate::core::paths::test_mode() {
            return Ok(vec![
                PythonVersion {
                    version: "3.12.0".to_string(),
                    prebuilt: true,
                },
                PythonVersion {
                    version: "3.11.0".to_string(),
                    prebuilt: true,
                },
            ]);
        }
        let releases: Vec<GithubRelease> = self
            .client
            .get(format!("{PBS_RELEASES_URL}?per_page=10"))
            .send()
            .await
            .context("Failed to fetch Python releases from GitHub")?
            .json()
            .await
            .context("Failed to parse Python release data")?;

        let arch = match std::env::consts::ARCH {
            "aarch64" => "aarch64",
            _ => "x86_64",
        };

        let mut versions = std::collections::HashSet::new();

        for release in &releases {
            for asset in &release.assets {
                // Only include assets that match our architecture and are install_only
                if asset.name.contains(arch)
                    && asset.name.contains("linux-gnu")
                    && asset.name.contains("install_only")
                    && let Some(version) = Self::extract_cpython_version(&asset.name)
                {
                    versions.insert(version);
                }
            }
        }

        let mut result: Vec<PythonVersion> = versions
            .into_iter()
            .map(|v| PythonVersion {
                version: v,
                prebuilt: true,
            })
            .collect();

        result.sort_unstable_by(|a, b| version_cmp(&b.version, &a.version));
        Ok(result)
    }

    fn extract_cpython_version(name: &str) -> Option<String> {
        let (_, tail) = name.split_once("cpython-")?;
        let version = tail
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>();
        Self::is_semver_like(&version).then_some(version)
    }

    fn is_semver_like(value: &str) -> bool {
        let mut parts = value.split('.');
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(major), Some(minor), Some(patch), None) => [major, minor, patch]
                .iter()
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())),
            _ => false,
        }
    }

    /// Install Python - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        crate::core::security::validate_runtime_version(&version)?;
        let version_dir = self.versions_dir.join(&version);

        if crate::runtimes::common::is_valid_version_dir(&version_dir) {
            print_already_installed("Python", &version);
            return self.use_version(&version);
        }

        if crate::core::paths::test_mode() {
            fs::create_dir_all(version_dir.join("bin"))?;
            fs::write(version_dir.join("bin/python3"), "mock")?;
            print_installed("Python", &version);
            return self.use_version(&version);
        }

        println!(
            "{} Installing Python {}...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        let arch = match std::env::consts::ARCH {
            "x86_64" => "x86_64",
            "aarch64" => "aarch64",
            arch => anyhow::bail!("Unsupported architecture: {arch}"),
        };

        println!("{} Finding Python {} release...", "→".blue(), version);

        let releases: Vec<GithubRelease> = self
            .client
            .get(PBS_RELEASES_URL)
            .header("User-Agent", "omg-package-manager")
            .send()
            .await
            .context("Failed to fetch Python releases")?
            .json()
            .await
            .context("Failed to parse Python release data")?;

        let python_prefix = format!("cpython-{version}");
        let asset = releases
            .iter()
            .flat_map(|release| &release.assets)
            .find(|asset| {
                asset.name.contains(&python_prefix)
                    && asset.name.contains(arch)
                    && asset.name.contains("linux-gnu")
                    && asset.name.contains("install_only")
                    && asset.name.ends_with(".tar.gz")
            })
            .ok_or_else(|| {
                anyhow::anyhow!("Python {version} not found. Try: omg list python --available")
            })?;

        let url = &asset.browser_download_url;
        let asset_name = &asset.name;
        let checksum = asset
            .digest
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Python release asset has no SHA-256 digest"))
            .and_then(|digest| parse_sha256_digest(digest, "GitHub Python release"))?;

        fs::create_dir_all(&self.versions_dir)?;

        println!("{} Downloading {}...", "→".blue(), asset_name);
        let download_path = self.versions_dir.join(asset_name);
        download_with_progress(&self.client, url, &download_path, Some(&checksum)).await?;

        println!("{} Extracting (pure Rust)...", "→".blue());
        let staging = begin_staged_install(&self.versions_dir)?;
        extract_tar_gz(&download_path, staging.path(), 1).await?;
        complete_staged_install(&staging, &version_dir, &version)?;

        remove_file_best_effort(&download_path, "runtime archive");

        print_installed("Python", &version);
        self.use_version(&version)?;

        Ok(())
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        activate_version(&self.versions_dir, &version, Path::new("bin/python3"))?;
        print_using("Python", &version, &self.bin_dir());
        Ok(())
    }
}

// Generate common runtime manager methods (list_installed, current_version, uninstall)
crate::impl_runtime_common!(PythonManager, "Python");

impl Default for PythonManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_python_manager_new() {
        let mgr = PythonManager::new();
        assert!(mgr.versions_dir.ends_with("python"));
    }

    #[test]
    fn test_extract_cpython_version() {
        assert_eq!(
            PythonManager::extract_cpython_version(
                "cpython-3.12.0+20231002-x86_64-unknown-linux-gnu-install_only.tar.gz"
            ),
            Some("3.12.0".to_string())
        );
        assert_eq!(
            PythonManager::extract_cpython_version("cpython-3.11.5-x86_64.tar.gz"),
            Some("3.11.5".to_string())
        );
    }

    #[test]
    fn test_is_semver_like() {
        assert!(PythonManager::is_semver_like("3.12.0"));
        assert!(PythonManager::is_semver_like("3.11.5"));
        assert!(!PythonManager::is_semver_like("3.12"));
        assert!(!PythonManager::is_semver_like("3"));
    }
}
