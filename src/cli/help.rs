//! Help command module
//!
//! Provides tiered help display to solve command discovery crisis.
//! Shows only essential commands by default, all commands with --all flag.

use clap::CommandFactory;

use crate::cli::args::Cli;

/// Essential commands that new users need most frequently
const ESSENTIAL_COMMANDS: &[&str] = &[
    "search", "install", "remove", "update", "use", "run", "help",
];

/// Show essential help by default, all commands with --all flag
pub fn print_help(cli: &Cli, use_all: bool) -> anyhow::Result<()> {
    if use_all {
        // Show all commands when --all flag is used
        Cli::command().print_help()?;
    } else {
        // Show only essential commands by default for new users
        print_essential_help(cli);
    }

    Ok(())
}

/// Print essential commands help with getting started guidance
fn print_essential_help(_cli: &Cli) {
    println!("🚀 OMG - The Fastest Unified Package Manager");
    println!();

    println!("📖 Essential Commands (Most Common):");
    println!("═══════════════════════════════");
    println!();

    // Show help for essential commands only
    for cmd_name in ESSENTIAL_COMMANDS {
        if let Some(essential_cmd) = get_essential_command_help(cmd_name) {
            println!("{essential_cmd}");
            println!();
        }
    }

    println!("💡 Show all commands with: omg --help --all-commands");
    println!("🔍 Explore interactive TUI with: omg dash");
    println!("📚 Complete documentation: https://github.com/PyRo1121/omg/tree/main/docs");
    println!();

    print_getting_started();
}

/// Get help text for an essential command
fn get_essential_command_help(cmd_name: &str) -> Option<String> {
    match cmd_name {
        "search" => Some("  🔍 search <query>     Search packages (22x faster than pacman)")
            .map(|s| s.to_string() + "\n     Examples: omg search firefox, omg s node"),
        "install" => Some("  📦 install <pkg>      Install packages (auto-detects AUR)")
            .map(|s| s.to_string() + "\n     Examples: omg install firefox, omg i spotify"),
        "remove" => Some("  🗑️ remove <pkg>      Remove packages with dependency cleanup")
            .map(|s| s.to_string() + "\n     Examples: omg remove firefox, omg r node"),
        "update" => Some("  ⬆️ update             Update all packages and runtimes")
            .map(|s| s.to_string() + "\n     Examples: omg update, omg update --check"),
        "use" => Some("  🔄 use <runtime>     Switch runtime versions instantly")
            .map(|s| s.to_string() + "\n     Examples: omg use node 20, omg use python 3.12"),
        "run" => Some("  🏃 run <task>       Run project tasks (auto-detects package.json)")
            .map(|s| s.to_string() + "\n     Examples: omg run build, omg run dev, omg run test"),
        "help" => Some("  ❓ help [--all-commands]      Show help (essential by default)")
            .map(|s| s.to_string() + "\n     Examples: omg help, omg --help --all-commands"),
        _ => None,
    }
}

/// Print getting started guidance
fn print_getting_started() {
    println!("🎯 Quick Start:");
    println!("   1️⃣  omg install firefox    # Install anything");
    println!("   2️⃣  omg use node 20       # Switch runtime");
    println!("   3️⃣  omg run dev          # Run project");
    println!("   4️⃣  omg search <query>    # Find packages");
    println!();

    println!("🔧 Setup:");
    println!("   eval \"$(omg hook zsh)\"  # Add completions");
    println!("   omg sync                # Initialize databases");
    println!();

    println!("⚡ Performance: 6ms searches, 22x faster than pacman");
    println!("📚 Documentation: https://github.com/PyRo1121/omg/tree/main/docs");
}
