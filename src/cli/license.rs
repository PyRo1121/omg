//! License CLI commands

use anyhow::Result;
use owo_colors::OwoColorize;
use std::io::{self, Write};

use crate::cli::style;
use crate::core::license::{
    self, ENTERPRISE_FEATURES, FREE_FEATURES, Feature, PRO_FEATURES, TEAM_FEATURES, Tier,
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

    match license::activate_with_user(key, user_name_opt, user_email_opt).await {
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
        }
    }

    Ok(())
}

/// Show current license status
pub fn status() -> Result<()> {
    println!("{} License Status\n", style::runtime("OMG"));

    let tier = license::current_tier();

    match license::status() {
        Some(stored) => {
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
        }
        None => {
            println!(
                "  Status: {} (Free tier)",
                style::maybe_color("No license", |t| t.yellow().to_string())
            );
        }
    }

    // Show features by tier
    println!(
        "\n  {} {} features:",
        style::maybe_color("Free", |t| t.green().bold().to_string()),
        style::maybe_color("✓", |t| t.green().to_string())
    );
    for feature in FREE_FEATURES {
        println!(
            "    {} {}",
            style::maybe_color("✓", |t| t.green().to_string()),
            feature.display_name()
        );
    }

    println!(
        "\n  {} {} features:",
        style::runtime("Pro"),
        if tier >= Tier::Pro {
            style::maybe_color("✓", |t| t.green().to_string())
        } else {
            style::dim("$9/mo")
        }
    );
    for feature in PRO_FEATURES {
        let icon = if license::has_feature(feature.as_str()) {
            style::maybe_color("✓", |t| t.green().to_string())
        } else {
            style::maybe_color("✗", |t| t.red().to_string())
        };
        println!("    {} {}", icon, feature.display_name());
    }

    println!(
        "\n  {} {} features:",
        style::maybe_color("Team", |t| t.magenta().bold().to_string()),
        if tier >= Tier::Team {
            style::maybe_color("✓", |t| t.green().to_string())
        } else {
            style::dim("$200/mo")
        }
    );
    for feature in TEAM_FEATURES {
        let icon = if license::has_feature(feature.as_str()) {
            style::maybe_color("✓", |t| t.green().to_string())
        } else {
            style::maybe_color("✗", |t| t.red().to_string())
        };
        println!("    {} {}", icon, feature.display_name());
    }

    println!(
        "\n  {} {} features:",
        style::highlight("Enterprise"),
        if tier >= Tier::Enterprise {
            style::maybe_color("✓", |t| t.green().to_string())
        } else {
            style::dim("$200/mo")
        }
    );
    for feature in ENTERPRISE_FEATURES {
        let icon = if license::has_feature(feature.as_str()) {
            style::maybe_color("✓", |t| t.green().to_string())
        } else {
            style::maybe_color("✗", |t| t.red().to_string())
        };
        println!("    {} {}", icon, feature.display_name());
    }

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
    // SECURITY: Validate feature name
    if feature_name.len() > 64
        || feature_name
            .chars()
            .any(|c| !c.is_ascii_alphanumeric() && c != '-')
    {
        anyhow::bail!("Invalid feature name");
    }

    let Some(feature) = Feature::from_str(feature_name) else {
        println!(
            "{} Unknown feature '{}'",
            style::maybe_color("✗", |t| t.red().to_string()),
            style::maybe_color(feature_name, |t| t.cyan().to_string())
        );
        println!(
            "  Run {} to see available features.",
            style::command("omg license status")
        );
        return Ok(());
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
