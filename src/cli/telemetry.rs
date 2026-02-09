use anyhow::{Context, Result};
use owo_colors::OwoColorize;

use crate::cli::style;
use crate::config::Settings;
use crate::core::license;
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

// =============================================================================
// Privacy Rights (GDPR/CCPA - Available Globally)
// =============================================================================

const PRIVACY_API_URL: &str = "https://api.pyro1121.com/api/privacy";

/// Show privacy status and user's current settings
pub async fn privacy_status() -> Result<()> {
    println!(
        "{}",
        style::maybe_color("OMG Privacy Settings", |t| t.bold().underline().to_string())
    );
    println!();

    // Get license info if available
    let license_key = license::load_license().map(|l| l.key);

    // Fetch privacy status from API
    let url = if let Some(ref key) = license_key {
        format!("{PRIVACY_API_URL}status?license_key={key}")
    } else {
        format!("{PRIVACY_API_URL}status")
    };

    match crate::core::http::shared_client()
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => {
            let data: serde_json::Value = response.json().await?;

            println!(
                "  {} Privacy rights available globally",
                style::maybe_color("✓", |t| t.green().to_string())
            );
            println!(
                "  {} Policy version: {}",
                style::maybe_color("•", |t| t.blue().to_string()),
                data["privacy_policy_version"].as_str().unwrap_or("1.0")
            );
            println!();

            println!(
                "  {}",
                style::maybe_color("Your Rights:", |t| t.bold().to_string())
            );
            if let Some(rights) = data["your_rights"].as_array() {
                for right in rights {
                    println!("    • {}", right.as_str().unwrap_or(""));
                }
            }
            println!();

            println!(
                "  {}",
                style::maybe_color("Data Retention:", |t| t.bold().to_string())
            );
            if let Some(retention) = data["data_retention"].as_object() {
                for (key, value) in retention {
                    println!(
                        "    • {}: {}",
                        key.replace('_', " "),
                        value.as_str().unwrap_or("")
                    );
                }
            }
            println!();

            if let Some(user_status) = data["user_status"].as_object() {
                println!(
                    "  {}",
                    style::maybe_color("Your Status:", |t| t.bold().to_string())
                );
                let opt_out = user_status
                    .get("telemetry_opt_out")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                println!(
                    "    • Telemetry: {}",
                    if opt_out {
                        style::maybe_color("Disabled", |t| t.red().to_string())
                    } else {
                        style::maybe_color("Enabled", |t| t.green().to_string())
                    }
                );
            }
        }
        Ok(_) | Err(_) => {
            // Fallback to local info
            println!(
                "  {}",
                style::maybe_color("Your Rights:", |t| t.bold().to_string())
            );
            println!("    • Right to access your data");
            println!("    • Right to delete your data");
            println!("    • Right to opt-out of telemetry");
            println!("    • Right to data portability (export)");
            println!();
            println!("  Use 'omg privacy --help' for available commands.");
        }
    }

    println!();
    println!(
        "  {}",
        style::maybe_color("Commands:", |t| t.bold().to_string())
    );
    println!("    omg privacy export   Export all your data");
    println!("    omg privacy delete   Request data deletion");
    println!("    omg privacy opt-out  Disable telemetry collection");
    println!("    omg privacy opt-in   Re-enable telemetry");

    Ok(())
}

