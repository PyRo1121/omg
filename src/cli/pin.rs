//! `omg pin` - Pin packages or runtimes to prevent updates

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::cli::style;
use crate::core::paths;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PinConfig {
    /// Pinned packages: name -> version (None = pin to current)
    pub packages: HashMap<String, Option<String>>,
    /// Pinned runtimes: runtime@version
    pub runtimes: HashMap<String, String>,
}

impl PinConfig {
    fn path() -> PathBuf {
        paths::config_dir().join("pins.toml")
    }

    pub fn load() -> Result<Self> {
        let path = Self::path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read pins file: {}", path.display()))?;
            Ok(toml::from_str(&content)
                .with_context(|| format!("Failed to parse pins file: {}", path.display()))?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create pins directory: {}", parent.display())
            })?;
        }
        let content = toml::to_string_pretty(self).context("Failed to serialize pins")?;
        std::fs::write(&path, &content)
            .with_context(|| format!("Failed to write pins file: {}", path.display()))?;
        Ok(())
    }

    pub fn is_pinned(&self, name: &str) -> bool {
        self.packages.contains_key(name) || self.runtimes.contains_key(name)
    }
}

/// Run the pin command
pub fn run(target: &str, unpin: bool, list: bool) -> Result<()> {
    if list {
        return list_pins();
    }

    let mut config = PinConfig::load()?;

    if unpin {
        unpin_target(&mut config, target);
    } else {
        pin_target(&mut config, target)?;
    }

    config.save()?;
    Ok(())
}

fn list_pins() -> Result<()> {
    let config = PinConfig::load()?;

    println!("{} Pinned Packages & Runtimes\n", style::runtime("OMG"));

    if config.packages.is_empty() && config.runtimes.is_empty() {
        println!("  {} No pins configured", style::dim("○"));
        println!();
        println!("  Pin a package:  {}", style::command("omg pin gcc"));
        println!(
            "  Pin a runtime:  {}",
            style::command("omg pin node@20.10.0")
        );
        return Ok(());
    }

    if !config.packages.is_empty() {
        println!(
            "  {}",
            style::maybe_color("Packages:", |t| t.bold().to_string())
        );
        for (name, version) in &config.packages {
            let ver_str = version.as_deref().unwrap_or("(current)");
            println!(
                "    {} {} @ {}",
                style::dim("📌"),
                style::path(name),
                style::dim(ver_str)
            );
        }
        println!();
    }

    if !config.runtimes.is_empty() {
        println!(
            "  {}",
            style::maybe_color("Runtimes:", |t| t.bold().to_string())
        );
        for (runtime, version) in &config.runtimes {
            println!(
                "    {} {} @ {}",
                style::dim("📌"),
                style::path(runtime),
                style::dim(version)
            );
        }
        println!();
    }

    println!("  To unpin: {}", style::command("omg pin <name> --unpin"));

    Ok(())
}

fn pin_target(config: &mut PinConfig, target: &str) -> Result<()> {
    // Check if it's a runtime pin (contains @)
    if let Some((runtime, version)) = target.split_once('@') {
        // Validate runtime
        let valid_runtimes = ["node", "python", "go", "rust", "ruby", "java", "bun"];
        if !valid_runtimes.contains(&runtime) {
            anyhow::bail!(
                "Unknown runtime '{}'. Valid runtimes: {}",
                runtime,
                valid_runtimes.join(", ")
            );
        }

        // SECURITY: Validate version string
        crate::core::security::validate_version(version)?;

        config
            .runtimes
            .insert(runtime.to_string(), version.to_string());
        println!(
            "{} Pinned {} to version {}",
            style::maybe_color("✓", |t| t.green().to_string()),
            style::path(runtime),
            style::maybe_color(version, |t| t.cyan().to_string())
        );
        println!("  This runtime will not be auto-updated.");
    } else {
        // SECURITY: Validate package name
        crate::core::security::validate_package_name(target)?;

        // Package pin
        let current_version = get_package_version(target)?;
        config
            .packages
            .insert(target.to_string(), current_version.clone());

        let ver_display = current_version.as_deref().unwrap_or("current version");
        println!(
            "{} Pinned {} to {}",
            style::maybe_color("✓", |t| t.green().to_string()),
            style::path(target),
            style::maybe_color(ver_display, |t| t.cyan().to_string())
        );
        println!("  This package will be excluded from updates.");
    }

    Ok(())
}

fn unpin_target(config: &mut PinConfig, target: &str) {
    // Check if it's a runtime
    let (runtime_name, version) = if let Some((r, v)) = target.split_once('@') {
        (r, Some(v))
    } else {
        (target, None)
    };

    if config.runtimes.remove(runtime_name).is_some() {
        println!(
            "{} Unpinned runtime {}",
            style::maybe_color("✓", |t| t.green().to_string()),
            style::path(runtime_name)
        );
        println!("  This runtime will now receive updates.");
        return;
    }

    if config.packages.remove(target).is_some() {
        println!(
            "{} Unpinned package {}",
            style::maybe_color("✓", |t| t.green().to_string()),
            style::path(target)
        );
        println!("  This package will now receive updates.");
        return;
    }

    if let Some(_v) = version
        && config.runtimes.remove(runtime_name).is_some()
    {
        println!(
            "{} Unpinned runtime {}",
            style::maybe_color("✓", |t| t.green().to_string()),
            style::path(runtime_name)
        );
        return;
    }
    println!(
        "{} '{}' is not pinned",
        style::maybe_color("⚠", |t| t.yellow().to_string()),
        target
    );
}

#[cfg(feature = "arch")]
fn get_package_version(name: &str) -> Result<Option<String>> {
    use alpm::Alpm;

    let handle = Alpm::new("/", "/var/lib/pacman")
        .map_err(|e| anyhow::anyhow!("Failed to open ALPM: {e}"))?;

    let localdb = handle.localdb();

    if let Ok(pkg) = localdb.pkg(name) {
        Ok(Some(pkg.version().to_string()))
    } else {
        // Check if it exists in sync dbs
        for syncdb in handle.syncdbs() {
            if syncdb.pkg(name).is_ok() {
                return Ok(None); // Exists but not installed
            }
        }
        anyhow::bail!("Package '{name}' not found")
    }
}

#[cfg(all(feature = "debian", not(feature = "arch")))]
fn get_package_version(name: &str) -> Result<Option<String>> {
    use std::process::Command;

    let output = Command::new("dpkg-query")
        .args(["-W", "-f=${Version}", "--", name])
        .output()?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(Some(version))
    } else {
        // Check if package exists
        let check = Command::new("apt-cache")
            .args(["show", "--", name])
            .output()?;

        if check.status.success() {
            Ok(None) // Exists but not installed
        } else {
            anyhow::bail!("Package '{name}' not found")
        }
    }
}

#[cfg(not(any(feature = "arch", feature = "debian")))]
#[allow(clippy::unnecessary_wraps)] // Result return required: API compat with feature-gated impls
fn get_package_version(_name: &str) -> Result<Option<String>> {
    Ok(None)
}
