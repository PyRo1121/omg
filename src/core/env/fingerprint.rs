//! Environment fingerprinting and drift detection
//!
//! Captures the state of all managed runtimes and system packages
//! to detect environment drift and ensure reproducibility.

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tokio::task;

// When arch is enabled, use arch's list_explicit
// When only debian features are enabled (no arch), use apt_list_explicit
#[cfg(all(feature = "debian", not(feature = "arch")))]
use crate::package_managers::apt_list_explicit as list_explicit;
#[cfg(feature = "arch")]
use crate::package_managers::list_explicit;
use crate::runtimes::{
    BunManager, GoManager, JavaManager, NodeManager, PythonManager, RubyManager, RustManager,
};

/// Represents the captured state of the environment
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[expect(clippy::unsafe_derive_deserialize)] // Struct fields are all owned safe types (HashMap, Vec, String, i64); no unsafe in fields
pub struct EnvironmentState {
    /// Runtime versions (`runtime_name` -> version)
    pub runtimes: HashMap<String, String>,
    /// Explicitly installed system packages
    pub packages: Vec<String>,
    /// Timestamp of capture
    pub timestamp: i64,
    /// SHA256 hash of the state (runtimes + packages)
    pub hash: String,
}

impl EnvironmentState {
    /// Capture the current environment state
    pub async fn capture() -> Result<Self> {
        let mut runtimes = HashMap::new();

        // Capture runtimes in parallel
        let (node, python, rust, go, ruby, java, bun) = tokio::join!(
            task::spawn_blocking(|| NodeManager::new().current_version()),
            task::spawn_blocking(|| PythonManager::new().current_version()),
            task::spawn_blocking(|| RustManager::new().current_version()),
            task::spawn_blocking(|| GoManager::new().current_version()),
            task::spawn_blocking(|| RubyManager::new().current_version()),
            task::spawn_blocking(|| JavaManager::new().current_version()),
            task::spawn_blocking(|| BunManager::new().current_version()),
        );

        if let Some(v) = join_probed_version(node, "node")? {
            runtimes.insert("node".to_string(), v.trim().to_string());
        }
        if let Some(v) = join_probed_version(python, "python")? {
            runtimes.insert("python".to_string(), v.trim().to_string());
        }
        if let Some(v) = join_probed_version(rust, "rust")? {
            runtimes.insert("rust".to_string(), v.trim().to_string());
        }
        if let Some(v) = join_probed_version(go, "go")? {
            runtimes.insert("go".to_string(), v.trim().to_string());
        }
        if let Some(v) = join_probed_version(ruby, "ruby")? {
            runtimes.insert("ruby".to_string(), v.trim().to_string());
        }
        if let Some(v) = join_probed_version(java, "java")? {
            runtimes.insert("java".to_string(), v.trim().to_string());
        }
        if let Some(v) = join_probed_version(bun, "bun")? {
            runtimes.insert("bun".to_string(), v.trim().to_string());
        }

        // Capture system packages
        let mut packages = explicit_packages_for_fingerprint().await?;
        packages.sort_unstable();
        packages.dedup();

        let timestamp = jiff::Timestamp::now().as_second();

        let mut state = Self {
            runtimes,
            packages,
            timestamp,
            hash: String::new(),
        };

        state.normalize();

        // Calculate hash
        state.hash = state.calculate_hash();

        Ok(state)
    }

    /// Calculate SHA256 hash of the state
    #[must_use]
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();

        let (runtimes, packages) = self.normalized_parts();

        for (key, value) in runtimes {
            hasher.update(key.as_bytes());
            hasher.update(b":");
            hasher.update(value.as_bytes());
            hasher.update(b";");
        }

        for pkg in packages {
            hasher.update(pkg.as_bytes());
            hasher.update(b";");
        }

        hex::encode(hasher.finalize())
    }

    /// Save state to omg.lock file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut normalized = self.clone();
        normalized.normalize();
        let content = toml::to_string_pretty(&normalized)?;
        write_lockfile(path.as_ref(), content.as_bytes())
    }

    /// Load state from omg.lock file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let mut state: Self = toml::from_str(&content)?;
        state.normalize();
        Ok(state)
    }

    fn normalize(&mut self) {
        let (runtimes, packages) = self.normalized_parts();
        self.runtimes = runtimes.into_iter().collect();
        self.packages = packages;
    }

    fn normalized_parts(&self) -> (Vec<(String, String)>, Vec<String>) {
        let mut runtimes: Vec<(String, String)> = self
            .runtimes
            .iter()
            .map(|(k, v)| (k.trim().to_string(), v.trim().to_string()))
            .collect();
        runtimes.sort_by(|a, b| a.0.cmp(&b.0));

        let mut packages: Vec<String> = self
            .packages
            .iter()
            .map(|pkg| pkg.trim().to_string())
            .filter(|pkg| !pkg.is_empty())
            .collect();
        packages.sort_unstable();
        packages.dedup();

        (runtimes, packages)
    }
}

fn join_probed_version(
    result: std::result::Result<Option<String>, tokio::task::JoinError>,
    runtime: &str,
) -> Result<Option<String>> {
    result.with_context(|| format!("Failed to probe {runtime} runtime"))
}

