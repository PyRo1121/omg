use anyhow::{Context, Result};
use console::user_attended;
use dialoguer::{Select, theme::ColorfulTheme};
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::cli::style;

/// Tool registry - maps common tool names to their optimal installation source
/// Format: (name, source, description, category)
const TOOL_REGISTRY: &[(&str, &str, &str, &str)] = &[
    // System tools (pacman)
    (
        "ripgrep",
        "pacman:ripgrep",
        "Ultra-fast regex search tool",
        "search",
    ),
    ("rg", "pacman:ripgrep", "Alias for ripgrep", "search"),
    ("fd", "pacman:fd", "Fast find alternative", "search"),
    ("fzf", "pacman:fzf", "Fuzzy finder", "search"),
    ("jq", "pacman:jq", "JSON processor", "data"),
    ("yq", "pacman:yq", "YAML processor", "data"),
    ("bat", "pacman:bat", "Cat with syntax highlighting", "files"),
    ("eza", "pacman:eza", "Modern ls replacement", "files"),
    (
        "zoxide",
        "pacman:zoxide",
        "Smarter cd command",
        "navigation",
    ),
    ("delta", "pacman:git-delta", "Better git diffs", "git"),
    ("lazygit", "pacman:lazygit", "Terminal UI for git", "git"),
    (
        "htop",
        "pacman:htop",
        "Interactive process viewer",
        "system",
    ),
    ("btop", "pacman:btop", "Resource monitor", "system"),
    ("dust", "pacman:dust", "Disk usage analyzer", "system"),
    ("duf", "pacman:duf", "Disk usage/free utility", "system"),
    ("procs", "pacman:procs", "Modern ps replacement", "system"),
    (
        "hyperfine",
        "pacman:hyperfine",
        "Command benchmarking",
        "dev",
    ),
    ("tokei", "pacman:tokei", "Code statistics", "dev"),
    ("just", "pacman:just", "Command runner", "dev"),
    ("watchexec", "pacman:watchexec", "File watcher", "dev"),
    // Node.js tools (npm)
    ("tldr", "npm:tldr", "Simplified man pages", "docs"),
    ("serve", "npm:serve", "Static file server", "web"),
    (
        "http-server",
        "npm:http-server",
        "Simple HTTP server",
        "web",
    ),
    ("yarn", "npm:yarn", "Package manager", "node"),
    ("pnpm", "npm:pnpm", "Fast package manager", "node"),
    ("tsx", "npm:tsx", "TypeScript execute", "node"),
    ("nodemon", "npm:nodemon", "Node.js auto-restart", "node"),
    ("prettier", "npm:prettier", "Code formatter", "formatting"),
    ("eslint", "npm:eslint", "JavaScript linter", "linting"),
    (
        "typescript",
        "npm:typescript",
        "TypeScript compiler",
        "node",
    ),
    ("turbo", "npm:turbo", "Monorepo build system", "node"),
    ("vercel", "npm:vercel", "Vercel CLI", "deploy"),
    ("netlify-cli", "npm:netlify-cli", "Netlify CLI", "deploy"),
    (
        "wrangler",
        "npm:wrangler",
        "Cloudflare Workers CLI",
        "deploy",
    ),
    // Rust tools (cargo)
    (
        "cargo-watch",
        "cargo:cargo-watch",
        "Watch and rebuild",
        "rust",
    ),
    (
        "cargo-edit",
        "cargo:cargo-edit",
        "Cargo add/rm/upgrade",
        "rust",
    ),
    (
        "cargo-expand",
        "cargo:cargo-expand",
        "Macro expansion",
        "rust",
    ),
    (
        "cargo-nextest",
        "cargo:cargo-nextest",
        "Fast test runner",
        "rust",
    ),
    (
        "cargo-audit",
        "cargo:cargo-audit",
        "Security audits",
        "rust",
    ),
    (
        "cargo-outdated",
        "cargo:cargo-outdated",
        "Check outdated deps",
        "rust",
    ),
    ("diesel", "cargo:diesel_cli", "Diesel ORM CLI", "rust"),
    ("sqlx", "cargo:sqlx-cli", "SQLx CLI", "rust"),
    ("bacon", "cargo:bacon", "Background code checker", "rust"),
    (
        "sccache",
        "cargo:sccache",
        "Shared compilation cache",
        "rust",
    ),
    // Python tools (pip)
    ("yt-dlp", "pip:yt-dlp", "Video downloader", "media"),
    ("glances", "pip:glances", "System monitor", "system"),
    ("httpie", "pip:httpie", "HTTP client", "web"),
    ("black", "pip:black", "Python formatter", "python"),
    ("ruff", "pip:ruff", "Fast Python linter", "python"),
    ("mypy", "pip:mypy", "Python type checker", "python"),
    ("poetry", "pip:poetry", "Python packaging", "python"),
    ("pipx", "pip:pipx", "Install Python apps", "python"),
    ("rich-cli", "pip:rich-cli", "Rich text in terminal", "cli"),
    // Go tools
    (
        "hey",
        "go:github.com/rakyll/hey",
        "HTTP load generator",
        "web",
    ),
    (
        "dive",
        "go:github.com/wagoodman/dive",
        "Docker image explorer",
        "docker",
    ),
    (
        "lazydocker",
        "go:github.com/jesseduffield/lazydocker",
        "Docker TUI",
        "docker",
    ),
    (
        "glow",
        "go:github.com/charmbracelet/glow",
        "Markdown renderer",
        "docs",
    ),
    ("air", "go:github.com/cosmtrek/air", "Go live reload", "go"),
    (
        "golangci-lint",
        "go:github.com/golangci/golangci-lint/cmd/golangci-lint",
        "Go linter",
        "go",
    ),
];

