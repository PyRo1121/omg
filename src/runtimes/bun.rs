//! Native Bun runtime manager - PURE RUST
//!
//! Downloads and manages Bun versions from GitHub.

use anyhow::{Context, Result};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Deserialize;
use std::fs::{self, File};
use std::path::PathBuf;

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

    /// Install Bun - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version = version.trim_start_matches('v');
        let version_dir = self.versions_dir.join(version);

        if version_dir.exists() {
            println!("{} Bun {} is already installed", "✓".green(), version);
            return self.use_version(version);
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
        self.download_file(&url, &download_path).await?;

        // PURE RUST ZIP EXTRACTION
        println!("{} Extracting (pure Rust)...", "→".blue());

        let file = File::open(&download_path)?;
        let mut archive = zip::ZipArchive::new(file)?;

        fs::create_dir_all(&version_dir)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => {
                    // Strip first component (bun-linux-x64/)
                    let stripped: PathBuf = path.components().skip(1).collect();
                    if stripped.as_os_str().is_empty() {
                        continue;
                    }
                    version_dir.join(stripped)
                }
                None => continue,
            };

            if file.is_dir() {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }

            // Set permissions
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
                }
            }
        }

        let _ = fs::remove_file(&download_path);

        println!("{} Bun {} installed!", "✓".green(), version);
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
            anyhow::bail!("Bun {version} is not installed");
        }

        if self.current_link.exists() {
            fs::remove_file(&self.current_link)?;
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(&version_dir, &self.current_link)?;

        println!("{} Now using Bun {}", "✓".green(), version);
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
            println!("{} Bun {} is not installed", "→".dimmed(), version);
            return Ok(());
        }

        if let Some(current) = self.current_version() {
            if current == version {
                let _ = fs::remove_file(&self.current_link);
            }
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
