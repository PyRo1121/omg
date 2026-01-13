//! Shell completion generation for all shells

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{generate, Shell};
use std::io;

use crate::cli::Cli;

/// Generate shell completions
pub fn generate_completions(shell: &str) -> Result<()> {
    match shell.to_lowercase().as_str() {
        "bash" => {
            println!("{}", include_str!("completions/bash.sh"));
        }
        "zsh" => {
            println!("{}", include_str!("completions/zsh.zsh"));
        }
        "fish" => {
            let mut cmd = Cli::command();
            generate(Shell::Fish, &mut cmd, "omg", &mut io::stdout());
        }
        "powershell" | "pwsh" => {
            let mut cmd = Cli::command();
            generate(Shell::PowerShell, &mut cmd, "omg", &mut io::stdout());
        }
        "elvish" => {
            let mut cmd = Cli::command();
            generate(Shell::Elvish, &mut cmd, "omg", &mut io::stdout());
        }
        _ => {
            anyhow::bail!(
                "Unsupported shell: {}. Supported: bash, zsh, fish, powershell, elvish",
                shell
            );
        }
    };

    Ok(())
}

/// Print completion installation instructions
pub fn print_completion_instructions(shell: &str) {
    match shell.to_lowercase().as_str() {
        "bash" => {
            println!("# Add to ~/.bashrc:");
            println!("eval \"$(omg completions bash)\"");
            println!();
            println!("# Or install system-wide:");
            println!("sudo omg completions bash > /etc/bash_completion.d/omg");
        }
        "zsh" => {
            println!("# Add to ~/.zshrc (before compinit):");
            println!("eval \"$(omg completions zsh)\"");
            println!();
            println!("# Or save to fpath:");
            println!("omg completions zsh > ~/.zfunc/_omg");
        }
        "fish" => {
            println!("# Add to ~/.config/fish/config.fish:");
            println!("omg completions fish | source");
            println!();
            println!("# Or save to completions dir:");
            println!("omg completions fish > ~/.config/fish/completions/omg.fish");
        }
        _ => {
            println!("See 'omg completions --help' for supported shells");
        }
    }
}
