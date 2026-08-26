//! License CLI commands

use anyhow::Result;
use owo_colors::OwoColorize;
use std::io::{self, Write};

use crate::cli::style;
use crate::core::license::{
    self, ENTERPRISE_FEATURES, FREE_FEATURES, Feature, PRO_FEATURES, StoredLicense, TEAM_FEATURES,
    Tier,
};

/// Prompt for user input
fn prompt(message: &str) -> String {
    print!("{message}");
    let _ = io::stdout().flush();
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
    input.trim().to_string()
}

/// Activate a license key
///
/// # Errors
///
/// Returns an error when the license key format is invalid or the activation
/// request fails (network, server rejection, or persistence failure).
pub async fn activate(key: &str) -> Result<()> {
    // SECURITY: Validate license key format
    if key.len() > 128 || key.chars().any(|c| !c.is_ascii_alphanumeric() && c != '-') {
        anyhow::bail!("Invalid license key format");
    }

    println!("{} Activating license...\n", style::runtime("OMG"));

    // Prompt for user identification (for team management)
    println!(
        "  {} For team licenses, please provide your info so your manager",
        style::maybe_color("📋", |t| t.cyan().to_string())
    );
    println!("     can identify you in the dashboard. Press Enter to skip.\n");

    let user_name = prompt("  Your name (optional): ");
    let user_email = prompt("  Your email (optional): ");

    let user_name_opt = if user_name.is_empty() {
        None
    } else {
        Some(user_name.as_str())
    };
    let user_email_opt = if user_email.is_empty() {
        None
    } else {
        Some(user_email.as_str())
    };

    println!("\n  Validating license...");

    report_activation(license::activate_with_user(key, user_name_opt, user_email_opt).await)
}

/// Prints activation outcome. Failed validation must be `Err` so the CLI exits non-zero.
fn report_activation(result: Result<license::StoredLicense>) -> Result<()> {
    match result {
        Ok(stored) => {
            let tier = stored.tier_enum();
            println!(
                "\n{} License activated successfully!\n",
                style::maybe_color("✓", |t| t.green().to_string())
            );
            println!(
                "  Tier: {} {}",
                style::runtime(tier.display_name()),
                style::dim(tier.price())
            );
            if let Some(customer) = &stored.customer {
                println!("  Customer: {customer}");
            }
            if let Some(expires) = &stored.expires_at {
                println!("  Expires: {expires}");
            }
            println!("\n  Features unlocked:");
            for feature in license::features_for_tier(tier) {
                println!(
                    "    {} {}",
                    style::maybe_color("✓", |t| t.green().to_string()),
                    feature.display_name()
                );
            }
            Ok(())
        }
        Err(e) => {
            println!(
                "\n{} Activation failed: {}",
                style::maybe_color("✗", |t| t.red().to_string()),
                e
            );
            println!(
                "\n  Get a license at: {}",
                style::url("https://pyro1121.com/pricing")
            );
            Err(e)
        }
    }
}

/// Print one tier's feature list. `unlocked` drives both the header mark
/// (✓ vs price) and the per-feature ✓/✗ icons; within a tier every feature
/// has the same required tier, so one flag covers the whole group.
fn print_feature_group(
    styled_label: &str,
    unlocked: bool,
    locked_label: &str,
    features: &[Feature],
) {
    let mark = if unlocked {
        style::maybe_color("✓", |t| t.green().to_string())
    } else {
        style::dim(locked_label)
    };
    println!("\n  {styled_label} {mark} features:");

    let icon = if unlocked {
        style::maybe_color("✓", |t| t.green().to_string())
    } else {
        style::maybe_color("✗", |t| t.red().to_string())
    };
    for feature in features {
        println!("    {icon} {}", feature.display_name());
    }
}

/// Show current license status
pub fn status() -> Result<()> {
    println!("{} License Status\n", style::runtime("OMG"));

    // Read stored license once; derive both the display record and the
    // effective (signature-verified) tier from it.
    let stored = license::status();
    let tier = stored.as_ref().map_or(Tier::Free, StoredLicense::tier_enum);

    if let Some(stored) = &stored {
        println!("  Status: {} ✓", style::version("Active"));
        println!(
            "  Tier: {} {}",
            style::runtime(tier.display_name()),
            style::dim(tier.price())
        );
        if let Some(customer) = &stored.customer {
            println!("  Customer: {customer}");
        }
        if let Some(expires) = &stored.expires_at {
            println!("  Expires: {expires}");
        }
    } else {
        println!(
            "  Status: {} (Free tier)",
            style::maybe_color("No license", |t| t.yellow().to_string())
        );
    }

    print_feature_group(
        &style::maybe_color("Free", |t| t.green().bold().to_string()),
        true,
        "",
        FREE_FEATURES,
    );
    print_feature_group(
        &style::runtime("Pro"),
        tier >= Tier::Pro,
        "$9/mo",
        PRO_FEATURES,
    );
    print_feature_group(
        &style::maybe_color("Team", |t| t.magenta().bold().to_string()),
        tier >= Tier::Team,
        "$200/mo",
        TEAM_FEATURES,
    );
    print_feature_group(
        &style::highlight("Enterprise"),
        tier >= Tier::Enterprise,
        "$200/mo",
        ENTERPRISE_FEATURES,
    );

    if tier == Tier::Free {
        println!(
            "\n  Upgrade: {}",
            style::url("https://pyro1121.com/pricing")
        );
    }

    Ok(())
}

/// Deactivate current license
pub fn deactivate() -> Result<()> {
    println!("{} Deactivating license...", style::runtime("OMG"));

    license::remove_license()?;

    println!(
        "\n{} License deactivated.",
        style::maybe_color("✓", |t| t.green().to_string())
    );
    println!("  You are now on the free tier.");

    Ok(())
}

/// Check if a specific feature is available
pub fn check_feature(feature_name: &str) -> Result<()> {
    // No manual charset pre-validation needed: Feature::from_str is the
    // authoritative allowlist and rejects anything that isn't a known
    // feature name, including malformed input.
    let Some(feature) = Feature::from_str(feature_name) else {
        anyhow::bail!(
            "Unknown feature '{feature_name}'. Run `omg license status` to see available features."
        );
    };

    if license::has_feature(feature_name) {
        println!(
            "{} Feature '{}' is available",
            style::maybe_color("✓", |t| t.green().to_string()),
            style::maybe_color(feature_name, |t| t.cyan().to_string())
        );
    } else {
        let required = feature.required_tier();
        println!(
            "{} Feature '{}' requires {} tier",
            style::maybe_color("✗", |t| t.red().to_string()),
            style::maybe_color(feature_name, |t| t.cyan().to_string()),
            style::maybe_color(required.display_name(), |t| t.bold().to_string())
        );
        println!("\n  {} tier: {}", required.display_name(), required.price());
        println!("  Activate: omg license activate <key>");
        println!("  Upgrade: {}", style::url("https://pyro1121.com/pricing"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activation_failure_returns_err() {
        let err = anyhow::anyhow!("Invalid license: revoked");
        let result = report_activation(Err(err));
        assert!(
            result.is_err(),
            "failed activation must be a CLI error so the process exits non-zero"
        );
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid license: revoked"),
            "original activation error must be preserved"
        );
    }
}
