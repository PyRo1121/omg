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
use std::fs;
use std::path::{Path, PathBuf};

use super::common::{
    GITHUB_USER_AGENT, GithubRelease, activate_version_with_linked_binary, begin_staged_install,
    complete_staged_install, download_with_progress, extract_tar_gz, normalize_version,
    parse_sha256_digest, print_already_installed, print_installed, print_using,
    remove_file_best_effort, validate_download_filename, version_cmp,
};
use crate::core::http::download_client;

const PBS_RELEASES_URL: &str =
    "https://api.github.com/repos/indygreg/python-build-standalone/releases";

/// Python version info for available versions
#[derive(Debug, Clone)]
pub(crate) struct PythonVersion {
    pub version: String,
}

pub(crate) struct PythonManager {
    versions_dir: PathBuf,
    client: &'static reqwest::Client,
}

impl PythonManager {
    pub fn new() -> Self {
        Self {
            versions_dir: super::DATA_DIR.join("versions/python"),
            client: download_client(),
        }
    }

    /// List available Python versions from python-build-standalone
    pub async fn list_available(&self) -> Result<Vec<PythonVersion>> {
        if crate::core::paths::test_mode() {
            return Ok(vec![
                PythonVersion {
                    version: "3.12.0".to_string(),
                },
                PythonVersion {
                    version: "3.11.0".to_string(),
                },
            ]);
        }
        let releases: Vec<GithubRelease> = self
            .client
            .get(format!("{PBS_RELEASES_URL}?per_page=10"))
            .header("User-Agent", GITHUB_USER_AGENT)
            .send()
            .await
            .context("Failed to fetch Python releases from GitHub")?
            .json()
            .await
            .context("Failed to parse Python release data")?;

        let target = python_target()?;

        let mut versions = std::collections::HashSet::new();

        for release in &releases {
            for asset in &release.assets {
                if asset.name.contains(&target)
                    && asset.name.contains("install_only")
                    && asset.name.ends_with(".tar.gz")
                    && let Some(version) = Self::extract_cpython_version(&asset.name)
                {
                    versions.insert(version);
                }
            }
        }

        let mut result: Vec<PythonVersion> = versions
            .into_iter()
            .map(|version| PythonVersion { version })
            .collect();

        result.sort_unstable_by(|a, b| version_cmp(&b.version, &a.version));
        Ok(result)
    }

    fn extract_cpython_version(name: &str) -> Option<String> {
        let (_, tail) = name.split_once("cpython-")?;
        let version = tail
            .split(|character: char| !character.is_ascii_digit() && character != '.')
            .next()?;
        Self::is_semver_like(version).then(|| version.to_owned())
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
            fs::write(version_dir.join("bin/python3.12"), "mock")?;
            #[cfg(unix)]
            std::os::unix::fs::symlink("python3.12", version_dir.join("bin/python3"))?;
            #[cfg(not(unix))]
            fs::copy(
                version_dir.join("bin/python3.12"),
                version_dir.join("bin/python3"),
            )?;
            fs::write(
                version_dir.join(super::common::TEST_RUNTIME_MARKER),
                "debug-only synthetic runtime\n",
            )?;
            println!(
                "{} OMG_TEST_MODE active — synthetic Python runtime was not activated",
                "⚠".yellow()
            );
            print_installed("Python", &version);
            return Ok(());
        }

        println!(
            "{} Installing Python {}...\n",
            "OMG".cyan().bold(),
            version.yellow()
        );

        let target = python_target()?;

        println!("{} Finding Python {} release...", "→".blue(), version);

        let releases: Vec<GithubRelease> = self
            .client
            .get(PBS_RELEASES_URL)
            .header("User-Agent", GITHUB_USER_AGENT)
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
                    && asset.name.contains(&target)
                    && asset.name.contains("install_only")
                    && asset.name.ends_with(".tar.gz")
            })
            .ok_or_else(|| {
                anyhow::anyhow!("Python {version} not found. Try: omg list python --available")
            })?;

        let url = asset
            .browser_download_url
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Python release asset has no browser download URL"))?;
        let asset_name = validate_download_filename(&asset.name)?;
        let checksum = asset
            .digest
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("Python release asset has no SHA-256 digest"))
            .and_then(|digest| parse_sha256_digest(digest, "GitHub Python release"))?;

        fs::create_dir_all(&self.versions_dir)?;

        println!("{} Downloading {}...", "→".blue(), asset_name);
        let download_path = self.versions_dir.join(asset_name);
        download_with_progress(self.client, url, &download_path, &checksum).await?;

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
        activate_version_with_linked_binary(
            &self.versions_dir,
            &version,
            Path::new("bin/python3"),
        )?;
        print_using("Python", &version, &self.versions_dir.join("current/bin"));
        Ok(())
    }
}

// Generate common runtime manager methods (list_installed, current_version)
crate::runtimes::common::impl_runtime_common!(PythonManager);

fn python_target() -> Result<String> {
    let arch = match std::env::consts::ARCH {
        "x86_64" | "aarch64" => std::env::consts::ARCH,
        arch => anyhow::bail!("Unsupported architecture for Python: {arch}"),
    };
    match std::env::consts::OS {
        "linux" => Ok(format!("{arch}-unknown-linux-gnu")),
        "macos" => Ok(format!("{arch}-apple-darwin")),
        other => anyhow::bail!("Unsupported operating system for Python: {other}"),
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

    #[cfg(unix)]
    #[test]
    fn python_manager_activates_vendor_symlink_layout() {
        let temp = tempfile::tempdir().expect("temp dir");
        let version_dir = temp.path().join("3.12.0");
        fs::create_dir_all(version_dir.join("bin")).expect("bin dir");
        fs::write(version_dir.join("bin/python3.12"), b"python").expect("python binary");
        std::os::unix::fs::symlink("python3.12", version_dir.join("bin/python3"))
            .expect("vendor launcher link");
        let manager = PythonManager {
            versions_dir: temp.path().to_path_buf(),
            client: download_client(),
        };

        manager
            .use_version("3.12.0")
            .expect("vendor layout must activate");

        assert_eq!(
            fs::read_link(temp.path().join("current")).expect("current link"),
            version_dir
        );
    }

    #[test]
    fn python_target_uses_host_os_and_arch() {
        let target = python_target().expect("host platform should be supported");
        if std::env::consts::OS == "linux" {
            assert!(target.contains("linux-gnu"));
        } else {
            assert!(!target.contains("linux-gnu"));
        }
    }
}
