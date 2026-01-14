//! Shell completion generation for all shells

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap_complete::{generate, Shell};
use colored::Colorize;
use std::fs;
use std::io;

use crate::cli::Cli;

/// Generate and optionally install shell completions
pub fn generate_completions(shell: &str, to_stdout: bool) -> Result<()> {
    if to_stdout {
        // Just print to stdout
        print_completions(shell)?;
    } else {
        // Install to appropriate location
        install_completions(shell)?;
    }
    Ok(())
}

/// Print completions to stdout
fn print_completions(shell: &str) -> Result<()> {
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
                "Unsupported shell: {shell}. Supported: bash, zsh, fish, powershell, elvish"
            );
        }
    }
    Ok(())
}

/// Install completions to the appropriate location
fn install_completions(shell: &str) -> Result<()> {
    let home = dirs::home_dir().context("Could not find home directory")?;

    match shell.to_lowercase().as_str() {
        "bash" => {
            // Install to ~/.local/share/bash-completion/completions/
            let dir = home.join(".local/share/bash-completion/completions");
            fs::create_dir_all(&dir)?;
            let path = dir.join("omg");

            let content = include_str!("completions/bash.sh");
            fs::write(&path, content)?;

            println!(
                "{} Installed bash completions to {}",
                "✓".green(),
                path.display()
            );
            println!();
            println!("  Restart your shell or run:");
            println!(
                "  {}",
                "source ~/.local/share/bash-completion/completions/omg".cyan()
            );
        }
        "zsh" => {
            // Install to ~/.zfunc/_omg (common zsh completions dir)
            let dir = home.join(".zfunc");
            fs::create_dir_all(&dir)?;
            let path = dir.join("_omg");

            let content = include_str!("completions/zsh.zsh");
            fs::write(&path, content)?;

            println!(
                "{} Installed zsh completions to {}",
                "✓".green(),
                path.display()
            );
            println!();
            println!(
                "  Add this to your {} (before compinit):",
                "~/.zshrc".cyan()
            );
            println!("  {}", "fpath=(~/.zfunc $fpath)".yellow());
            println!();
            println!("  Then restart your shell or run:");
            println!("  {}", "autoload -Uz compinit && compinit".cyan());
        }
        "fish" => {
            // Install to ~/.config/fish/completions/
            let dir = home.join(".config/fish/completions");
            fs::create_dir_all(&dir)?;
            let path = dir.join("omg.fish");

            let mut file = fs::File::create(&path)?;
            let mut cmd = Cli::command();
            generate(Shell::Fish, &mut cmd, "omg", &mut file);

            println!(
                "{} Installed fish completions to {}",
                "✓".green(),
                path.display()
            );
            println!();
            println!("  Restart your shell to enable completions.");
        }
        "powershell" | "pwsh" => {
            // Print instructions - PowerShell is complex
            let mut content = Vec::new();
            let mut cmd = Cli::command();
            generate(Shell::PowerShell, &mut cmd, "omg", &mut content);

            let profile_dir = if cfg!(windows) {
                home.join("Documents/WindowsPowerShell")
            } else {
                home.join(".config/powershell")
            };
            fs::create_dir_all(&profile_dir)?;
            let path = profile_dir.join("omg.ps1");
            fs::write(&path, &content)?;

            println!(
                "{} Installed PowerShell completions to {}",
                "✓".green(),
                path.display()
            );
            println!();
            println!("  Add this to your PowerShell profile:");
            println!("  {}", format!(". {}", path.display()).cyan());
        }
        "elvish" => {
            let dir = home.join(".config/elvish/lib");
            fs::create_dir_all(&dir)?;
            let path = dir.join("omg.elv");

            let mut file = fs::File::create(&path)?;
            let mut cmd = Cli::command();
            generate(Shell::Elvish, &mut cmd, "omg", &mut file);

            println!(
                "{} Installed elvish completions to {}",
                "✓".green(),
                path.display()
            );
            println!();
            println!("  Add this to your rc.elv:");
            println!("  {}", "use omg".cyan());
        }
        _ => {
            anyhow::bail!(
                "Unsupported shell: {shell}. Supported: bash, zsh, fish, powershell, elvish"
            );
        }
    }

    Ok(())
}

/// Print completion installation instructions (legacy)
pub fn print_completion_instructions(shell: &str) {
    match shell.to_lowercase().as_str() {
        "bash" => {
            println!("# Add to ~/.bashrc:");
            println!("eval \"$(omg completions bash --stdout)\"");
            println!();
            println!("# Or install system-wide:");
            println!("sudo omg completions bash --stdout > /etc/bash_completion.d/omg");
        }
        "zsh" => {
            println!("# Add to ~/.zshrc (before compinit):");
            println!("eval \"$(omg completions zsh --stdout)\"");
            println!();
            println!("# Or save to fpath:");
            println!("omg completions zsh --stdout > ~/.zfunc/_omg");
        }
        "fish" => {
            println!("# Add to ~/.config/fish/config.fish:");
            println!("omg completions fish --stdout | source");
            println!();
            println!("# Or save to completions dir:");
            println!("omg completions fish --stdout > ~/.config/fish/completions/omg.fish");
        }
        _ => {
            println!("See 'omg completions --help' for supported shells");
        }
    }
}
