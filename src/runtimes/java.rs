//! Native Java runtime manager - PURE RUST
//!
//! Downloads JDK from Eclipse Adoptium (Temurin).
//!
//! Features:
//! - Official Eclipse Adoptium builds
//! - LTS version detection
//! - `JAVA_HOME` auto-configuration

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::Deserialize;

use super::common::{
    activate_version, begin_staged_install, complete_staged_install, download_with_progress,
    extract_tar_gz, parse_sha256_digest, print_already_installed, print_installed,
    remove_file_best_effort,
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
    checksum: String,
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
        let versions_dir = super::DATA_DIR.join("versions/java");

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

    /// List available Java versions from Adoptium
    pub async fn list_available(&self) -> Result<Vec<JavaVersion>> {
        #[derive(Deserialize)]
        struct AvailableReleases {
            available_lts_releases: Vec<u32>,
            available_releases: Vec<u32>,
        }

        let releases: AvailableReleases = self
            .client
            .get(format!("{ADOPTIUM_API}/info/available_releases"))
            .send()
            .await
            .context("Failed to fetch Java versions from Adoptium")?
            .json()
            .await
            .context("Failed to parse Java version data")?;

        let lts_set: HashSet<u32> = releases.available_lts_releases.into_iter().collect();

        let mut versions: Vec<JavaVersion> = releases
            .available_releases
            .into_iter()
            .map(|v| JavaVersion {
                version: v.to_string(),
                lts: lts_set.contains(&v),
            })
            .collect();

        versions.sort_by_key(|v| std::cmp::Reverse(v.version.parse::<u32>().unwrap_or(0)));

        Ok(versions)
    }

    /// Install Java - PURE RUST, NO SUBPROCESS
    pub async fn install(&self, version: &str) -> Result<()> {
        crate::core::security::validate_runtime_version(version)?;
        let version_dir = self.versions_dir.join(version);

        if crate::runtimes::common::is_valid_version_dir(&version_dir) {
            print_already_installed("Java", version);
            return self.use_version(version);
        }

        println!(
            "{} Installing Java {} (Adoptium)...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        let (os, arch) = java_platform()?;

        println!("{} Querying Adoptium API...", "→".blue());

        let binaries: Vec<AdoptiumBinary> = self
            .client
            .get(format!(
                "{ADOPTIUM_API}/assets/latest/{version}/hotspot?\
                 architecture={arch}&image_type=jdk&os={os}&vendor=eclipse"
            ))
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
        let checksum = parse_sha256_digest(&binary.package.checksum, "Adoptium")?;
        download_with_progress(
            &self.client,
            &binary.package.link,
            &download_path,
            Some(&checksum),
        )
        .await?;

        println!("{} Extracting (pure Rust)...", "→".blue());
        let staging = begin_staged_install(&self.versions_dir)?;
        extract_tar_gz(&download_path, staging.path(), 1).await?;
        complete_staged_install(&staging, &version_dir, version)?;

        remove_file_best_effort(&download_path, "runtime archive");

        print_installed("Java", version);
        self.use_version(version)
    }

    /// Switch to a specific version
    pub fn use_version(&self, version: &str) -> Result<()> {
        let version_dir = self.versions_dir.join(version);
        activate_version(&self.versions_dir, version, Path::new("bin/java"))?;

        println!("{} Now using Java {version}", "✓".green());
        println!(
            "  {} {}",
            "JAVA_HOME:".dimmed(),
            version_dir.display().dimmed()
        );
        println!(
            "  {} {}",
            "PATH:".dimmed(),
            self.bin_dir().display().dimmed()
        );

        Ok(())
    }
}

// Generate common runtime manager methods (list_installed, current_version, uninstall)
crate::impl_runtime_common!(JavaManager, "Java");

fn java_platform() -> Result<(&'static str, &'static str)> {
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "macos" => "mac",
        other => anyhow::bail!("Unsupported operating system for Java: {other}"),
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        arch => anyhow::bail!("Unsupported architecture for Java: {arch}"),
    };
    Ok((os, arch))
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

    #[test]
    fn java_platform_is_host_specific() {
        let (os, _arch) = java_platform().expect("host platform should be supported");
        if std::env::consts::OS == "linux" {
            assert_eq!(os, "linux");
        } else {
            assert_ne!(os, "linux");
        }
    }
}
