//! Native Python runtime manager - PURE RUST
//!
//! Downloads pre-built Python binaries from python-build-standalone.

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::fs::{self, File};
use std::path::PathBuf;
use tar::Archive;

use crate::core::http::download_client;
const PBS_RELEASES_URL: &str =
    "https://api.github.com/repos/indygreg/python-build-standalone/releases";

#[derive(Debug, Deserialize)]
struct GithubRelease {
    #[allow(dead_code)]
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
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
        let data_dir = super::DATA_DIR.clone();

        let client = download_client().clone();

        Self {
            versions_dir: data_dir.join("versions").join("python"),
            current_link: data_dir.join("versions").join("python").join("current"),
            client,
        }
    }

    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.current_link.join("bin")
    }

    /// List available Python versions from python-build-standalone
    pub async fn list_available(&self) -> Result<Vec<PythonVersion>> {
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

        result.sort_by(|a, b| version_cmp(&b.version, &a.version));
        Ok(result)
    }

    fn extract_cpython_version(name: &str) -> Option<String> {
        let (_, tail) = name.split_once("cpython-")?;
        let version = tail
            .chars()
            .take_while(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>();
        if Self::is_semver_like(&version) {
            Some(version)
        } else {
            None
        }
    }

    fn is_semver_like(value: &str) -> bool {
        let mut parts = value.split('.');
        let Some(major) = parts.next() else {
            return false;
        };
        let Some(minor) = parts.next() else {
            return false;
        };
        let Some(patch) = parts.next() else {
            return false;
        };
        if parts.next().is_some() {
            return false;
        }
        [major, minor, patch]
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
    }

    pub fn list_installed(&self) -> Result<Vec<String>> {
        if !self.versions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();
        for entry in fs::read_dir(&self.versions_dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "current" && entry.file_type()?.is_dir() {
                versions.push(name);
            }
        }
        versions.sort_by(|a, b| version_cmp(b, a));
        Ok(versions)
    }

    #[must_use]
    pub fn current_version(&self) -> Option<String> {
        fs::read_link(&self.current_link)
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
    }

    /// Install Python - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = version.trim_start_matches('v');
        let version_dir = self.versions_dir.join(version);

        if version_dir.exists() {
            println!("{} Python {} is already installed", "✓".green(), version);
            return self.use_version(version);
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
            .send()
            .await
            .context("Failed to fetch Python releases")?
            .json()
            .await
            .context("Failed to parse Python release data")?;

        let mut download_url = None;
        let mut asset_name = String::new();

        for release in &releases {
            for asset in &release.assets {
                if asset.name.contains(&format!("cpython-{version}"))
                    && asset.name.contains(arch)
                    && asset.name.contains("linux-gnu")
                    && asset.name.contains("install_only")
                    && asset.name.ends_with(".tar.gz")
                {
                    download_url = Some(asset.browser_download_url.clone());
                    asset_name.clone_from(&asset.name);
                    break;
                }
            }
            if download_url.is_some() {
                break;
            }
        }

        let url = download_url.ok_or_else(|| {
            anyhow::anyhow!("Python {version} not found in python-build-standalone releases")
        })?;

        fs::create_dir_all(&self.versions_dir)?;

        println!("{} Downloading {}...", "→".blue(), asset_name);
        let download_path = self.versions_dir.join(&asset_name);
        self.download_file(&url, &download_path).await?;

        // PURE RUST EXTRACTION
        println!("{} Extracting (pure Rust)...", "→".blue());

        let file = File::open(&download_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        // Extract with stripping python/ prefix
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;

            let stripped: PathBuf = path.components().skip(1).collect();
            if stripped.as_os_str().is_empty() {
                continue;
            }

            let dest_path = version_dir.join(&stripped);

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else {
                entry.unpack(&dest_path)?;
            }
        }

        let _ = fs::remove_file(&download_path);

        println!("{} Python {} installed!", "✓".green(), version);
        self.use_version(version)?;

        Ok(())
    }

    async fn download_file(&self, url: &str, path: &PathBuf) -> Result<()> {
        let response =
            self.client.get(url).send().await.with_context(|| {
                format!("Failed to download from {url}. Check your connection.")
            })?;

        let total = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .expect("valid template")
                .progress_chars("█▓▒░"),
        );

        let bytes = response
            .bytes()
            .await
            .context("Failed to read download stream")?;
        pb.set_position(bytes.len() as u64);
        tokio::fs::write(path, &bytes)
            .await
            .with_context(|| format!("Failed to write to {}. Check disk space.", path.display()))?;
        pb.finish_and_clear();
        Ok(())
    }

    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = version.trim_start_matches('v');
        let version_dir = self.versions_dir.join(version);

        if !version_dir.exists() {
            anyhow::bail!("Python {version} is not installed");
        }

        if self.current_link.exists() {
            fs::remove_file(&self.current_link)?;
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(&version_dir, &self.current_link)?;

        println!("{} Now using Python {}", "✓".green(), version);
        println!(
            "  Add to PATH: {}",
            self.bin_dir().display().to_string().dimmed()
        );

        Ok(())
    }

    pub fn uninstall(&self, version: &str) -> Result<()> {
        let version = version.trim_start_matches('v');
        let version_dir = self.versions_dir.join(version);

        if !version_dir.exists() {
            println!("{} Python {} is not installed", "→".dimmed(), version);
            return Ok(());
        }

        if let Some(current) = self.current_version()
            && current == version
        {
            let _ = fs::remove_file(&self.current_link);
        }

        fs::remove_dir_all(&version_dir)?;
        println!("{} Python {} uninstalled", "✓".green(), version);
        Ok(())
    }
}

impl Default for PythonManager {
    fn default() -> Self {
        Self::new()
    }
}

fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<u32> = a.split('.').filter_map(|p| p.parse().ok()).collect();
    let b_parts: Vec<u32> = b.split('.').filter_map(|p| p.parse().ok()).collect();

    for i in 0..3 {
        let a_part = a_parts.get(i).unwrap_or(&0);
        let b_part = b_parts.get(i).unwrap_or(&0);
        if a_part != b_part {
            return a_part.cmp(b_part);
        }
    }
    std::cmp::Ordering::Equal
}