#[must_use]
pub fn registry_tool_names() -> Vec<String> {
    TOOL_REGISTRY
        .iter()
        .map(|(name, _, _, _)| (*name).to_string())
        .collect()
}

#[must_use]
pub fn installed_tool_names() -> Vec<String> {
    let (tools_dir, _bin_dir) = get_dirs();
    let mut names = Vec::new();

    if let Ok(entries) = fs::read_dir(&tools_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            if let Ok(manager_entries) = fs::read_dir(&path) {
                for tool in manager_entries.flatten() {
                    if let Some(name) = tool.file_name().to_str() {
                        names.push(name.to_string());
                    }
                }
            }
        }
    }

    names.sort();
    names.dedup();
    names
}

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
    if let Some((_, source, desc, _)) = TOOL_REGISTRY.iter().find(|(k, _, _, _)| *k == name) {
        let Some((manager, pkg)) = source.split_once(':') else {
            anyhow::bail!("Invalid registry format for {name}");
        };
        println!(
            "{} Found in registry: {} ({})",
            style::success("✓"),
            style::package(pkg),
            style::info(manager)
        );
        println!("  {} {}", style::dim("→"), desc);
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
            let install_path = install_dir
                .to_str()
                .context("Install directory path contains invalid UTF-8")?;
            let status = Command::new("npm")
                .args(["install", "--prefix", install_path, pkg])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null()) // Silence noisy npm
                .status()?;

            if !status.success() {
                pb.finish_and_clear();
                anyhow::bail!("NPM install of '{pkg}' failed. Try: npm install -g {pkg}");
            }
        }
        "cargo" => {
            // cargo install --root <dir> <pkg>
            let install_path = install_dir
                .to_str()
                .context("Install directory path contains invalid UTF-8")?;
            let status = Command::new("cargo")
                .args(["install", "--root", install_path, pkg])
                .stdout(std::process::Stdio::null()) // Cargo is noisy
                .status()?;

            if !status.success() {
                pb.finish_and_clear();
                anyhow::bail!("Cargo install of '{pkg}' failed. Try: cargo install {pkg}");
            }
        }
        "pip" => {
            // 1. Create venv
            let install_path = install_dir
                .to_str()
                .context("Install directory path contains invalid UTF-8")?;
            let status_venv = Command::new("python")
                .args(["-m", "venv", install_path])
                .status()?;

            if !status_venv.success() {
                pb.finish_and_clear();
                anyhow::bail!("Failed to create python venv at '{install_path}'");
            }

            // 2. Install into venv
            let pip_path = install_dir.join("bin").join("pip");
            let status_install = Command::new(pip_path)
                .args(["install", pkg])
                .stdout(std::process::Stdio::null())
                .status()?;

            if !status_install.success() {
                pb.finish_and_clear();
                anyhow::bail!("Pip install of '{pkg}' failed. Try: pip install {pkg}");
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
                anyhow::bail!("Go install of '{pkg}' failed. Try: go install {target}");
            }
        }
        _ => anyhow::bail!(
            "Unknown package manager '{manager}'. Supported: npm, cargo, pip, go, pacman"
        ),
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
                    if let Ok(meta) = path.metadata()
                        && meta.permissions().mode() & 0o111 == 0
                    {
                        continue; // Not executable
                    }
                }

                let Some(filename) = path.file_name() else {
                    continue;
                };
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
            style::info(&bin_dir.to_string_lossy())
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
                style::package(
                    &path
                        .file_name()
                        .map(|f| f.to_string_lossy())
                        .unwrap_or_default()
                ),
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
        if let Ok(target) = fs::read_link(&path)
            && !target.exists()
        {
            fs::remove_file(&path)?;
            println!(
                "    {} Removed link {}",
                style::error("-"),
                path.file_name()
                    .map(|f| f.to_string_lossy())
                    .unwrap_or_default()
            );
        }
    }

    println!("\n{}", style::success("Removal complete"));
    Ok(())
}

