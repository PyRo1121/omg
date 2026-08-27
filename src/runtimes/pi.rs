//! Native Pi coding-agent version management.
//!
//! Each Pi release is installed into OMG's version tree with npm's documented
//! `--ignore-scripts` mode, then activated through the same atomic `current`
//! link used by other native runtimes. Failed npm installs remain confined to
//! an unpublished staging directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde::Deserialize;

use super::common::{
    begin_staged_install, complete_staged_install, print_already_installed, print_installed,
    print_using, set_current_version,
};

const PI_PACKAGE: &str = "@earendil-works/pi-coding-agent";

pub(crate) struct PiManager {
    versions_dir: PathBuf,
    npm: PathBuf,
}

impl PiManager {
    pub fn new() -> Self {
        Self {
            versions_dir: super::DATA_DIR.join("versions/pi"),
            npm: PathBuf::from("npm"),
        }
    }

    #[cfg(test)]
    fn with_paths(versions_dir: PathBuf, npm: PathBuf) -> Self {
        Self { versions_dir, npm }
    }

    /// Install and activate one exact Pi release.
    pub async fn install(&self, version: &str) -> Result<()> {
        crate::core::security::validate_runtime_version(version)?;
        let version_dir = self.versions_dir.join(version);
        if super::common::is_valid_version_dir(&version_dir) {
            validate_pi_install(&version_dir, version)?;
            print_already_installed("Pi", version);
            return self.use_version(version);
        }

        let staging = begin_staged_install(&self.versions_dir)?;
        let npm = self.npm.clone();
        let prefix = staging.path().to_path_buf();
        let package_spec = format!("{PI_PACKAGE}@{version}");
        let output = tokio::task::spawn_blocking(move || {
            Command::new(&npm)
                .args([
                    "install",
                    "--global",
                    "--ignore-scripts",
                    "--audit=false",
                    "--fund=false",
                    "--prefix",
                ])
                .arg(&prefix)
                .arg(&package_spec)
                .env_remove("NODE_OPTIONS")
                .env_remove("NPM_CONFIG_PREFIX")
                .env_remove("npm_config_prefix")
                .output()
        })
        .await
        .context("npm task failed while installing Pi")?
        .with_context(|| format!("Failed to execute npm at {}", self.npm.display()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "npm failed to install Pi {version} ({}): {}",
                output.status,
                if stderr.trim().is_empty() {
                    "no stderr output"
                } else {
                    stderr.trim()
                }
            );
        }

        validate_pi_install(staging.path(), version)?;
        complete_staged_install(&staging, &version_dir, version)?;
        print_installed("Pi", version);
        self.use_version(version)
    }

    /// Atomically activate an installed Pi release.
    pub fn use_version(&self, version: &str) -> Result<()> {
        crate::core::security::validate_runtime_version(version)?;
        let version_dir = self.versions_dir.join(version);
        validate_pi_install(&version_dir, version)?;
        set_current_version(&self.versions_dir, version)?;
        print_using("Pi", version, &self.versions_dir.join("current/bin"));
        Ok(())
    }
}

crate::runtimes::common::impl_runtime_common!(PiManager);

#[derive(Deserialize)]
struct PiPackageManifest {
    version: String,
}

