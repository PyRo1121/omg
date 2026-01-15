//! Native Rust toolchain manager - PURE RUST, NO RUSTUP
//!
//! Downloads Rust toolchains directly from static.rust-lang.org

use anyhow::{Context, Result};
use colored::Colorize;
use flate2::read::GzDecoder;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use tar::Archive;

use crate::core::http::download_client;
const RUST_DIST_URL: &str = "https://static.rust-lang.org/dist";
const RUST_MANIFEST_URL: &str = "https://static.rust-lang.org/dist/channel-rust-stable.toml";

/// Rust version info
#[derive(Debug, Clone)]
pub struct RustVersion {
    pub version: String,
    pub channel: String,
}

pub struct RustManager {
    versions_dir: PathBuf,
    current_link: PathBuf,
    client: reqwest::Client,
}

impl RustManager {
    pub fn new() -> Self {
        let data_dir = super::DATA_DIR.clone();

        Self {
            versions_dir: data_dir.join("versions").join("rust"),
            current_link: data_dir.join("versions").join("rust").join("current"),
            client: download_client().clone(),
        }
    }

    #[must_use]
    pub fn bin_dir(&self) -> PathBuf {
        self.current_link.join("bin")
    }

    /// List available Rust versions (stable, beta, nightly + recent releases)
    pub async fn list_available(&self) -> Result<Vec<RustVersion>> {
        let mut versions = Vec::new();

        // Get stable version from manifest
        let manifest = self
            .client
            .get(RUST_MANIFEST_URL)
            .send()
            .await
            .context("Failed to fetch Rust version manifest. Check your internet connection.")?
            .text()
            .await
            .context("Failed to read Rust version manifest")?;

        // Parse version from TOML manifest
        for line in manifest.lines() {
            if line.starts_with("version = ") {
                let version = line
                    .trim_start_matches("version = ")
                    .trim_matches('"')
                    .split(' ')
                    .next()
                    .unwrap_or("");
                if !version.is_empty() {
                    versions.push(RustVersion {
                        version: version.to_string(),
                        channel: "stable".to_string(),
                    });
                }
                break;
            }
        }

        // Add channel aliases
        versions.push(RustVersion {
            version: "stable".to_string(),
            channel: "stable".to_string(),
        });
        versions.push(RustVersion {
            version: "beta".to_string(),
            channel: "beta".to_string(),
        });
        versions.push(RustVersion {
            version: "nightly".to_string(),
            channel: "nightly".to_string(),
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
        versions.sort();
        versions.reverse();
        Ok(versions)
    }

    #[must_use]
    pub fn current_version(&self) -> Option<String> {
        fs::read_link(&self.current_link)
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
    }

    /// Install Rust - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version_dir = self.versions_dir.join(version);

        if version_dir.exists() {
            println!("{} Rust {} is already installed", "✓".green(), version);
            return self.use_version(version);
        }

        println!(
            "{} Installing Rust {}...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        let target = match std::env::consts::ARCH {
            "x86_64" => "x86_64-unknown-linux-gnu",
            "aarch64" => "aarch64-unknown-linux-gnu",
            arch => anyhow::bail!("Unsupported architecture: {arch}"),
        };

        fs::create_dir_all(&self.versions_dir)?;
        fs::create_dir_all(&version_dir)?;

        // Download and install rust-std, rustc, and cargo
        let components = ["rust-std", "rustc", "cargo"];

        for component in components {
            let filename = format!("{component}-{version}-{target}.tar.gz");
            let url = format!("{RUST_DIST_URL}/{filename}");

            println!("{} Downloading {}...", "→".blue(), component);
            let download_path = self.versions_dir.join(&filename);

            match self.download_file(&url, &download_path).await {
                Ok(()) => {
                    println!("{} Extracting {}...", "→".blue(), component);
                    Self::extract_component(
                        &download_path,
                        &version_dir,
                        component,
                        version,
                        target,
                    )?;
                    let _ = fs::remove_file(&download_path);
                }
                Err(e) => {
                    println!("{} Failed to download {}: {}", "!".yellow(), component, e);
                    // Continue with other components
                }
            }
        }

        println!("{} Rust {} installed!", "✓".green(), version);
        self.use_version(version)?;

        Ok(())
    }

    fn extract_component(
        archive_path: &Path,
        dest_dir: &Path,
        component: &str,
        version: &str,
        target: &str,
    ) -> Result<()> {
        let file = File::open(archive_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);

        let _prefix = format!("{component}-{version}-{target}");

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy();

            // Skip manifest and installer files, only extract from the component subdirectory
            if !path_str.contains("/lib/")
                && !path_str.contains("/bin/")
                && !path_str.contains("/share/")
            {
                continue;
            }

            // Strip prefix and component name
            let stripped: PathBuf = path
                .components()
                .skip(2) // Skip "component-version-target/component/"
                .collect();

            if stripped.as_os_str().is_empty() {
                continue;
            }

            let dest_path = dest_dir.join(&stripped);

            if let Some(parent) = dest_path.parent() {
                fs::create_dir_all(parent)?;
            }

            if entry.header().entry_type().is_dir() {
                fs::create_dir_all(&dest_path)?;
            } else {
                entry.unpack(&dest_path)?;
            }
        }

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
        let version_dir = self.versions_dir.join(version);

        if !version_dir.exists() {
            anyhow::bail!("Rust {version} is not installed");
        }

        if self.current_link.exists() {
            fs::remove_file(&self.current_link)?;
        }

        #[cfg(unix)]
        std::os::unix::fs::symlink(&version_dir, &self.current_link)?;

        println!("{} Now using Rust {}", "✓".green(), version);
        println!(
            "  Add to PATH: {}",
            self.bin_dir().display().to_string().dimmed()
        );

        Ok(())
    }

    pub fn uninstall(&self, version: &str) -> Result<()> {
        let version_dir = self.versions_dir.join(version);

        if !version_dir.exists() {
            println!("{} Rust {} is not installed", "→".dimmed(), version);
            return Ok(());
        }

        if let Some(current) = self.current_version() {
            if current == version {
                let _ = fs::remove_file(&self.current_link);
            }
        }

        fs::remove_dir_all(&version_dir)?;
        println!("{} Rust {} uninstalled", "✓".green(), version);
        Ok(())
    }
}

impl Default for RustManager {
    fn default() -> Self {
        Self::new()
    }
}
