//! Native Go runtime manager - PURE RUST
//!
//! Downloads and manages Go versions from go.dev.

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::fs::{self, File};
use std::path::PathBuf;
use tar::Archive;

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

    /// Install Go - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = version.trim_start_matches('v');
        let version_dir = self.versions_dir.join(version);

        if version_dir.exists() {
            println!("{} Go {} is already installed", "✓".green(), version);
            return self.use_version(version);
        }

        println!(
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

        println!("{} Downloading {}...", "→".blue(), filename);
        let download_path = self.versions_dir.join(&filename);
        self.download_file(&url, &download_path).await?;

        // PURE RUST EXTRACTION
        println!("{} Extracting (pure Rust)...", "→".blue());

        let file = File::open(&download_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        // Extract with stripping go/ prefix
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

        println!("{} Go {} installed!", "✓".green(), version);
        self.use_version(version)?;

        Ok(())
    }

    async fn download_file(&self, url: &str, path: &PathBuf) -> Result<()> {
        let response =
            self.client.get(url).send().await.with_context(|| {
                format!("Failed to download from {url}. Check your connection.")
            })?;

        if !response.status().is_success() {
            anyhow::bail!(
                "✗ Download Error: Server returned {} for {}",
                response.status(),
                url
            );
        }

        let total = response.content_length().unwrap_or(0);
        let pb = ProgressBar::new(total);
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

    pub fn use_version(&self, version: &str) -> Result<()> {
        let version = version.trim_start_matches('v');
        let version_dir = self.versions_dir.join(version);

        if !version_dir.exists() {
            anyhow::bail!("Go {version} is not installed");
        }

        if self.current_link.exists() {
            fs::remove_file(&self.current_link)?;
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(&version_dir, &self.current_link)?;

        println!("{} Now using Go {}", "✓".green(), version);
        println!(
            "  Set GOROOT: {}",
            version_dir.display().to_string().dimmed()
        );
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
            println!("{} Go {} is not installed", "→".dimmed(), version);
            return Ok(());
        }

        if let Some(current) = self.current_version() {
            if current == version {
                let _ = fs::remove_file(&self.current_link);
            }
        }

        fs::remove_dir_all(&version_dir)?;
        println!("{} Go {} uninstalled", "✓".green(), version);
        Ok(())
    }
}

impl Default for GoManager {
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
