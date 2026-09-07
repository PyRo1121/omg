//! Shell completion generation for all shells

use anyhow::{Context, Result};
use clap::CommandFactory;
use clap_complete::{Shell, generate};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::cli::{Cli, style};

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
            println!("{}", include_str!("completions/fish.fish"));
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
                style::positive("✓"),
                path.display()
            );
            println!();
            println!("  Restart your shell or run:");
            println!(
                "  {}",
                style::accent("source ~/.local/share/bash-completion/completions/omg")
            );
        }
        "zsh" => {
            let zfunc_dir = home.join(".zfunc");
            fs::create_dir_all(&zfunc_dir)?;
            let zfunc_path = zfunc_dir.join("_omg");
            let content = include_str!("completions/zsh.zsh");
            fs::write(&zfunc_path, content)?;

            let omz_path = oh_my_zsh_completions_dir(&home).map(|dir| dir.join("_omg"));
            if let Some(ref path) = omz_path {
                fs::write(path, content)?;
            }

            let loaded = omz_path.as_ref().unwrap_or(&zfunc_path);
            println!(
                "{} Installed zsh completions to {}",
                style::positive("✓"),
                loaded.display()
            );
            println!();
            if omz_path.is_none() {
                println!(
                    "  Add this to your {} (before compinit):",
                    style::accent("~/.zshrc")
                );
                println!("  {}", style::caution("fpath=(~/.zfunc $fpath)"));
                println!();
                println!("  Then restart your shell or run:");
            } else {
                println!("  Restart your shell or run:");
            }
            println!("  {}", style::accent("autoload -Uz compinit && compinit"));
        }
        "fish" => {
            // Install to ~/.config/fish/completions/
            let dir = home.join(".config/fish/completions");
            fs::create_dir_all(&dir)?;
            let path = dir.join("omg.fish");

            let content = include_str!("completions/fish.fish");
            fs::write(&path, content)?;

            println!(
                "{} Installed fish completions to {}",
                style::positive("✓"),
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
                style::positive("✓"),
                path.display()
            );
            println!();
            println!("  Add this to your PowerShell profile:");
            println!("  {}", style::accent(&format!(". {}", path.display())));
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
                style::positive("✓"),
                path.display()
            );
            println!();
            println!("  Add this to your rc.elv:");
            println!("  {}", style::accent("use omg"));
        }
        _ => {
            anyhow::bail!(
                "Unsupported shell: {shell}. Supported: bash, zsh, fish, powershell, elvish"
            );
        }
    }

    Ok(())
}

fn oh_my_zsh_completions_dir(home: &Path) -> Option<PathBuf> {
    let omz_root = std::env::var_os("ZSH")
        .map(PathBuf::from)
        .filter(|zsh| is_oh_my_zsh_layout(zsh))
        .or_else(|| {
            let fallback = home.join(".oh-my-zsh");
            let known = fallback.join("oh-my-zsh.sh").is_file()
                || fallback.join("completions").is_dir();
            known.then_some(fallback)
        })?;
    let completions = omz_root.join("completions");
    fs::create_dir_all(&completions).ok()?;
    Some(completions)
}

fn is_oh_my_zsh_layout(zsh: &Path) -> bool {
    zsh.file_name().is_some_and(|name| name == "oh-my-zsh") || zsh.join("oh-my-zsh.sh").is_file()
}

#[cfg(test)]
#[expect(clippy::unwrap_used)] // Idiomatic in tests: panics on failure with clear error context
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[serial_test::serial]
    #[test]
    fn zsh_install_writes_omz_completions_when_present() {
        let home = tempdir().unwrap();
        let omz = home.path().join(".oh-my-zsh").join("completions");
        fs::create_dir_all(&omz).unwrap();
        let home_str = home.path().to_string_lossy().into_owned();
        let vars: Vec<(&str, Option<&str>)> =
            vec![("HOME", Some(home_str.as_str())), ("ZSH", None)];
        temp_env::with_vars(&vars, || {
            assert_eq!(
                dirs::home_dir().as_deref(),
                Some(home.path()),
                "dirs::home_dir must honor HOME"
            );
            install_completions("zsh").unwrap();
            assert!(omz.join("_omg").is_file());
            assert!(home.path().join(".zfunc").join("_omg").is_file());
        });
    }

    #[serial_test::serial]
    #[test]
    fn zsh_install_creates_omz_completions_dir() {
        let home = tempdir().unwrap();
        let omz = home.path().join(".oh-my-zsh");
        fs::create_dir_all(&omz).unwrap();
        fs::write(omz.join("oh-my-zsh.sh"), "# stub\n").unwrap();
        let home_str = home.path().to_string_lossy().into_owned();
        let vars: Vec<(&str, Option<&str>)> =
            vec![("HOME", Some(home_str.as_str())), ("ZSH", None)];
        temp_env::with_vars(&vars, || {
            install_completions("zsh").unwrap();
            assert!(omz.join("completions").join("_omg").is_file());
        });
    }

    #[serial_test::serial]
    #[test]
    fn zsh_install_writes_zfunc_only_without_omz() {
        let home = tempdir().unwrap();
        let home_str = home.path().to_string_lossy().into_owned();
        let vars: Vec<(&str, Option<&str>)> =
            vec![("HOME", Some(home_str.as_str())), ("ZSH", None)];
        temp_env::with_vars(&vars, || {
            install_completions("zsh").unwrap();
            assert!(home.path().join(".zfunc").join("_omg").is_file());
            assert!(
                !home
                    .path()
                    .join(".oh-my-zsh")
                    .join("completions")
                    .join("_omg")
                    .is_file()
            );
        });
    }
}
