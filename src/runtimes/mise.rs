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

use crate::core::archive::stripped_archive_path;
use crate::core::http::download_client;
use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::fs::{self, File};
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::common::{
    GITHUB_USER_AGENT, GithubRelease, download_with_progress, normalize_version,
    parse_sha256_digest, remove_file_best_effort,
};

const MISE_GITHUB_RELEASES: &str = "https://github.com/jdx/mise/releases";
const MISE_GITHUB_API: &str = "https://api.github.com/repos/jdx/mise";

/// Mise runtime manager - bundled with OMG.
pub(crate) struct MiseManager {
    /// Directory where mise binary is stored
    bin_dir: PathBuf,
    /// Path to the mise binary
    mise_bin: PathBuf,
    /// HTTP client for downloads
    client: &'static reqwest::Client,
}

impl MiseManager {
    pub fn new() -> Self {
        let bin_dir = super::DATA_DIR.join("mise");
        Self {
            mise_bin: bin_dir.join("mise"),
            bin_dir,
            client: download_client(),
        }
    }

    /// Check if mise is available (either bundled or system-installed)
    #[must_use]
    pub fn is_available(&self) -> bool {
        // First check bundled mise
        if super::common::require_regular_file(&self.mise_bin).is_ok() {
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
        if super::common::require_regular_file(&self.mise_bin).is_ok() {
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
        let prefix = "OMG".cyan().bold().to_string();
        tracing::info!("{prefix} Installing mise (runtime version manager)...\n");

        fs::create_dir_all(&self.bin_dir)?;

        let version = self.get_latest_version().await?;
        let filename = format!("mise-v{version}-{}.tar.gz", mise_platform()?);
        let url = format!("{MISE_GITHUB_RELEASES}/download/v{version}/{filename}");
        let checksum = self.fetch_checksum(&version, &filename).await?;

        tracing::info!("{} Downloading mise v{}...", "→".blue(), version);
        let download_path = self.bin_dir.join(&filename);
        download_with_progress(self.client, &url, &download_path, &checksum).await?;

        // Extract the tarball
        tracing::info!("{} Extracting...", "→".blue());
        self.extract_tarball(&download_path)?;

        remove_file_best_effort(&download_path, "mise archive");

        // Verify installation
        super::common::require_regular_file(&self.mise_bin)
            .context("mise binary not found after extraction")?;

        // Make executable (Unix only)
        #[cfg(unix)]
        {
            let mut perms = fs::metadata(&self.mise_bin)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&self.mise_bin, perms)?;
        }

        tracing::info!("{} mise v{} installed!", "✓".green(), version);
        Ok(())
    }

    async fn fetch_checksum(&self, version: &str, filename: &str) -> Result<String> {
        let release: GithubRelease = self
            .client
            .get(format!("{MISE_GITHUB_API}/releases/tags/v{version}"))
            .header("User-Agent", GITHUB_USER_AGENT)
            .send()
            .await
            .context("Failed to fetch mise release metadata")?
            .error_for_status()
            .context("mise release metadata request failed")?
            .json()
            .await
            .context("Failed to parse mise release metadata")?;
        let asset = release
            .assets
            .iter()
            .find(|asset| asset.name == filename)
            .ok_or_else(|| anyhow::anyhow!("mise release asset not found: {filename}"))?;
        let digest = asset
            .digest
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("mise release asset has no SHA-256 digest"))?;
        parse_sha256_digest(digest, "GitHub mise release")
    }

    /// Get the latest mise version from GitHub
    async fn get_latest_version(&self) -> Result<String> {
        let release: GithubRelease = self
            .client
            .get(format!("{MISE_GITHUB_API}/releases/latest"))
            .header("User-Agent", GITHUB_USER_AGENT)
            .send()
            .await
            .context("Failed to fetch mise releases")?
            .json()
            .await
            .context("Failed to parse mise release info")?;

        Ok(normalize_version(&release.tag_name))
    }

    /// Extract mise tarball
    fn extract_tarball(&self, tarball_path: &Path) -> Result<()> {
        let file = File::open(tarball_path)?;
        let decoder = flate2::read::GzDecoder::new(file);
        let mut archive = tar::Archive::new(decoder);

        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            stripped_archive_path(&path, 0)?;
            let path_str = path.to_string_lossy();

            // Look for the mise binary in the archive.
            if path_str.ends_with("/mise") || path_str == "mise" {
                if !entry.header().entry_type().is_file() {
                    anyhow::bail!("Mise archive binary entry is not a regular file");
                }
                self.persist_mise_binary(&mut entry)?;
                return Ok(());
            }
        }

