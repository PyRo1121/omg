//! Built-in mise runtime manager
//!
//! Downloads and manages mise as a bundled tool - NO EXTERNAL INSTALL REQUIRED.
//! Mise provides support for 100+ additional runtimes beyond OMG's native managers.
//!
//! ## Third-Party Attribution
//!
//! This module integrates with mise (<https://github.com/jdx/mise>)
//! Copyright (c) 2025 Jeff Dickey, licensed under the MIT License.
//! See THIRD-PARTY-LICENSES.md for the full mise license text.
//!
//! The integration code in this file is part of OMG and licensed under AGPL-3.0.

use anyhow::{Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Command;

use crate::core::http::download_client;

const MISE_GITHUB_RELEASES: &str = "https://github.com/jdx/mise/releases";

/// Mise runtime manager - bundled with OMG
pub struct MiseManager {
    /// Directory where mise binary is stored
    bin_dir: PathBuf,
    /// Path to the mise binary
    mise_bin: PathBuf,
    /// HTTP client for downloads
    client: reqwest::Client,
}

impl MiseManager {
    pub fn new() -> Self {
        let bin_dir = super::DATA_DIR.join("mise");
        Self {
            mise_bin: bin_dir.join("mise"),
            bin_dir,
            client: download_client().clone(),
        }
    }

    /// Check if mise is available (either bundled or system-installed)
    #[must_use]
    pub fn is_available(&self) -> bool {
        // First check bundled mise
        if self.mise_bin.exists() {
            return true;
        }

        static AVAILABLE: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        *AVAILABLE.get_or_init(|| {
            // Fall back to system mise
            Command::new("mise")
                .arg("--version")
                .output()
                .is_ok_and(|out| out.status.success())
        })
    }

    /// Get the path to the mise binary (bundled or system)
    #[must_use]
    pub fn mise_path(&self) -> &std::path::Path {
        if self.mise_bin.exists() {
            &self.mise_bin
        } else {
            std::path::Path::new("mise")
        }
    }

    /// Ensure mise is installed (download if needed)
    pub async fn ensure_installed(&self) -> Result<()> {
        if self.is_available() {
            return Ok(());
        }

        self.install().await
    }

    /// Install mise binary
    pub async fn install(&self) -> Result<()> {
        println!(
            "{} Installing mise (runtime version manager)...\n",
            "OMG".cyan().bold()
        );

        fs::create_dir_all(&self.bin_dir)?;

        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            arch => anyhow::bail!("Unsupported architecture: {arch}"),
        };

        // Get latest version from GitHub API
        let version = self.get_latest_version().await?;
        let filename = format!("mise-v{version}-linux-{arch}.tar.gz");
        let url = format!("{MISE_GITHUB_RELEASES}/download/v{version}/{filename}");

        println!("{} Downloading mise v{}...", "→".blue(), version);
        let download_path = self.bin_dir.join(&filename);
        self.download_file(&url, &download_path).await?;

        // Extract the tarball
        println!("{} Extracting...", "→".blue());
        self.extract_tarball(&download_path)?;

        // Cleanup
        let _ = fs::remove_file(&download_path);

        // Verify installation
        if !self.mise_bin.exists() {
            anyhow::bail!("mise binary not found after extraction");
        }

        // Make executable
        let mut perms = fs::metadata(&self.mise_bin)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&self.mise_bin, perms)?;

        println!("{} mise v{} installed!", "✓".green(), version);
        Ok(())
    }

    /// Get the latest mise version from GitHub
    async fn get_latest_version(&self) -> Result<String> {
        #[derive(serde::Deserialize)]
        struct Release {
            tag_name: String,
        }

        let release: Release = self
            .client
            .get("https://api.github.com/repos/jdx/mise/releases/latest")
            .header("User-Agent", "omg-package-manager")
            .send()
            .await
            .context("Failed to fetch mise releases")?
            .json()
            .await
            .context("Failed to parse mise release info")?;

        Ok(release
            .tag_name
            .strip_prefix('v')
            .unwrap_or(&release.tag_name)
            .to_owned())
    }

    /// Download a file with progress bar
    async fn download_file(&self, url: &str, path: &PathBuf) -> Result<()> {
        let response = self
            .client
            .get(url)
            .header("User-Agent", "omg-package-manager")
            .send()
            .await
            .with_context(|| format!("Failed to download from {url}"))?;

        let status = response.status();
        anyhow::ensure!(status.is_success(), "Download failed with status: {status}");

        let total_size = response.content_length().unwrap_or(0);

        let pb = ProgressBar::new(total_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .unwrap_or_else(|_| ProgressStyle::default_bar())
                .progress_chars("█▓░"),
        );

        let bytes = response.bytes().await?;
        pb.inc(bytes.len() as u64);

        let mut file = File::create(path)?;
        file.write_all(&bytes)?;

        pb.finish_and_clear();
        Ok(())
    }

    /// Extract mise tarball
    fn extract_tarball(&self, tarball_path: &PathBuf) -> Result<()> {
        let file = File::open(tarball_path)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        // First pass: try to find and extract mise directly
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy();

            // Look for the mise binary in the archive
            if path_str.ends_with("/mise") || path_str == "mise" {
                entry.unpack(&self.mise_bin)?;
                return Ok(());
            }
        }

        // Second pass: extract everything with path stripping
        let file = File::open(tarball_path)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        for entry in archive.entries()? {
            let mut entry = entry?;
            if !entry.header().entry_type().is_file() {
                continue;
            }

            let path = entry.path()?.to_path_buf();
            // Strip first component if present
            let stripped: PathBuf = path.components().skip(1).collect();
            let dest = if stripped.as_os_str().is_empty() {
                self.bin_dir.join(&path)
            } else {
                self.bin_dir.join(&stripped)
            };

            if let Some(parent) = dest.parent() {
                fs::create_dir_all(parent)?;
            }
            entry.unpack(&dest)?;
        }

        Ok(())
    }

    /// Get current version of a runtime via mise
    pub fn current_version(&self, runtime: &str) -> Result<Option<String>> {
        // SECURITY: Validate runtime name to prevent argument injection
        crate::core::security::validate_package_name(runtime)?;

        let output = Command::new(self.mise_path())
            .args(["current", "--", runtime])
            .output()
            .with_context(|| format!("Failed to run mise current {runtime}"))?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let Some(line) = stdout.lines().find(|line| !line.trim().is_empty()) else {
            return Ok(None);
        };
        let line = line.trim();

        // Parse "runtime version" or "runtime@version" format
        if let Some(rest) = line.strip_prefix(runtime)
            && let Some(version) = rest.split_whitespace().find(|token| !token.is_empty())
        {
            return Ok(Some(version.to_owned()));
        }

        if let Some((_, version)) = line.split_once('@') {
            return Ok(Some(version.trim().to_owned()));
        }

        Ok(Some(line.to_owned()))
    }

    /// Install a runtime version via mise
    pub fn install_runtime(&self, runtime: &str) -> Result<bool> {
        // SECURITY: Validate runtime name
        crate::core::security::validate_package_name(runtime)?;

        let status = Command::new(self.mise_path())
            .args(["install", "--", runtime])
            .status()
            .with_context(|| format!("Failed to run mise install {runtime}"))?;

        Ok(status.success())
    }

    /// List installed runtimes via mise
    pub fn list_installed(&self) -> Result<Vec<String>> {
        let output = Command::new(self.mise_path())
            .args(["ls", "--"])
            .output()
            .context("Failed to run mise ls")?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut runtimes: Vec<_> = stdout
            .lines()
            .filter_map(|line| {
                let runtime = line.split_whitespace().next()?;
                (!runtime.is_empty()).then(|| runtime.to_owned())
            })
            .collect();

        runtimes.sort_unstable();
        runtimes.dedup();
        Ok(runtimes)
    }

    /// Use a specific version of a runtime
    pub fn use_version(&self, runtime: &str, version: &str) -> Result<()> {
        // SECURITY: Validate runtime and version
        crate::core::security::validate_package_name(runtime)?;
        crate::core::security::validate_version(version)?;

        let tool_spec = format!("{runtime}@{version}");

        // Install if needed
        let install_status = Command::new(self.mise_path())
            .args(["install", "--", &tool_spec])
            .status()
            .with_context(|| format!("Failed to run mise install {tool_spec}"))?;

        if !install_status.success() {
            anyhow::bail!("mise failed to install {tool_spec}");
        }

        // Activate in current directory (creates mise.toml)
        let use_status = Command::new(self.mise_path())
            .args(["use", "--", &tool_spec])
            .status()
            .with_context(|| format!("Failed to run mise use {tool_spec}"))?;

        if !use_status.success() {
            anyhow::bail!("mise failed to activate {tool_spec}");
        }

        println!("{} Using {} {} (via mise)", "✓".green(), runtime, version);
        Ok(())
    }

    /// Get the bin directory for a mise-managed runtime
    #[must_use]
    pub fn runtime_bin_path(&self, runtime: &str, version: &str) -> Option<PathBuf> {
        // mise installs to ~/.local/share/mise/installs/<runtime>/<version>/bin
        let mise_data = dirs::data_dir()?.join("mise").join("installs");
        let bin_path = mise_data.join(runtime).join(version).join("bin");

        if bin_path.exists() {
            Some(bin_path)
        } else {
            None
        }
    }
}

impl Default for MiseManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mise_manager_new() {
        let mgr = MiseManager::new();
        assert!(mgr.bin_dir.ends_with("mise"));
    }
}