fn validate_pi_install(prefix: &Path, expected_version: &str) -> Result<()> {
    let manifest_path = prefix
        .join("lib/node_modules")
        .join(PI_PACKAGE)
        .join("package.json");
    super::common::require_regular_file(&manifest_path)?;
    let metadata = fs::metadata(&manifest_path)
        .with_context(|| format!("Failed to inspect {}", manifest_path.display()))?;
    if metadata.len() > 1024 * 1024 {
        anyhow::bail!("Pi package manifest exceeds 1 MiB");
    }
    let manifest: PiPackageManifest = serde_json::from_slice(
        &fs::read(&manifest_path)
            .with_context(|| format!("Failed to read {}", manifest_path.display()))?,
    )
    .with_context(|| format!("Failed to parse {}", manifest_path.display()))?;
    if manifest.version != expected_version {
        anyhow::bail!(
            "npm installed Pi version {}, expected {expected_version}",
            manifest.version
        );
    }

    let launcher = prefix.join("bin/pi");
    let launcher_target = launcher
        .canonicalize()
        .with_context(|| format!("Failed to resolve Pi launcher {}", launcher.display()))?;
    let canonical_prefix = prefix
        .canonicalize()
        .with_context(|| format!("Failed to resolve Pi prefix {}", prefix.display()))?;
    if !launcher_target.starts_with(&canonical_prefix) || !launcher_target.is_file() {
        anyhow::bail!(
            "Pi launcher escapes its installation prefix: {}",
            launcher.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[cfg(unix)]
    fn fake_npm(directory: &Path, installed_version: &str, succeed: bool) -> Result<PathBuf> {
        let path = directory.join("npm");
        let status = if succeed { 0 } else { 17 };
        let arguments_path = directory.join("npm-args");
        let script = format!(
            "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$@\" > '{}'\nprefix=''\nprevious=''\nfor arg in \"$@\"; do\n  if [ \"$previous\" = '--prefix' ]; then prefix=$arg; fi\n  previous=$arg\ndone\n[ -n \"$prefix\" ]\nmkdir -p \"$prefix/lib/node_modules/@earendil-works/pi-coding-agent/dist/bundle\" \"$prefix/bin\"\nprintf '{{\"version\":\"{installed_version}\"}}' > \"$prefix/lib/node_modules/@earendil-works/pi-coding-agent/package.json\"\nprintf '#!/usr/bin/env node\\n' > \"$prefix/lib/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js\"\nln -s ../lib/node_modules/@earendil-works/pi-coding-agent/dist/bundle/cli.js \"$prefix/bin/pi\"\nexit {status}\n",
            arguments_path.display()
        );
        let mut file = fs::File::create(&path)?;
        file.write_all(script.as_bytes())?;
        let mut permissions = file.metadata()?.permissions();
        permissions.set_mode(0o755);
        file.set_permissions(permissions)?;
        Ok(path)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn install_publishes_exact_version_and_activates_it() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let versions_dir = temp.path().join("versions/pi");
        let npm = fake_npm(temp.path(), "0.83.0", true)?;
        let manager = PiManager::with_paths(versions_dir.clone(), npm);

        manager.install("0.83.0").await?;

        assert_eq!(manager.current_version().as_deref(), Some("0.83.0"));
        assert_eq!(manager.list_installed()?, vec!["0.83.0"]);
        assert!(
            versions_dir
                .join("current/bin/pi")
                .canonicalize()?
                .is_file()
        );
        let arguments = fs::read_to_string(temp.path().join("npm-args"))?;
        assert!(
            arguments
                .lines()
                .any(|argument| argument == "--ignore-scripts")
        );
        assert!(arguments.lines().any(|argument| argument == "--global"));
        assert!(
            arguments
                .lines()
                .any(|argument| argument == "@earendil-works/pi-coding-agent@0.83.0")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn hostile_version_is_rejected_before_npm_execution() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let manager = PiManager::with_paths(
            temp.path().join("versions/pi"),
            temp.path().join("npm-does-not-exist"),
        );

        let error = manager
            .install("../../malicious")
            .await
            .expect_err("hostile version must be rejected at the boundary");

        assert!(!error.to_string().is_empty());
        assert!(!temp.path().join("npm-args").exists());
        assert!(!temp.path().join("versions/pi").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn failed_install_does_not_publish_or_replace_active_version() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let versions_dir = temp.path().join("versions/pi");
        let initial_npm = fake_npm(temp.path(), "0.82.0", true)?;
        let initial_manager = PiManager::with_paths(versions_dir.clone(), initial_npm);
        initial_manager.install("0.82.0").await?;

        let failing_npm = fake_npm(temp.path(), "0.83.0", false)?;
        let manager = PiManager::with_paths(versions_dir.clone(), failing_npm);
        let error = manager
            .install("0.83.0")
            .await
            .expect_err("failed npm install must remain unpublished");

        assert!(error.to_string().contains("npm failed"));
        assert_eq!(manager.current_version().as_deref(), Some("0.82.0"));
        assert!(!versions_dir.join("0.83.0").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn mismatched_registry_version_fails_closed() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let versions_dir = temp.path().join("versions/pi");
        let npm = fake_npm(temp.path(), "9.9.9", true)?;
        let manager = PiManager::with_paths(versions_dir.clone(), npm);

        let error = manager
            .install("0.83.0")
            .await
            .expect_err("unexpected package version must not be published");

        assert!(error.to_string().contains("expected 0.83.0"));
        assert!(!versions_dir.join("0.83.0").exists());
        Ok(())
    }
}
