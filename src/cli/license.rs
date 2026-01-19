//! License CLI commands

use anyhow::Result;
use owo_colors::OwoColorize;
use std::io::{self, Write};

use crate::core::license::{
    self, ENTERPRISE_FEATURES, FREE_FEATURES, Feature, PRO_FEATURES, TEAM_FEATURES, Tier,
};

/// Prompt for user input
fn prompt(message: &str) -> String {
    print!("{message}");
    io::stdout().flush().unwrap();
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

/// Activate a license key
pub async fn activate(key: &str) -> Result<()> {
    println!("{} Activating license...\n", "OMG".cyan().bold());

    // Prompt for user identification (for team management)
    println!(
        "  {} For team licenses, please provide your info so your manager",
        "📋".cyan()
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
            println!("\n{} License activated successfully!\n", "✓".green());
            println!(
                "  Tier: {} {}",
                tier.display_name().cyan().bold(),
                tier.price().dimmed()
            );
            if let Some(customer) = &stored.customer {
                println!("  Customer: {customer}");
            }
            if let Some(expires) = &stored.expires_at {
                println!("  Expires: {expires}");
            }
            println!("\n  Features unlocked:");
            for feature in license::features_for_tier(tier) {
                println!("    {} {}", "✓".green(), feature.display_name());
            }
        }
        Err(e) => {
            println!("\n{} Activation failed: {}", "✗".red(), e);
            println!(
                "\n  Get a license at: {}",
                "https://pyro1121.com/pricing".cyan()
            );
        }
    }

    Ok(())
}

/// Show current license status
pub fn status() -> Result<()> {
    println!("{} License Status\n", "OMG".cyan().bold());

    let tier = license::current_tier();

    match license::status() {
        Some(stored) => {
            println!("  Status: {} ✓", "Active".green());
            println!(
                "  Tier: {} {}",
                tier.display_name().cyan().bold(),
                tier.price().dimmed()
            );
            if let Some(customer) = &stored.customer {
                println!("  Customer: {customer}");
            }
            if let Some(expires) = &stored.expires_at {
                println!("  Expires: {expires}");
            }
        }
        None => {
            println!("  Status: {} (Free tier)", "No license".yellow());
        }
    }

    // Show features by tier
    println!("\n  {} {} features:", "Free".green().bold(), "✓".green());
    for feature in FREE_FEATURES {
        println!("    {} {}", "✓".green(), feature.display_name());
    }

    println!(
        "\n  {} {} features:",
        "Pro".cyan().bold(),
        if tier >= Tier::Pro {
            "✓".green().to_string()
        } else {
            format!("{}", "$9/mo".dimmed())
        }
    );
    for feature in PRO_FEATURES {
        let icon = if license::has_feature(feature.as_str()) {
            "✓".green().to_string()
        } else {
            "✗".red().to_string()
        };
        println!("    {} {}", icon, feature.display_name());
    }

    println!(
        "\n  {} {} features:",
        "Team".magenta().bold(),
        if tier >= Tier::Team {
            "✓".green().to_string()
        } else {
            format!("{}", "$200/mo".dimmed())
        }
    );
    for feature in TEAM_FEATURES {
        let icon = if license::has_feature(feature.as_str()) {
            "✓".green().to_string()
        } else {
            "✗".red().to_string()
        };
        println!("    {} {}", icon, feature.display_name());
    }

    println!(
        "\n  {} {} features:",
        "Enterprise".yellow().bold(),
        if tier >= Tier::Enterprise {
            "✓".green().to_string()
        } else {
            format!("{}", "$200/mo".dimmed())
        }
    );
    for feature in ENTERPRISE_FEATURES {
        let icon = if license::has_feature(feature.as_str()) {
            "✓".green().to_string()
        } else {
            "✗".red().to_string()
        };
        println!("    {} {}", icon, feature.display_name());
    }

    if tier == Tier::Free {
        println!("\n  Upgrade: {}", "https://pyro1121.com/pricing".cyan());
    }

    Ok(())
}

/// Deactivate current license
pub fn deactivate() -> Result<()> {
    println!("{} Deactivating license...", "OMG".cyan().bold());

    license::remove_license()?;

    println!("\n{} License deactivated.", "✓".green());
    println!("  You are now on the free tier.");

    Ok(())
}

/// Check if a specific feature is available
pub fn check_feature(feature_name: &str) -> Result<()> {
    let Some(feature) = Feature::from_str(feature_name) else {
        println!("{} Unknown feature '{}'", "✗".red(), feature_name.cyan());
        println!(
            "  Run {} to see available features.",
            "omg license status".cyan()
        );
        return Ok(());
    };

    if license::has_feature(feature_name) {
        println!(
            "{} Feature '{}' is available",
            "✓".green(),
            feature_name.cyan()
        );
    } else {
        let required = feature.required_tier();
        println!(
            "{} Feature '{}' requires {} tier",
            "✗".red(),
            feature_name.cyan(),
            required.display_name().bold()
        );
        println!("\n  {} tier: {}", required.display_name(), required.price());
        println!("  Activate: omg license activate <key>");
        println!("  Upgrade: {}", "https://pyro1121.com/pricing".cyan());
    }

    Ok(())
}
