use anyhow::{Context, Result};
use owo_colors::OwoColorize;

use crate::cli::style;
use crate::config::Settings;
use crate::core::telemetry::is_telemetry_opt_out;

pub fn status() -> Result<()> {
    let opt_out = is_telemetry_opt_out();
    let settings = Settings::load().context("Failed to load OMG settings")?;

    println!(
        "{}",
        style::maybe_color("OMG Telemetry Status", |t| {
            t.bold().underline().to_string()
        })
    );
    println!();

    let status_str = if opt_out {
        style::maybe_color("Disabled", |t| t.red().bold().to_string())
    } else {
        style::maybe_color("Enabled", |t| t.green().bold().to_string())
    };

    println!("  Status: {status_str}");
    println!(
        "  Config: {}",
        if settings.telemetry_enabled {
            "Enabled in config"
        } else {
            "Disabled in config"
        }
    );

    if std::env::var("OMG_TELEMETRY").is_ok() || std::env::var("OMG_DISABLE_TELEMETRY").is_ok() {
        println!("  Environment: {}", style::path("Overridden by env var"));
    }

    println!();
    println!("  Telemetry helps us improve OMG by collecting anonymous usage data.");
    println!("  We never collect personal information or sensitive data.");
    println!("  Data is sent to: {}", crate::core::service_api::ORIGIN);

    Ok(())
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let mut settings = Settings::load().context("Failed to load OMG settings")?;
    settings.telemetry_enabled = enabled;
    settings.save()?;

    let status = if enabled {
        style::maybe_color("enabled", |t| t.green().bold().to_string())
    } else {
        style::maybe_color("disabled", |t| t.red().bold().to_string())
    };

    println!(
        "{} Telemetry has been {}.",
        style::maybe_color("✓", |t| t.green().to_string()),
        status
    );

    if std::env::var("OMG_TELEMETRY").is_ok() || std::env::var("OMG_DISABLE_TELEMETRY").is_ok() {
        println!();
        println!(
            "{} An environment variable (OMG_TELEMETRY or OMG_DISABLE_TELEMETRY) is currently set and may override this setting.",
            style::maybe_color("Note:", |t| t.bold().yellow().to_string())
        );
    }

    Ok(())
}

pub fn toggle() -> Result<()> {
    let settings = Settings::load().context("Failed to load OMG settings")?;
    set_enabled(!settings.telemetry_enabled)
}

// =============================================================================
// Privacy Rights (GDPR/CCPA - Available Globally)
// =============================================================================

/// Show local privacy settings and direct account-level requests to the authenticated website.
pub fn privacy_status() -> Result<()> {
    let settings = Settings::load().context("Failed to load OMG settings")?;
    println!(
        "{}",
        style::maybe_color("OMG Privacy Settings", |t| t.bold().underline().to_string())
    );
    println!();
    println!(
        "  Telemetry: {}",
        if settings.telemetry_enabled && !is_telemetry_opt_out() {
            style::maybe_color("Enabled", |t| t.green().to_string())
        } else {
            style::maybe_color("Disabled", |t| t.red().to_string())
        }
    );
    println!();
    println!("  omg privacy export   Export local OMG data");
    println!("  omg privacy opt-out  Disable telemetry collection");
    println!("  omg privacy opt-in   Re-enable telemetry");
    println!();
    println!("  Account export and deletion require an authenticated session:");
    println!("  https://omg.latham.cloud/privacy/");
    Ok(())
}

/// Export all user data (Right to Portability)
pub fn export_data(output_path: Option<&str>) -> Result<()> {
    println!(
        "  {} Collecting local data...",
        style::maybe_color("⏳", |_| "⏳".to_string())
    );

    let data = serde_json::json!({
        "exported_at": jiff::Timestamp::now().to_string(),
        "scope": "local",
        "local": collect_local_privacy_data()?,
    });
    let path = output_path.map_or_else(
        || {
            let date = jiff::Zoned::now().date().to_string();
            format!("omg-data-export-{date}.json")
        },
        String::from,
    );
    crate::core::safe_ops::atomic_write_file_sync(&path, serde_json::to_vec_pretty(&data)?)?;

    println!(
        "  {} Data exported to: {}",
        style::maybe_color("✓", |t| t.green().to_string()),
        style::path(&path)
    );
    Ok(())
}

fn collect_local_privacy_data() -> Result<serde_json::Value> {
    let data_dir = crate::core::paths::data_dir();
    let config_path = crate::core::paths::config_dir().join("config.toml");
    let mut files = serde_json::Map::new();

    for name in [
        "usage.json",
        "telemetry_queue.json",
        "telemetry_session.json",
        "history.json",
    ] {
        let path = data_dir.join(name);
        if path.is_file() {
            let bytes = std::fs::read(&path)
                .with_context(|| format!("Failed to read {}", path.display()))?;
            let value = serde_json::from_slice(&bytes)
                .with_context(|| format!("Failed to parse {}", path.display()))?;
            files.insert(name.to_string(), value);
        }
    }

    if config_path.is_file() {
        let config = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read {}", config_path.display()))?;
        files.insert("config.toml".to_string(), serde_json::Value::String(config));
    }

    Ok(serde_json::Value::Object(files))
}

/// Disable telemetry on this machine.
pub fn opt_out_api() -> Result<()> {
    let mut settings = Settings::load().context("Failed to load OMG settings")?;
    settings.telemetry_enabled = false;
    settings.save()?;
    println!(
        "  {} Telemetry disabled locally",
        style::maybe_color("✓", |t| t.green().to_string())
    );
    Ok(())
}

/// Enable telemetry on this machine.
pub fn opt_in_api() -> Result<()> {
    let mut settings = Settings::load().context("Failed to load OMG settings")?;
    settings.telemetry_enabled = true;
    settings.save()?;
    println!(
        "  {} Telemetry enabled locally",
        style::maybe_color("✓", |t| t.green().to_string())
    );
    println!("  Thank you for helping improve OMG!");
    Ok(())
}
