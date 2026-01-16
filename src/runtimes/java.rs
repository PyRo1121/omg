//! Native Java runtime manager - PURE RUST
//!
//! Downloads JDK from Eclipse Adoptium.

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::fs::{self, File};
use std::path::PathBuf;
use tar::Archive;

use crate::core::http::download_client;
const ADOPTIUM_API: &str = "https://api.adoptium.net/v3";

#[derive(Debug, Deserialize)]
struct AdoptiumBinary {
    package: AdoptiumPackage,
}

#[derive(Debug, Deserialize)]
struct AdoptiumPackage {
    link: String,
    name: String,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AdoptiumVersionInfo {
    major: u32,
    minor: u32,
    security: u32,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct AdoptiumRelease {
    version_data: AdoptiumVersionInfo,
}

/// Java version info
#[derive(Debug, Clone)]
pub struct JavaVersion {
    pub version: String,
    pub lts: bool,
}

pub struct JavaManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}

impl JavaManager {
    pub fn new() -> Self {
        let data_dir = super::DATA_DIR.clone();

        let client = download_client().clone();

        Self {
            versions_dir: data_dir.join("versions").join("java"),
            current_link: data_dir.join("versions").join("java").join("current"),
            client,
        }
    }

    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.current_link.join("bin")
    }

    /// List available Java versions from Adoptium
    pub async fn list_available(&self) -> Result<Vec<JavaVersion>> {
        // Get available feature versions (major versions)
        let available_url = format!("{ADOPTIUM_API}/info/available_releases");

        #[derive(Deserialize)]
        struct AvailableReleases {
            available_lts_releases: Vec<u32>,
            available_releases: Vec<u32>,
        }

        let releases: AvailableReleases = self
            .client
            .get(&available_url)
            .send()
            .await
            .context("Failed to fetch Java versions from Adoptium")?
            .json()
            .await
            .context("Failed to parse Java version data")?;

        let lts_set: std::collections::HashSet<u32> =
            releases.available_lts_releases.into_iter().collect();

        let mut versions: Vec<JavaVersion> = releases
            .available_releases
            .into_iter()
            .map(|v| JavaVersion {
                version: v.to_string(),
                lts: lts_set.contains(&v),
            })
            .collect();

        // Sort descending
        versions.sort_by(|a, b| {
            b.version
                .parse::<u32>()
                .unwrap_or(0)
                .cmp(&a.version.parse::<u32>().unwrap_or(0))
        });

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
        versions.sort_by(|a, b| b.cmp(a));
        Ok(versions)
    }

    #[must_use]
    pub fn current_version(&self) -> Option<String> {
        fs::read_link(&self.current_link)
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
    }

    /// Install Java - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version_dir = self.versions_dir.join(version);

        if version_dir.exists() {
            println!("{} Java {} is already installed", "✓".green(), version);
            return self.use_version(version);
        }

        println!(
            "{} Installing Java {} (Adoptium)...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "aarch64",
            arch => anyhow::bail!("Unsupported architecture: {arch}"),
        };

        // Query Adoptium API
        let api_url = format!(
            "{ADOPTIUM_API}/assets/latest/{version}/hotspot?architecture={arch}&image_type=jdk&os=linux&vendor=eclipse"
        );

        println!("{} Querying Adoptium API...", "→".blue());

        let binaries: Vec<AdoptiumBinary> = self
            .client
            .get(&api_url)
            .send()
            .await
            .context("Failed to fetch JDK data from Adoptium")?
            .json()
            .await
            .context("Failed to parse JDK data")?;

        let binary = binaries
            .first()
            .ok_or_else(|| anyhow::anyhow!("No JDK {version} found for {arch}"))?;

        fs::create_dir_all(&self.versions_dir)?;

        println!("{} Downloading {}...", "→".blue(), binary.package.name);
        let download_path = self.versions_dir.join(&binary.package.name);
        self.download_file(&binary.package.link, &download_path)
            .await?;

        // PURE RUST EXTRACTION
        println!("{} Extracting (pure Rust)...", "→".blue());

        let file = File::open(&download_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        // Extract with stripping top-level directory
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

        println!("{} Java {} installed!", "✓".green(), version);
        self.use_version(version)?;

        Ok(())
    }

    #[allow(clippy::expect_used)]
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
        let version_dir = self.versions_dir.join(version);

        if !version_dir.exists() {
            anyhow::bail!("Java {version} is not installed");
        }

        if self.current_link.exists() {
            fs::remove_file(&self.current_link)?;
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(&version_dir, &self.current_link)?;

        println!("{} Now using Java {}", "✓".green(), version);
        println!(
            "  Set JAVA_HOME: {}",
            version_dir.display().to_string().dimmed()
        );
        println!(
            "  Add to PATH: {}",
            self.bin_dir().display().to_string().dimmed()
        );

        Ok(())
    }

    pub fn uninstall(&self, version: &str) -> Result<()> {
        let version_dir = self.versions_dir.join(version);

        if !version_dir.exists() {
            println!("{} Java {} is not installed", "→".dimmed(), version);
            return Ok(());
        }

        if let Some(current) = self.current_version()
            && current == version
        {
            let _ = fs::remove_file(&self.current_link);
        }

        fs::remove_dir_all(&version_dir)?;
        println!("{} Java {} uninstalled", "✓".green(), version);
        Ok(())
    }
}

impl Default for JavaManager {
    fn default() -> Self {
        Self::new()
    }
}
