//! Man page generation for OMG CLI
//!
//! Generates man pages from clap command definitions using clap_mangen.

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap_mangen::Man;
use std::fs;
use std::path::PathBuf;

use super::args::Cli;
use super::style;

/// Generate man pages for all OMG commands
pub fn generate(output_dir: Option<String>) -> Result<()> {
    let output_path = if let Some(dir) = output_dir {
        PathBuf::from(shellexpand(&dir))
    } else {
        // Default to ~/.local/share/man/man1
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("~/.local/share"))
            .join("man")
            .join("man1")
    };

    // Create output directory if it doesn't exist
    fs::create_dir_all(&output_path)
        .with_context(|| format!("Failed to create directory: {}", output_path.display()))?;

    println!(
        "{} Generating man pages to {}",
        style::info("→"),
        output_path.display()
    );

    let cmd = Cli::command();
    let mut generated = 0;

    // Generate main omg man page
    let man = Man::new(cmd.clone());
    let man_path = output_path.join("omg.1");
    let mut buffer = Vec::new();
    man.render(&mut buffer)?;
    fs::write(&man_path, buffer)
        .with_context(|| format!("Failed to write {}", man_path.display()))?;
    println!("  {} omg.1", style::success("✓"));
    generated += 1;

    // Generate subcommand man pages
    for subcommand in cmd.get_subcommands() {
        if subcommand.is_hide_set() {
            continue;
        }

        let name = subcommand.get_name();
        let sub_man = Man::new(subcommand.clone());
        let sub_man_path = output_path.join(format!("omg-{name}.1"));
        let mut buffer = Vec::new();
        sub_man.render(&mut buffer)?;
        fs::write(&sub_man_path, buffer)
            .with_context(|| format!("Failed to write {}", sub_man_path.display()))?;
        println!("  {} omg-{name}.1", style::success("✓"));
        generated += 1;

        // Generate nested subcommand pages (e.g., omg-env-capture.1)
        for nested in subcommand.get_subcommands() {
            if nested.is_hide_set() {
                continue;
            }

            let nested_name = nested.get_name();
            let nested_man = Man::new(nested.clone());
            let nested_path = output_path.join(format!("omg-{name}-{nested_name}.1"));
            let mut buffer = Vec::new();
            nested_man.render(&mut buffer)?;
            fs::write(&nested_path, buffer)
                .with_context(|| format!("Failed to write {}", nested_path.display()))?;
            println!("  {} omg-{name}-{nested_name}.1", style::success("✓"));
            generated += 1;
        }
    }

    println!();
    println!("{} Generated {} man pages", style::success("✓"), generated);
    println!();
    println!("{} To view: man omg", style::dim("Tip:"));
    println!(
        "{}",
        style::dim("     You may need to run 'mandb' to update the man database.")
    );

    Ok(())
}

/// Simple shell expansion for ~ paths
fn shellexpand(path: &str) -> String {
    if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return path.replacen('~', &home, 1);
        }
    }
    path.to_string()
}