        anyhow::bail!("Mise archive does not contain a regular mise binary")
    }

    /// Unpack the mise binary into a same-directory temp file and atomically
    /// publish it. A crash mid-extract therefore never leaves a live binary
    /// that [`is_available`] would treat as installed.
    fn persist_mise_binary<R: io::Read>(&self, entry: &mut tar::Entry<'_, R>) -> Result<()> {
        fs::create_dir_all(&self.bin_dir).with_context(|| {
            format!(
                "Failed to create mise directory: {}",
                self.bin_dir.display()
            )
        })?;
        let mut temporary = tempfile::NamedTempFile::new_in(&self.bin_dir).with_context(|| {
            format!(
                "Failed to create temporary mise binary in {}",
                self.bin_dir.display()
            )
        })?;
        io::copy(entry, &mut temporary).with_context(|| {
            format!(
                "Failed to write temporary mise binary in {}",
                self.bin_dir.display()
            )
        })?;
        temporary.flush()?;
        temporary.as_file_mut().sync_all()?;
        #[cfg(unix)]
        {
            let mut perms = temporary.as_file().metadata()?.permissions();
            perms.set_mode(0o755);
            temporary.as_file().set_permissions(perms)?;
        }
        temporary
            .persist(&self.mise_bin)
            .map_err(|error| error.error)
            .with_context(|| {
                format!(
                    "Failed to persist mise binary at {}",
                    self.mise_bin.display()
                )
            })?;
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
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.to_ascii_lowercase().contains("not found")
                || stderr.to_ascii_lowercase().contains("no version")
            {
                return Ok(None);
            }
            anyhow::bail!("mise current {runtime} failed: {}", stderr.trim());
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

        // Capture output so a failed install can surface mise's stderr
        // instead of being indistinguishable from "runtime not available".
        let output = Command::new(self.mise_path())
            .args(["install", "--", runtime])
            .output()
            .with_context(|| format!("Failed to run mise install {runtime}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr = stderr.trim();
            anyhow::bail!(
                "mise failed to install {runtime} ({}): {}",
                output.status,
                if stderr.is_empty() {
                    "no stderr output"
                } else {
                    stderr
                }
            );
        }

        Ok(true)
    }

    /// List installed runtimes via mise
    pub fn list_installed(&self) -> Result<Vec<String>> {
        let output = Command::new(self.mise_path())
            .args(["ls", "--"])
            .output()
            .context("Failed to run mise ls")?;

        if !output.status.success() {
            anyhow::bail!(
                "mise ls failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
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
        crate::core::security::validate_runtime_version(version)?;

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

        tracing::info!("{} Using {} {} (via mise)", "✓".green(), runtime, version);
        Ok(())
    }
}

fn mise_platform() -> Result<String> {
    let os = super::common::host_os_tag("mise", "linux", "macos")?;
    let arch = super::common::host_arch_tag("mise", "x64", "arm64")?;
    Ok(format!("{os}-{arch}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::GzEncoder};
    use std::io::Cursor;
    use tar::{Builder, EntryType, Header};
    use tempfile::TempDir;

    fn mise_archive(entry_type: EntryType, link_name: Option<&str>) -> Result<Vec<u8>> {
        let encoder = GzEncoder::new(Vec::new(), Compression::default());
        let mut builder = Builder::new(encoder);
        let mut header = Header::new_gnu();
        header.set_entry_type(entry_type);
        header.set_mode(0o755);
        let contents = if entry_type.is_file() {
            b"mise".as_slice()
        } else {
            b"".as_slice()
        };
        header.set_size(contents.len() as u64);
        if let Some(target) = link_name {
            header.set_link_name(target)?;
        }
        header.set_cksum();
        builder.append_data(&mut header, "release/mise", Cursor::new(contents))?;
        let encoder = builder.into_inner()?;
        Ok(encoder.finish()?)
    }

    fn test_manager(temp: &TempDir) -> Result<MiseManager> {
        let bin_dir = temp.path().join("bin");
        fs::create_dir(&bin_dir)?;
        Ok(MiseManager {
            mise_bin: bin_dir.join("mise"),
            bin_dir,
            client: download_client(),
        })
    }

    #[test]
    fn test_mise_manager_new() {
        let mgr = MiseManager::new();
        assert!(mgr.bin_dir.ends_with("mise"));
    }

    #[test]
    fn mise_extraction_writes_a_regular_binary() -> Result<()> {
        let temp = TempDir::new()?;
        let manager = test_manager(&temp)?;
        let archive_path = temp.path().join("mise.tar.gz");
        fs::write(&archive_path, mise_archive(EntryType::Regular, None)?)?;

        manager.extract_tarball(&archive_path)?;

        assert_eq!(fs::read(&manager.mise_bin)?, b"mise");
        Ok(())
    }

    #[test]
    fn mise_extraction_rejects_a_linked_binary() -> Result<()> {
        let temp = TempDir::new()?;
        let manager = test_manager(&temp)?;
        let archive_path = temp.path().join("mise.tar.gz");
        fs::write(
            &archive_path,
            mise_archive(EntryType::Symlink, Some("../../escape"))?,
        )?;

        assert!(manager.extract_tarball(&archive_path).is_err());
        assert!(!manager.mise_bin.exists());
        Ok(())
    }

    #[test]
    fn mise_platform_is_host_specific() {
        let platform = mise_platform().expect("host platform should be supported");
        if std::env::consts::OS == "linux" {
            assert!(platform.starts_with("linux-"));
        } else {
            assert!(!platform.starts_with("linux-"));
        }
    }
}
