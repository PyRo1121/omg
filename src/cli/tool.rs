use anyhow::{Context, Result};
use console::user_attended;
use dialoguer::{theme::ColorfulTheme, Select};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::style;

/// Tool registry - maps common tool names to their optimal installation source
const TOOL_REGISTRY: &[(&str, &str)] = &[
    ("ripgrep", "pacman:ripgrep"),
    ("rg", "pacman:ripgrep"),
    ("fd", "pacman:fd"),
    ("jq", "pacman:jq"),
    ("bat", "pacman:bat"),
    ("tldr", "npm:tldr"), // NPM version is often more up to date/standard
    ("serve", "npm:serve"),
    ("http-server", "npm:http-server"),
    ("yarn", "npm:yarn"),
    ("pnpm", "npm:pnpm"),
    ("cargo-watch", "cargo:cargo-watch"),
    ("diesel", "cargo:diesel_cli"),
    ("sqlx", "cargo:sqlx-cli"),
    ("yt-dlp", "pip:yt-dlp"),
    ("glances", "pip:glances"),
    ("httpie", "pip:httpie"),
    ("hey", "go:github.com/rakyll/hey"),
    ("dive", "go:github.com/wagoodman/dive"),
];

/// Base directories
fn get_dirs() -> (PathBuf, PathBuf) {
    let data_dir = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("~/.local/share"))
        .join("omg");
    let tools_dir = data_dir.join("tools");
    let bin_dir = data_dir.join("bin"); // This should be in PATH via omg hook
    (tools_dir, bin_dir)
}

pub async fn install(name: &str) -> Result<()> {
    println!(
        "{} Installing tool '{}'...",
        style::header("OMG Tool"),
        style::package(name)
    );

    let (tools_dir, bin_dir) = get_dirs();
    fs::create_dir_all(&tools_dir)?;
    fs::create_dir_all(&bin_dir)?;

    // 1. Check Registry
    if let Some((_, source)) = TOOL_REGISTRY.iter().find(|(k, _)| *k == name) {
        let (manager, pkg) = source.split_once(':').unwrap();
        println!(
            "{} Found in registry: {} ({})",
            style::success("✓"),
            style::package(pkg),
            style::info(manager)
        );
        return install_managed(manager, pkg, name, &tools_dir, &bin_dir).await;
    }

    // 2. Interactive Fallback
    if !user_attended() {
        anyhow::bail!(
            "Tool '{name}' not in registry. Re-run in an interactive shell to choose a source."
        );
    }
    let choices = vec![
        "Pacman (System)",
        "Cargo (Isolated)",
        "NPM (Isolated)",
        "Pip (Isolated)",
        "Go (Isolated)",
    ];
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt(format!("Tool '{name}' not in registry. Source?"))
        .default(0)
        .items(&choices)
        .interact()?;

    match selection {
        0 => crate::cli::packages::install(&[name.to_string()], false).await,
        1 => install_managed("cargo", name, name, &tools_dir, &bin_dir).await,
        2 => install_managed("npm", name, name, &tools_dir, &bin_dir).await,
        3 => install_managed("pip", name, name, &tools_dir, &bin_dir).await,
        4 => install_managed("go", name, name, &tools_dir, &bin_dir).await,
        _ => Ok(()),
    }
}

async fn install_managed(
    manager: &str,
    pkg: &str,
    tool_name: &str,
    tools_dir: &Path,
    bin_dir: &Path,
) -> Result<()> {
    // Create isolation directory: ~/.local/share/omg/tools/<manager>/<pkg>
    let install_dir = tools_dir.join(manager).join(pkg);
    if install_dir.exists() {
        fs::remove_dir_all(&install_dir)?;
    }
    fs::create_dir_all(&install_dir)?;

    let pb = style::spinner(&format!("Installing {pkg} via {manager}..."));

    match manager {
        "pacman" => {
            // Pacman installs globally, breaks isolation pattern but is preferred for OS tools
            // We just delegate and return
            pb.finish_and_clear();
            return crate::cli::packages::install(&[pkg.to_string()], false).await;
        }
        "npm" => {
            // npm install --prefix <dir> <pkg>
            let status = Command::new("npm")
                .args(["install", "--prefix", install_dir.to_str().unwrap(), pkg])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null()) // Silence noisy npm
                .status()?;

            if !status.success() {
                pb.finish_and_clear();
                anyhow::bail!("NPM install failed");
            }
        }
        "cargo" => {
            // cargo install --root <dir> <pkg>
            let status = Command::new("cargo")
                .args(["install", "--root", install_dir.to_str().unwrap(), pkg])
                .stdout(std::process::Stdio::null()) // Cargo is noisy
                .status()?;

            if !status.success() {
                pb.finish_and_clear();
                anyhow::bail!("Cargo install failed");
            }
        }
        "pip" => {
            // 1. Create venv
            let status_venv = Command::new("python")
                .args(["-m", "venv", install_dir.to_str().unwrap()])
                .status()?;

            if !status_venv.success() {
                pb.finish_and_clear();
                anyhow::bail!("Failed to create python venv");
            }

            // 2. Install into venv
            let pip_path = install_dir.join("bin").join("pip");
            let status_install = Command::new(pip_path)
                .args(["install", pkg])
                .stdout(std::process::Stdio::null())
                .status()?;

            if !status_install.success() {
                pb.finish_and_clear();
                anyhow::bail!("Pip install failed");
            }
        }
        "go" => {
            // GOBIN=<dir>/bin go install <pkg>@latest
            let target = if pkg.contains('@') {
                pkg.to_string()
            } else {
                format!("{pkg}@latest")
            };

            // Go installs to $GOBIN
            let go_bin = install_dir.join("bin");
            fs::create_dir_all(&go_bin)?;

            let status = Command::new("go")
                .arg("install")
                .arg(&target)
                .env("GOBIN", &go_bin)
                .stdout(std::process::Stdio::null())
                .status()?;

            if !status.success() {
                pb.finish_and_clear();
                anyhow::bail!("Go install failed");
            }
        }
        _ => anyhow::bail!("Unknown manager"),
    }

    pb.finish_and_clear();
    println!("  {} Installation successful", style::success("✓"));

    // LINKING PHASE
    link_binaries(&install_dir, bin_dir, tool_name)?;

    Ok(())
}

