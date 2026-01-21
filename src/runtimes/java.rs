//! Native Java runtime manager - PURE RUST
//!
//! Downloads JDK from Eclipse Adoptium (Temurin).
//!
//! Features:
//! - Official Eclipse Adoptium builds
//! - LTS version detection
//! - `JAVA_HOME` auto-configuration

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Deserialize;
use std::fs;
use std::path::PathBuf;

use super::common::{
    download_with_progress, extract_tar_gz, get_current_version, list_installed_versions,
    print_already_installed, print_installed, set_current_version,
};
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
        list_installed_versions(&self.versions_dir)
    }

    #[must_use]
    pub fn current_version(&self) -> Option<String> {
        get_current_version(&self.versions_dir)
    }

    /// Install Java - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        let version_dir = self.versions_dir.join(version);

        if version_dir.exists() {
            print_already_installed("Java", version);
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

        let binary = binaries.first().ok_or_else(|| {
            anyhow::anyhow!("No JDK {version} found for {arch}. Try: omg list java --available")
        })?;

        fs::create_dir_all(&self.versions_dir)?;

        println!("{} Downloading {}...", "→".blue(), binary.package.name);
        let download_path = self.versions_dir.join(&binary.package.name);
        download_with_progress(&self.client, &binary.package.link, &download_path, None).await?;

        println!("{} Extracting (pure Rust)...", "→".blue());
        extract_tar_gz(&download_path, &version_dir, 1)?;

        let _ = fs::remove_file(&download_path);

        print_installed("Java", version);
        self.use_version(version)?;

        Ok(())
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version_dir = self.versions_dir.join(version);
        set_current_version(&self.versions_dir, version)?;

        println!("{} Now using Java {}", "✓".green(), version);
        println!(
            "  {} {}",
            "JAVA_HOME:".dimmed(),
            version_dir.display().to_string().dimmed()
        );
        println!(
            "  {} {}",
            "PATH:".dimmed(),
            self.bin_dir().display().to_string().dimmed()
        );

        Ok(())
    }

    /// Uninstall a version
    pub fn uninstall(&self, version: &str) -> Result<()> {
        let version_dir = self.versions_dir.join(version);

        if !version_dir.exists() {
            println!("{} Java {} is not installed", "→".dimmed(), version);
            return Ok(());
        }

        if let Some(current) = self.current_version()
            && current == version {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_java_manager_new() {
        let mgr = JavaManager::new();
        assert!(mgr.versions_dir.ends_with("java"));
    }
}
