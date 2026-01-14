//! Native Node.js runtime manager
//!
//! Downloads and manages Node.js versions - PURE RUST, NO SUBPROCESS.

use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::fs::{self, File};
use std::path::PathBuf;
use tar::Archive;

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
            client: reqwest::Client::new(),
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
            println!("{} Node.js {} is already installed", "✓".green(), version);
            return Ok(());
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

        println!("{} Downloading {}...", "→".blue(), filename);
        let download_path = self.versions_dir.join(&filename);
        self.download_file(&url, &download_path).await?;

        // PURE RUST EXTRACTION - NO SUBPROCESS
        println!("{} Extracting (pure Rust)...", "→".blue());

        let file = File::open(&download_path)
            .with_context(|| format!("Failed to open: {}", download_path.display()))?;

        let decoder = xz2::read::XzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        // Extract with stripping top-level directory
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;

            // Strip first component (node-v22.0.0-linux-x64/)
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

        println!("{} Node.js {} installed!", "✓".green(), version);
        self.use_version(&version)?;

        Ok(())
    }

    /// Download a file with progress bar
    async fn download_file(&self, url: &str, path: &PathBuf) -> Result<()> {
        let response =
            self.client.get(url).send().await.with_context(|| {
                format!("Failed to download from {url}. Check your connection.")
            })?;

        let total_size = response.content_length().unwrap_or(0);

        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap()
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

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = normalize_version(version);
        let version_dir = self.versions_dir.join(&version);

        if !version_dir.exists() {
            anyhow::bail!("Node.js {version} is not installed. Run: omg install node@{version}");
        }

        // Remove existing symlink
        if self.current_link.exists() {
            std::fs::remove_file(&self.current_link)?;
        }

        // Create new symlink
        #[cfg(unix)]
        std::os::unix::fs::symlink(&version_dir, &self.current_link)?;

        println!("{} Now using Node.js {}", "✓".green(), version);
        println!(
            "  Add to PATH: {}",
            self.bin_dir().display().to_string().dimmed()
        );

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
        if let Some(current) = self.current_version() {
            if current == version {
                // Remove the symlink
                let _ = std::fs::remove_file(&self.current_link);
            }
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

/// Normalize version string (remove leading 'v' if present)
fn normalize_version(version: &str) -> String {
    version.trim_start_matches('v').to_string()
}

/// Compare version strings
fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let a_parts: Vec<u32> = a.split('.').filter_map(|p| p.parse().ok()).collect();
    let b_parts: Vec<u32> = b.split('.').filter_map(|p| p.parse().ok()).collect();

    for i in 0..3 {
        let a_part = a_parts.get(i).unwrap_or(&0);
        let b_part = b_parts.get(i).unwrap_or(&0);

        match a_part.cmp(b_part) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }

    std::cmp::Ordering::Equal
}

/// Get LTS version name if applicable
#[must_use]
pub fn get_lts_name(version: &NodeVersion) -> Option<String> {
    match &version.lts {
        serde_json::Value::String(s) => Some(s.clone()),
        _ => None,
    }
}