/// Update an installed tool to latest version
pub async fn update(name: &str) -> Result<()> {
    let (tools_dir, bin_dir) = get_dirs();

    if name == "all" {
        println!("{} Updating all tools...\n", style::header("OMG Tool"));
        let installed = installed_tool_names();
        if installed.is_empty() {
            println!("{}", style::dim("No tools installed."));
            return Ok(());
        }
        for tool in installed {
            println!(
                "\n{} Updating {}...",
                style::dim("→"),
                style::package(&tool)
            );
            // Re-install to update
            if let Some((_, source, _, _)) = TOOL_REGISTRY.iter().find(|(k, _, _, _)| *k == tool)
                && let Some((manager, pkg)) = source.split_once(':')
            {
                let _ = install_managed(manager, pkg, &tool, &tools_dir, &bin_dir).await;
            }
        }
        println!("\n{}", style::success("All tools updated!"));
        return Ok(());
    }

    println!(
        "{} Updating tool '{}'...",
        style::header("OMG Tool"),
        style::package(name)
    );

    // Find the tool in registry or installed
    if let Some((_, source, _, _)) = TOOL_REGISTRY.iter().find(|(k, _, _, _)| *k == name) {
        let Some((manager, pkg)) = source.split_once(':') else {
            anyhow::bail!("Invalid registry format for {name}");
        };
        install_managed(manager, pkg, name, &tools_dir, &bin_dir).await?;
        println!("\n{}", style::success("Update complete!"));
    } else {
        anyhow::bail!("Tool '{name}' not found in registry. Cannot determine update source.");
    }

    Ok(())
}

/// Search for tools in the registry
pub fn search(query: &str) -> Result<()> {
    println!(
        "{} Searching for '{}'...\n",
        style::header("OMG Tool"),
        query
    );

    let query_lower = query.to_lowercase();
    let matches: Vec<_> = TOOL_REGISTRY
        .iter()
        .filter(|(name, _, desc, category)| {
            name.to_lowercase().contains(&query_lower)
                || desc.to_lowercase().contains(&query_lower)
                || category.to_lowercase().contains(&query_lower)
        })
        .collect();

    if matches.is_empty() {
        println!("{}", style::dim("No tools found matching your query."));
        println!("\nTry: omg tool registry  # to see all available tools");
        return Ok(());
    }

    println!("  Found {} tools:\n", matches.len());
    for (name, source, desc, category) in matches {
        let manager = source.split(':').next().unwrap_or("unknown");
        println!(
            "  {} {} {}",
            style::package(name),
            style::dim(&format!("[{category}]")),
            style::dim(&format!("via {manager}"))
        );
        println!("    {desc}\n");
    }

    println!("Install with: omg tool install <name>");
    Ok(())
}

/// Show all available tools in the registry
pub fn registry() -> Result<()> {
    println!("{} Tool Registry\n", style::header("OMG"));

    // Group by category
    let mut categories: std::collections::HashMap<&str, Vec<(&str, &str, &str)>> =
        std::collections::HashMap::new();

    for (name, source, desc, category) in TOOL_REGISTRY {
        categories
            .entry(*category)
            .or_default()
            .push((*name, *source, *desc));
    }

    let mut sorted_cats: Vec<_> = categories.keys().collect();
    sorted_cats.sort();

    for category in sorted_cats {
        let tools = &categories[category];
        println!(
            "  {} {}",
            style::info(&format!("[{category}]")),
            style::dim(&format!("({} tools)", tools.len()))
        );
        for (name, source, desc) in tools {
            let manager = source.split(':').next().unwrap_or("?");
            println!(
                "    {} {} - {}",
                style::package(name),
                style::dim(&format!("({manager})")),
                desc
            );
        }
        println!();
    }

    println!("Total: {} tools available", TOOL_REGISTRY.len());
    println!("\nInstall with: omg tool install <name>");
    Ok(())
}
