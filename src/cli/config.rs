//! Configuration management for OMG CLI
//!
//! Handles getting, setting, validating, and displaying configuration.

use anyhow::{Context, Result};
use dialoguer::Confirm;

use crate::cli::style;
use crate::config::Settings;
use crate::core::RuntimeBackend;

/// Get a configuration value
pub fn get(key: &str) -> Result<()> {
    // SECURITY: Validate config key
    validate_key(key)?;

    let settings = Settings::load().unwrap_or_default();

    let value = match key {
        "shims.enabled" => settings.shims_enabled.to_string(),
        "data_dir" => settings.data_dir.display().to_string(),
        "socket" => settings.socket_path.display().to_string(),
        "telemetry.enabled" => settings.telemetry_enabled.to_string(),
        "aur.build_concurrency" => settings.aur.build_concurrency.to_string(),
        "aur.enable_ccache" => settings.aur.enable_ccache.to_string(),
        "aur.enable_sccache" => settings.aur.enable_sccache.to_string(),
        "aur.secure_makepkg" => settings.aur.secure_makepkg.to_string(),
        "aur.makeflags" => settings.aur.makeflags.clone().unwrap_or_default(),
        "runtime_backend" => format!("{:?}", settings.runtime_backend).to_lowercase(),
        "default_shell" => settings.default_shell.clone(),
        _ => {
            println!("{} Unknown key: {}", style::error("✗"), key);
            return Ok(());
        }
    };

    println!("{}", value);
    Ok(())
}

/// Set a configuration value
pub fn set(key: &str, value: &str) -> Result<()> {
    // SECURITY: Validate config key and value
    validate_key(key)?;
    validate_value(value)?;

    let mut settings = Settings::load().unwrap_or_default();

    match key {
        "telemetry.enabled" => {
            settings.telemetry_enabled = value.parse().context("Invalid boolean value")?;
        }
        "aur.build_concurrency" => {
            settings.aur.build_concurrency = value.parse().context("Invalid number")?;
        }
        "aur.enable_ccache" => {
            settings.aur.enable_ccache = value.parse().context("Invalid boolean value")?;
        }
        "aur.enable_sccache" => {
            settings.aur.enable_sccache = value.parse().context("Invalid boolean value")?;
        }
        "aur.secure_makepkg" => {
            settings.aur.secure_makepkg = value.parse().context("Invalid boolean value")?;
        }
        "aur.makeflags" => {
            settings.aur.makeflags = if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            };
        }
        "runtime_backend" => {
            settings.runtime_backend = match value.to_lowercase().as_str() {
                "native" => RuntimeBackend::Native,
                "mise" => RuntimeBackend::Mise,
                "native-then-mise" | "nativetomise" => RuntimeBackend::NativeThenMise,
                _ => anyhow::bail!(
                    "Invalid runtime backend. Valid values: native, mise, native-then-mise"
                ),
            };
        }
        "default_shell" => {
            if !["bash", "zsh", "fish"].contains(&value) {
                anyhow::bail!("Invalid shell. Valid values: bash, zsh, fish");
            }
            settings.default_shell = value.to_string();
        }
        "shims.enabled" => {
            settings.shims_enabled = value.parse().context("Invalid boolean value")?;
        }
        _ => {
            anyhow::bail!("Unknown or read-only key: {key}");
        }
    }

    settings.save()?;
    println!("{} Set {} = {}", style::success("✓"), key, value);
    Ok(())
}

/// List all configuration values
pub fn list() -> Result<()> {
    let settings = Settings::load().unwrap_or_default();

    println!("{}", style::header("OMG Configuration"));
    println!();

    // Paths (read-only)
    println!("  {}", style::dim("Paths:"));
    println!(
        "    {} = {}",
        style::info("data_dir"),
        settings.data_dir.display()
    );
    println!(
        "    {} = {}",
        style::info("socket"),
        settings.socket_path.display()
    );
    println!("    {} = {}", style::info("config_file"), config_path());

    // General settings
    println!();
    println!("  {}", style::dim("General:"));
    println!(
        "    {} = {}",
        style::info("telemetry.enabled"),
        settings.telemetry_enabled
    );
    println!(
        "    {} = {}",
        style::info("runtime_backend"),
        format!("{:?}", settings.runtime_backend).to_lowercase()
    );
    println!(
        "    {} = {}",
        style::info("default_shell"),
        settings.default_shell
    );
    println!(
        "    {} = {}",
        style::info("shims.enabled"),
        settings.shims_enabled
    );

    // AUR settings
    println!();
    println!("  {}", style::dim("AUR Build:"));
    println!(
        "    {} = {}",
        style::info("aur.build_concurrency"),
        settings.aur.build_concurrency
    );
    println!(
        "    {} = {}",
        style::info("aur.enable_ccache"),
        settings.aur.enable_ccache
    );
    println!(
        "    {} = {}",
        style::info("aur.enable_sccache"),
        settings.aur.enable_sccache
    );
    println!(
        "    {} = {}",
        style::info("aur.secure_makepkg"),
        settings.aur.secure_makepkg
    );
    println!(
        "    {} = {}",
        style::info("aur.makeflags"),
        settings.aur.makeflags.as_deref().unwrap_or("(not set)")
    );

    println!();
    Ok(())
}