async fn explicit_packages_for_fingerprint() -> Result<Vec<String>> {
    #[cfg(feature = "arch")]
    {
        return Ok(list_explicit()
            .await
            .context("Failed to list explicitly installed packages")?
            .into_iter()
            .map(|pkg| pkg.trim().to_string())
            .filter(|pkg| !pkg.is_empty())
            .collect());
    }

    #[cfg(all(feature = "debian", not(feature = "arch")))]
    {
        return Ok(list_explicit()
            .context("Failed to list explicitly installed packages")?
            .into_iter()
            .map(|pkg| pkg.trim().to_string())
            .filter(|pkg| !pkg.is_empty())
            .collect());
    }

    #[cfg(all(
        feature = "debian-pure",
        not(feature = "arch"),
        not(feature = "debian")
    ))]
    {
        return Ok(crate::package_managers::list_explicit_fast()
            .context("Failed to list explicitly installed packages")?
            .into_iter()
            .map(|pkg| pkg.trim().to_string())
            .filter(|pkg| !pkg.is_empty())
            .collect());
    }

    #[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
    fingerprint_requires_backend()
}

#[cfg(any(
    not(any(feature = "arch", feature = "debian", feature = "debian-pure")),
    test
))]
fn fingerprint_requires_backend() -> Result<Vec<String>> {
    anyhow::bail!(
        "Environment fingerprinting is not available without an Arch or Debian package backend"
    )
}

fn write_lockfile(path: &Path, content: &[u8]) -> Result<()> {
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)
            .with_context(|| format!("Failed to create lockfile {}", path.display()))?;
        file.write_all(content)
            .with_context(|| format!("Failed to write lockfile {}", path.display()))?;
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .with_context(|| format!("Failed to set lockfile permissions {}", path.display()))?;
        Ok(())
    }
    #[cfg(not(unix))]
    {
        fs::write(path, content)
            .with_context(|| format!("Failed to write lockfile {}", path.display()))?;
        Ok(())
    }
}

/// Drift analysis result
#[derive(Debug)]
pub struct DriftReport {
    pub has_drift: bool,
    pub missing_runtimes: Vec<String>,
    pub different_runtimes: Vec<(String, String, String)>, // (name, expected, actual)
    pub extra_runtimes: Vec<String>,
    pub missing_packages: Vec<String>,
    pub extra_packages: Vec<String>,
}

impl DriftReport {
    /// Compare two states and generate a drift report
    #[must_use]
    pub fn compare(expected: &EnvironmentState, actual: &EnvironmentState) -> Self {
        let mut report = Self {
            has_drift: false,
            missing_runtimes: Vec::new(),
            different_runtimes: Vec::new(),
            extra_runtimes: Vec::new(),
            missing_packages: Vec::new(),
            extra_packages: Vec::new(),
        };

        // Check runtimes
        for (name, ver) in &expected.runtimes {
            if let Some(actual_ver) = actual.runtimes.get(name) {
                if ver != actual_ver {
                    report
                        .different_runtimes
                        .push((name.clone(), ver.clone(), actual_ver.clone()));
                    report.has_drift = true;
                }
            } else {
                report.missing_runtimes.push(name.clone());
                report.has_drift = true;
            }
        }

        for name in actual.runtimes.keys() {
            if !expected.runtimes.contains_key(name) {
                report.extra_runtimes.push(name.clone());
                report.has_drift = true;
            }
        }

        // Check packages
        for pkg in &expected.packages {
            if !actual.packages.contains(pkg) {
                report.missing_packages.push(pkg.clone());
                report.has_drift = true;
            }
        }

        for pkg in &actual.packages {
            if !expected.packages.contains(pkg) {
                report.extra_packages.push(pkg.clone());
                report.has_drift = true;
            }
        }

        report
    }

    /// Print the drift report
    pub fn print(&self) {
        if !self.has_drift {
            println!(
                "{} No drift detected. Environment matches lockfile.",
                "✓".green()
            );
            return;
        }

        println!("{} Environment drift detected!\n", "⚠".yellow().bold());

        if !self.missing_runtimes.is_empty() {
            println!("{}", "Missing Runtimes:".red());
            for r in &self.missing_runtimes {
                println!("  - {r}");
            }
        }

        if !self.different_runtimes.is_empty() {
            println!("{}", "Version Mismatches:".yellow());
            for (name, expected, actual) in &self.different_runtimes {
                println!(
                    "  ~ {} (expected: {}, actual: {})",
                    name,
                    expected.green(),
                    actual.red()
                );
            }
        }

        if !self.extra_runtimes.is_empty() {
            println!("{}", "Extra Runtimes (not in lockfile):".blue());
            for r in &self.extra_runtimes {
                println!("  + {r}");
            }
        }

        if !self.missing_packages.is_empty() {
            println!("\n{}", "Missing Packages:".red());
            for p in &self.missing_packages {
                println!("  - {p}");
            }
        }

        if !self.extra_packages.is_empty() {
            println!("\n{}", "Extra Packages (not in lockfile):".blue());
            for p in &self.extra_packages {
                println!("  + {p}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn save_creates_owner_only_lockfile() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::TempDir::new().expect("temp dir");
        let path = directory.path().join("omg.lock");
        let state = EnvironmentState {
            runtimes: HashMap::new(),
            packages: vec!["foo".to_string()],
            timestamp: 0,
            hash: "abc".to_string(),
        };

        state.save(&path).expect("save lockfile");

        let mode = std::fs::metadata(&path)
            .expect("lockfile metadata")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "lockfile must not be group or world accessible, got {mode:o}"
        );
    }

    #[test]
    fn fingerprint_without_backend_is_an_error() {
        let error = fingerprint_requires_backend()
            .expect_err("env capture with no backend must not invent an empty package list");
        assert!(
            error
                .to_string()
                .contains("not available without an Arch or Debian package backend"),
            "got: {error}"
        );
    }
}
