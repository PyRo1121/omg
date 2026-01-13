//! Native Ruby runtime manager - PURE RUST
//!
//! Downloads pre-built Ruby binaries from ruby-lang.org releases.

use anyhow::{Context, Result};
use colored::Colorize;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::fs::{self, File};
use std::path::PathBuf;
use tar::Archive;

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
        let data_dir = directories::ProjectDirs::from("com", "omg", "omg")
            .map(|d| d.data_dir().to_path_buf())
            .unwrap_or_else(|| {
                home::home_dir()
                    .unwrap_or_else(|| PathBuf::from("."))
                    .join(".omg")
            });

        let client = reqwest::Client::builder()
            .user_agent("omg-package-manager")
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        RubyManager {
            versions_dir: data_dir.join("versions").join("ruby"),
            current_link: data_dir.join("versions").join("ruby").join("current"),
            client,
        }
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.current_link.join("bin")
    }

    /// List available Ruby versions from ruby-builder releases
    pub async fn list_available(&self) -> Result<Vec<RubyVersion>> {
        let releases: Vec<GithubRelease> = self
            .client
            .get(RUBY_VERSIONS_URL)
            .query(&[("per_page", "20")])
            .send()
            .await
            .context("Failed to fetch Ruby releases from GitHub")?
            .json()
            .await
            .context("Failed to parse Ruby release data")?;

        // Extract unique Ruby versions from release tags
        // Tags are like "toolcache" or version-specific
        let mut versions = std::collections::HashSet::new();
        let re = regex::Regex::new(r"^(\d+\.\d+\.\d+)$")?;

        for release in &releases {
            if let Some(caps) = re.captures(&release.tag_name) {
                if let Some(version) = caps.get(1) {
                    versions.insert(version.as_str().to_string());
                }
            }
        }

        // If no version tags found, return common stable versions
        if versions.is_empty() {
            versions.insert("3.3.0".to_string());
            versions.insert("3.2.2".to_string());
            versions.insert("3.1.4".to_string());
            versions.insert("3.0.6".to_string());
        }

        let mut result: Vec<RubyVersion> = versions
            .into_iter()
            .map(|v| RubyVersion {
                version: v,
                prebuilt: true,
            })
            .collect();

        result.sort_by(|a, b| version_cmp(&b.version, &a.version));
        Ok(result)
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

    pub fn current_version(&self) -> Option<String> {
        fs::read_link(&self.current_link)
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
    }

    /// Install Ruby - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = version.trim_start_matches('v');
        let version_dir = self.versions_dir.join(version);

        if version_dir.exists() {
            println!("{} Ruby {} is already installed", "✓".green(), version);
            return self.use_version(version);
        }

        println!(
            "{} Installing Ruby {}...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        // Use pre-built Ruby from GitHub ruby-builder
        let os = "ubuntu-22.04"; // Most compatible glibc version
        let filename = format!("ruby-{}.tar.gz", version);
        let url = format!("{}/toolcache/{}-{}", RUBY_PREBUILT_URL, os, filename);

        fs::create_dir_all(&self.versions_dir)?;

        println!("{} Downloading pre-built Ruby {}...", "→".blue(), version);
        let download_path = self.versions_dir.join(&filename);

        match self.download_file(&url, &download_path).await {
            Ok(()) => {
                // PURE RUST EXTRACTION
                println!("{} Extracting (pure Rust)...", "→".blue());

                let file = File::open(&download_path)?;
                let decoder = GzDecoder::new(file);
                let mut archive = Archive::new(decoder);

                for entry in archive.entries()? {
                    let mut entry = entry?;
                    let path = entry.path()?;

                    // Strip first component (ruby-X.X.X/)
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

                println!("{} Ruby {} installed!", "✓".green(), version);
                self.use_version(version)?;
            }
            Err(e) => {
                println!(
                    "{} Pre-built Ruby {} not available: {}",
                    "!".yellow(),
                    version,
                    e
                );
                println!("  Try a different version (3.3.0, 3.2.2, 3.1.4, etc.)");
                return Err(e);
            }
        }

        Ok(())
    }

    async fn download_file(&self, url: &str, path: &PathBuf) -> Result<()> {
        let response =
            self.client.get(url).send().await.with_context(|| {
                format!("Failed to download from {}. Check your connection.", url)
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
            anyhow::bail!("Ruby {} is not installed", version);
        }

        if self.current_link.exists() {
            fs::remove_file(&self.current_link)?;
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(&version_dir, &self.current_link)?;

        println!("{} Now using Ruby {}", "✓".green(), version);
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
            println!("{} Ruby {} is not installed", "→".dimmed(), version);
            return Ok(());
        }

        if let Some(current) = self.current_version() {
            if current == version {
                let _ = fs::remove_file(&self.current_link);
            }
        }

        fs::remove_dir_all(&version_dir)?;
        println!("{} Ruby {} uninstalled", "✓".green(), version);
        Ok(())
    }
}

impl Default for RubyManager {
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
        match a_part.cmp(b_part) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}