/// Export all user data (Right to Portability)
pub async fn export_data(output_path: Option<&str>) -> Result<()> {
    let license = license::load_license()
        .context("No license found. Activate with 'omg license activate <key>' first.")?;

    println!(
        "  {} Requesting data export...",
        style::maybe_color("⏳", |_| "⏳".to_string())
    );

    let response = crate::core::http::shared_client()
        .post(format!("{PRIVACY_API_URL}/export"))
        .json(&serde_json::json!({ "license_key": license.key }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .context("Failed to connect to privacy API")?;

    if !response.status().is_success() {
        let err: serde_json::Value = response.json().await.unwrap_or_default();
        anyhow::bail!(
            "Export failed: {}",
            err["error"].as_str().unwrap_or("Unknown error")
        );
    }

    let data: serde_json::Value = response.json().await?;

    // Determine output path
    let path = output_path.map_or_else(
        || {
            let date = jiff::Zoned::now().date().to_string();
            format!("omg-data-export-{date}.json")
        },
        String::from,
    );

    // Write to file
    let content = serde_json::to_string_pretty(&data)?;
    std::fs::write(&path, &content)?;

    println!(
        "  {} Data exported to: {}",
        style::maybe_color("✓", |t| t.green().to_string()),
        style::path(&path)
    );

    Ok(())
}

/// Request data deletion (Right to Erasure)
pub async fn delete_data(confirm: bool) -> Result<()> {
    let license = license::load_license()
        .context("No license found. Activate with 'omg license activate <key>' first.")?;

    if !confirm {
        println!();
        println!(
            "  {} {}",
            style::maybe_color("⚠", |t| t.yellow().bold().to_string()),
            style::maybe_color("Data Deletion Request", |t| t.bold().to_string())
        );
        println!();
        println!("  This will permanently delete:");
        println!("    • Command history and session data");
        println!("    • Performance metrics");
        println!("    • Feature usage tracking");
        println!("    • Machine associations");
        println!();
        println!(
            "  This action is {} and will take effect immediately.",
            style::maybe_color("IRREVERSIBLE", |t| t.red().bold().to_string())
        );
        println!();
        println!("  Your license will remain active, but all telemetry data will be erased.");
        println!();
        println!("  To confirm, run:");
        println!("    omg privacy delete --confirm");
        return Ok(());
    }

    println!(
        "  {} Requesting data deletion...",
        style::maybe_color("⏳", |_| "⏳".to_string())
    );

    let response = crate::core::http::shared_client()
        .post(format!("{PRIVACY_API_URL}/delete"))
        .json(&serde_json::json!({
            "license_key": license.key,
            "machine_id": license::get_machine_id(),
            "confirm": true,
            "reason": "User requested via CLI"
        }))
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .context("Failed to connect to privacy API")?;

    if !response.status().is_success() {
        let err: serde_json::Value = response.json().await.unwrap_or_default();
        anyhow::bail!(
            "Deletion failed: {}",
            err["error"].as_str().unwrap_or("Unknown error")
        );
    }

    let result: serde_json::Value = response.json().await?;

    println!(
        "  {} {}",
        style::maybe_color("✓", |t| t.green().to_string()),
        style::maybe_color("Data deleted successfully", |t| t.bold().to_string())
    );
    println!();

    if let Some(deleted) = result["deleted"].as_object() {
        println!("  Deleted records:");
        for (table, count) in deleted {
            if let Some(n) = count.as_u64() && n > 0 {
                println!("    • {}: {n} records", table.replace('_', " "));
            }
        }
    }

    println!();
    println!(
        "  Request ID: {}",
        result["request_id"].as_str().unwrap_or("unknown")
    );

    Ok(())
}

/// Opt-out of telemetry via API (syncs with server)
pub async fn opt_out_api() -> Result<()> {
    let license = license::load_license().context("No license found. Using local opt-out only.")?;

    // Set local config
    let mut settings = Settings::load().unwrap_or_default();
    settings.telemetry_enabled = false;
    settings.save()?;

    // Sync with server
    let response = crate::core::http::shared_client()
        .post(format!("{PRIVACY_API_URL}/opt-out"))
        .json(&serde_json::json!({
            "license_key": license.key,
            "opt_out": true
        }))
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await;

    match response {
        Ok(r) if r.status().is_success() => {
            println!(
                "  {} Telemetry disabled (synced with server)",
                style::maybe_color("✓", |t| t.green().to_string())
            );
        }
        _ => {
            println!(
                "  {} Telemetry disabled locally (server sync pending)",
                style::maybe_color("✓", |t| t.green().to_string())
            );
        }
    }

    println!("  Your license remains fully functional.");

    Ok(())
}

/// Opt-in to telemetry via API (syncs with server)
pub async fn opt_in_api() -> Result<()> {
    let license = license::load_license();

    // Set local config
    let mut settings = Settings::load().unwrap_or_default();
    settings.telemetry_enabled = true;
    settings.save()?;

    // Sync with server if we have a license
    if let Some(lic) = license {
        let response = crate::core::http::shared_client()
            .post(format!("{PRIVACY_API_URL}/opt-out"))
            .json(&serde_json::json!({
                "license_key": lic.key,
                "opt_out": false
            }))
            .timeout(std::time::Duration::from_secs(10))
            .send()
            .await;

        match response {
            Ok(r) if r.status().is_success() => {
                println!(
                    "  {} Telemetry enabled (synced with server)",
                    style::maybe_color("✓", |t| t.green().to_string())
                );
            }
            _ => {
                println!(
                    "  {} Telemetry enabled locally (server sync pending)",
                    style::maybe_color("✓", |t| t.green().to_string())
                );
            }
        }
    } else {
        println!(
            "  {} Telemetry enabled",
            style::maybe_color("✓", |t| t.green().to_string())
        );
    }

    println!("  Thank you for helping improve OMG!");

    Ok(())
}
