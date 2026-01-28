use anyhow::Result;
use owo_colors::OwoColorize;

use crate::cli::style;
use crate::config::Settings;
use crate::core::telemetry::is_telemetry_opt_out;

pub fn status() -> Result<()> {
    let opt_out = is_telemetry_opt_out();
    let settings = Settings::load().unwrap_or_default();

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

    println!("  Status: {}", status_str);
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
    println!("  Data is sent to: https://api.pyro1121.com");

    Ok(())
}

pub fn set_enabled(enabled: bool) -> Result<()> {
    let mut settings = Settings::load().unwrap_or_default();
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
    let settings = Settings::load().unwrap_or_default();
    set_enabled(!settings.telemetry_enabled)
}
