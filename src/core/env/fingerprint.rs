//! Environment fingerprinting and drift detection
//!
//! Captures the state of all managed runtimes and system packages
//! to detect environment drift and ensure reproducibility.

use anyhow::Result;
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tokio::task;

#[cfg(feature = "debian")]
use crate::package_managers::apt_list_explicit as list_explicit;
#[cfg(feature = "arch")]
use crate::package_managers::list_explicit;
use crate::runtimes::{
    BunManager, GoManager, JavaManager, NodeManager, PythonManager, RubyManager, RustManager,
};

/// Represents the captured state of the environment
#[allow(clippy::unsafe_derive_deserialize)]
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
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

        if let Ok(Some(v)) = node {
            runtimes.insert("node".to_string(), v.trim().to_string());
        }
        if let Ok(Some(v)) = python {
            runtimes.insert("python".to_string(), v.trim().to_string());
        }
        if let Ok(Some(v)) = rust {
            runtimes.insert("rust".to_string(), v.trim().to_string());
        }
        if let Ok(Some(v)) = go {
            runtimes.insert("go".to_string(), v.trim().to_string());
        }
        if let Ok(Some(v)) = ruby {
            runtimes.insert("ruby".to_string(), v.trim().to_string());
        }
        if let Ok(Some(v)) = java {
            runtimes.insert("java".to_string(), v.trim().to_string());
        }
        if let Ok(Some(v)) = bun {
            runtimes.insert("bun".to_string(), v.trim().to_string());
        }

        // Capture system packages
        #[cfg(feature = "arch")]
        let mut packages: Vec<String> = list_explicit()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|pkg: String| pkg.trim().to_string())
            .filter(|pkg: &String| !pkg.is_empty())
            .collect();
        #[cfg(feature = "debian")]
        let mut packages: Vec<String> = list_explicit()
            .unwrap_or_default()
            .into_iter()
            .map(|pkg: String| pkg.trim().to_string())
            .filter(|pkg: &String| !pkg.is_empty())
            .collect();
        #[cfg(not(any(feature = "arch", feature = "debian")))]
        let mut packages: Vec<String> = Vec::new();
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
        fs::write(path, content)?;
        Ok(())
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
        runtimes.sort_by(|(lhs, _), (rhs, _)| lhs.cmp(rhs));

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