/// Validate configuration file
pub fn validate() -> Result<()> {
    println!("{} Validating configuration...", style::info("→"));
    println!();

    let config_file = config_path();
    let mut issues = 0;

    // Check if config file exists
    if !std::path::Path::new(&config_file).exists() {
        println!(
            "  {} No config file found (using defaults)",
            style::dim("•")
        );
        println!("    Path: {}", config_file);
        println!();
        println!(
            "{} Configuration is valid (using defaults)",
            style::success("✓")
        );
        return Ok(());
    }

    println!(
        "  {} Found config file: {}",
        style::success("✓"),
        config_file
    );

    // Try to parse the config
    let content = std::fs::read_to_string(&config_file)?;

    // Check TOML syntax
    match toml::from_str::<toml::Value>(&content) {
        Ok(_) => {
            println!("  {} TOML syntax is valid", style::success("✓"));
        }
        Err(e) => {
            println!("  {} TOML syntax error: {}", style::error("✗"), e);
            issues += 1;
        }
    }

    // Try to deserialize into Settings
    match Settings::load() {
        Ok(settings) => {
            println!("  {} Configuration schema is valid", style::success("✓"));

            // Validate specific values
            if settings.aur.build_concurrency == 0 {
                println!(
                    "  {} aur.build_concurrency should be > 0",
                    style::warning("⚠")
                );
                issues += 1;
            }

            if settings.aur.build_concurrency > 64 {
                println!(
                    "  {} aur.build_concurrency is unusually high ({})",
                    style::warning("⚠"),
                    settings.aur.build_concurrency
                );
            }

            // Check default shell
            if !["bash", "zsh", "fish"].contains(&settings.default_shell.as_str()) {
                println!(
                    "  {} Invalid default_shell: {}",
                    style::error("✗"),
                    settings.default_shell
                );
                issues += 1;
            }
        }
        Err(e) => {
            println!(
                "  {} Failed to load configuration: {}",
                style::error("✗"),
                e
            );
            issues += 1;
        }
    }

    // Check file permissions
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(&config_file) {
            let mode = meta.permissions().mode();
            if mode & 0o077 != 0 {
                println!(
                    "  {} Config file has loose permissions ({})",
                    style::warning("⚠"),
                    format!("{:o}", mode & 0o777)
                );
                println!("    Consider running: chmod 600 {}", config_file);
            } else {
                println!("  {} File permissions are secure", style::success("✓"));
            }
        }
    }

    println!();
    if issues == 0 {
        println!("{} Configuration is valid!", style::success("✓"));
    } else {
        println!("{} Found {} issue(s)", style::warning("⚠"), issues);
    }

    Ok(())
}

/// Reset configuration to defaults
pub fn reset(yes: bool) -> Result<()> {
    let config_file = config_path();

    if !std::path::Path::new(&config_file).exists() {
        println!("{} No config file exists", style::dim("•"));
        return Ok(());
    }

    if !yes {
        let confirm = Confirm::new()
            .with_prompt("Reset configuration to defaults? This cannot be undone.")
            .default(false)
            .interact()?;

        if !confirm {
            println!("{} Cancelled", style::dim("•"));
            return Ok(());
        }
    }

    // Create backup
    let backup_path = format!("{}.backup", config_file);
    std::fs::copy(&config_file, &backup_path)?;
    println!("  {} Created backup at {}", style::dim("•"), backup_path);

    // Remove the config file
    std::fs::remove_file(&config_file)?;

    // Create default config
    let default_settings = Settings::default();
    default_settings.save()?;

    println!("{} Configuration reset to defaults", style::success("✓"));
    Ok(())
}

/// Show configuration file path
pub fn path() -> Result<()> {
    println!("{}", config_path());
    Ok(())
}

/// Get the configuration file path
fn config_path() -> String {
    dirs::config_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("~/.config"))
        .join("omg")
        .join("config.toml")
        .display()
        .to_string()
}

/// Validate a configuration key
fn validate_key(key: &str) -> Result<()> {
    if key
        .chars()
        .any(|c| !c.is_ascii_alphanumeric() && c != '.' && c != '_')
    {
        anyhow::bail!("Invalid configuration key: {key}");
    }
    if key.len() > 64 {
        anyhow::bail!("Configuration key too long");
    }
    Ok(())
}

/// Validate a configuration value
fn validate_value(value: &str) -> Result<()> {
    if value.len() > 1024 {
        anyhow::bail!("Configuration value too long");
    }
    Ok(())
}