fn link_binaries(install_dir: &Path, bin_dir: &Path, _tool_name: &str) -> Result<()> {
    println!("  {} Linking binaries...", style::dim("→"));

    // Find binaries in standard locations within the isolated install dir
    // Standard locations: /bin, /node_modules/.bin (npm)

    let mut search_dirs = vec![install_dir.join("bin")];
    search_dirs.push(install_dir.join("node_modules").join(".bin")); // NPM structure

    let mut linked = 0;

    for dir in search_dirs {
        if !dir.exists() {
            continue;
        }

        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                // Check if executable (heuristic)
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Ok(meta) = path.metadata() {
                        if meta.permissions().mode() & 0o111 == 0 {
                            continue; // Not executable
                        }
                    }
                }

                let filename = path.file_name().unwrap();
                let dest = bin_dir.join(filename);

                // Remove existing link
                if dest.exists() || dest.symlink_metadata().is_ok() {
                    fs::remove_file(&dest)?;
                }

                // Create symlink
                symlink(&path, &dest).context("Failed to symlink binary")?;

                // If the tool name matches the binary name, or if we requested a specific tool, print it
                println!(
                    "    {} Linked {}",
                    style::success("+"),
                    filename.to_string_lossy()
                );
                linked += 1;
            }
        }
    }

    if linked == 0 {
        println!("  {} No binaries found to link!", style::warning("⚠"));
        // Heuristic failed?
    } else {
        println!(
            "  {} {} binaries available in {}",
            style::success("✓"),
            linked,
            style::info(bin_dir.to_str().unwrap())
        );
    }

    Ok(())
}

pub fn list() -> Result<()> {
    let (_, bin_dir) = get_dirs();
    if !bin_dir.exists() {
        println!("{}", style::dim("No tools installed via omg tool."));
        return Ok(());
    }

    println!("{} Installed Tools:", style::header("OMG"));

    for entry in fs::read_dir(bin_dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Ok(target) = fs::read_link(&path) {
            println!(
                "  {} {} -> {}",
                style::package(&path.file_name().unwrap().to_string_lossy()),
                style::arrow("points to"),
                style::dim(&target.to_string_lossy())
            );
        }
    }
    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    let (tools_dir, bin_dir) = get_dirs();

    // We need to find which manager installed it.
    // Check tools_dir/{manager}/{name}

    let managers = vec!["cargo", "npm", "pip", "go"];
    let mut found = false;

    for manager in managers {
        let install_path = tools_dir.join(manager).join(name);
        if install_path.exists() {
            println!(
                "{} Removing {} from {}...",
                style::header("OMG"),
                name,
                manager
            );
            fs::remove_dir_all(&install_path)?;
            found = true;
        }
    }

    if !found {
        println!(
            "{}",
            style::error(&format!("Tool '{name}' not found in managed storage"))
        );
        return Ok(());
    }

    // Cleanup symlinks (broken links)
    println!("  {} Cleaning symlinks...", style::dim("→"));
    for entry in fs::read_dir(bin_dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Ok(target) = fs::read_link(&path) {
            if !target.exists() {
                fs::remove_file(&path)?;
                println!(
                    "    {} Removed link {}",
                    style::error("-"),
                    path.file_name().unwrap().to_string_lossy()
                );
            }
        }
    }

    println!("\n{}", style::success("Removal complete"));
    Ok(())
}
