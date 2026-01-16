use std::process::Command;

use anyhow::{Context, Result};
use owo_colors::OwoColorize;

use crate::runtimes::{
    BunManager, GoManager, JavaManager, NodeManager, PythonManager, RubyManager, RustManager,
    SUPPORTED_RUNTIMES,
};

pub fn resolve_active_version(runtime: &str) -> Option<String> {
    let versions = crate::hooks::get_active_versions();
    if let Some(version) = versions.get(&runtime.to_lowercase()) {
        return Some(version.clone());
    }

    if mise_available() {
        return mise_current_version(runtime).ok().flatten();
    }

    None
}

pub fn ensure_active_version(runtime: &str) -> Option<String> {
    if let Some(version) = resolve_active_version(runtime) {
        return Some(version);
    }

    if !mise_available() {
        return None;
    }

    if let Ok(true) = mise_install_runtime(runtime) {
        return mise_current_version(runtime).ok().flatten();
    }

    None
}

pub fn known_runtimes() -> Vec<String> {
    let mut runtimes: Vec<String> = SUPPORTED_RUNTIMES
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

    if mise_available()
        && let Ok(extra) = mise_installed_runtimes()
    {
        runtimes.extend(extra);
    }

    runtimes.sort();
    runtimes.dedup();
    runtimes
}

/// Switch runtime version
pub async fn use_version(runtime: &str, version: Option<&str>) -> Result<()> {
    // Auto-detect version if not provided
    let version = if let Some(v) = version {
        v.to_string()
    } else {
        let active = crate::hooks::get_active_versions();
        if let Some(v) = active.get(&runtime.to_lowercase()) {
            println!("{} Detected version {} from file", "→".blue(), v.yellow());
            v.clone()
        } else {
            anyhow::bail!("No version specified and none detected in .tool-versions, .nvmrc, etc.");
        }
    };

    println!(
        "{} Switching {} to version {}\n",
        "OMG".cyan().bold(),
        runtime.green(),
        version.yellow()
    );

    match runtime.to_lowercase().as_str() {
        "node" | "nodejs" => {
            let node_mgr = NodeManager::new();

            // Check if version is installed
            let installed = node_mgr.list_installed().unwrap_or_default();
            let version_normalized = version.trim_start_matches('v');

            if installed.iter().any(|v| v == version_normalized) {
                // Use existing version
                node_mgr.use_version(version_normalized)?;
            } else {
                // Install and use
                node_mgr.install(version_normalized).await?;
            }
        }
        "python" | "python3" => {
            let py_mgr = PythonManager::new();
            let version_normalized = version.trim_start_matches('v');

            let installed = py_mgr.list_installed().unwrap_or_default();
            if installed.iter().any(|v| v == version_normalized) {
                py_mgr.use_version(version_normalized)?;
            } else {
                py_mgr.install(version_normalized).await?;
            }
        }
        "rust" => {
            let rust_mgr = RustManager::new();
            rust_mgr.install(&version).await?;
        }
        "go" | "golang" => {
            let go_mgr = GoManager::new();

            let installed = go_mgr.list_installed().unwrap_or_default();
            let version_normalized = version.trim_start_matches('v');

            if installed.iter().any(|v| v == version_normalized) {
                go_mgr.use_version(version_normalized)?;
            } else {
                go_mgr.install(version_normalized).await?;
            }
        }
        "ruby" => {
            let ruby_mgr = RubyManager::new();

            let installed = ruby_mgr.list_installed().unwrap_or_default();
            let version_normalized = version.trim_start_matches('v');

            if installed.iter().any(|v| v == version_normalized) {
                ruby_mgr.use_version(version_normalized)?;
            } else {
                ruby_mgr.install(version_normalized).await?;
            }
        }
        "java" | "jdk" | "openjdk" => {
            let java_mgr = JavaManager::new();

            let installed = java_mgr.list_installed().unwrap_or_default();

            if installed.iter().any(|v| v == &version) {
                java_mgr.use_version(&version)?;
            } else {
                java_mgr.install(&version).await?;
            }
        }
        "bun" | "bunjs" => {
            let bun_mgr = BunManager::new();

            let installed = bun_mgr.list_installed().unwrap_or_default();
            let version_normalized = version.trim_start_matches('v');

            if installed.iter().any(|v| v == version_normalized) {
                bun_mgr.use_version(version_normalized)?;
            } else {
                bun_mgr.install(version_normalized).await?;
            }
        }
        _ => {
            if mise_available() {
                mise_use_version(runtime, &version)?;
            } else {
                println!("{} Unknown runtime: {}", "✗".red(), runtime);
                println!("  Supported: node, python, rust, go, ruby, java, bun");
            }
        }
    }

    Ok(())
}

fn mise_available() -> bool {
    Command::new("mise")
        .arg("--version")
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

fn mise_current_version(runtime: &str) -> Result<Option<String>> {
    let output = Command::new("mise")
        .args(["current", runtime])
        .output()
        .with_context(|| format!("Failed to run `mise current {runtime}`"))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().find(|line| !line.trim().is_empty());
    let Some(line) = line else {
        return Ok(None);
    };
    let line = line.trim();

    if let Some(rest) = line.strip_prefix(runtime)
        && let Some(version) = rest.split_whitespace().find(|token| !token.is_empty())
    {
        return Ok(Some(version.to_string()));
    }

    if let Some((_, version)) = line.split_once('@') {
        return Ok(Some(version.trim().to_string()));
    }

    Ok(Some(line.to_string()))
}

fn mise_install_runtime(runtime: &str) -> Result<bool> {
    let status = Command::new("mise")
        .args(["install", runtime])
        .status()
        .with_context(|| format!("Failed to run `mise install {runtime}`"))?;

    Ok(status.success())
}

fn mise_installed_runtimes() -> Result<Vec<String>> {
    let output = Command::new("mise")
        .args(["ls"])
        .output()
        .context("Failed to run `mise ls`")?;

    if !output.status.success() {
        anyhow::bail!("mise failed to list installed runtimes");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut runtimes = Vec::new();
    for line in stdout.lines() {
        let runtime = line.split_whitespace().next().unwrap_or_default();
        if !runtime.is_empty() {
            runtimes.push(runtime.to_string());
        }
    }

    runtimes.sort();
    runtimes.dedup();
    Ok(runtimes)
}

fn mise_use_version(runtime: &str, version: &str) -> Result<()> {
    let tool_spec = format!("{runtime}@{version}");
    let install_status = Command::new("mise")
        .args(["install", &tool_spec])
        .status()
        .with_context(|| format!("Failed to run `mise install {tool_spec}`"))?;
    if !install_status.success() {
        anyhow::bail!("mise failed to install {tool_spec}");
    }

    let use_status = Command::new("mise")
        .args(["use", "--local", &tool_spec])
        .status()
        .with_context(|| format!("Failed to run `mise use --local {tool_spec}`"))?;
    if !use_status.success() {
        anyhow::bail!("mise failed to activate {tool_spec}");
    }

    println!("{} Using mise for {} {}", "✓".green(), runtime, version);
    Ok(())
}

fn mise_list_versions(runtime: &str, available: bool) -> Result<()> {
    let args = if available {
        vec!["ls-remote", runtime]
    } else {
        vec!["ls", runtime]
    };
    let output = Command::new("mise")
        .args(&args)
        .output()
        .with_context(|| format!("Failed to run `mise {}`", args.join(" ")))?;
    if !output.status.success() {
        anyhow::bail!("mise failed to list versions for {runtime}");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        println!("  {} No mise versions found for {}", "-".dimmed(), runtime);
    } else {
        for line in stdout.lines() {
            println!("  {line}");
        }
    }
    Ok(())
}

fn mise_list_all() -> Result<()> {
    let output = Command::new("mise")
        .args(["ls"])
        .output()
        .context("Failed to run `mise ls`")?;
    if !output.status.success() {
        anyhow::bail!("mise failed to list installed runtimes");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.trim().is_empty() {
        println!("  {} No mise runtimes installed", "-".dimmed());
    } else {
        for line in stdout.lines() {
            println!("  {line}");
        }
    }
    Ok(())
}

/// List installed versions - PURE NATIVE (no external tools)
pub async fn list_versions(runtime: Option<&str>, available: bool) -> Result<()> {
    if let Some(rt) = runtime {
        println!("{} {} versions:\n", "OMG".cyan().bold(), rt.green());

        match rt.to_lowercase().as_str() {
            "node" | "nodejs" => {
                let mgr = NodeManager::new();
                if available {
                    println!("{} Available remote versions:", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        let lts = crate::runtimes::node::get_lts_name(v)
                            .map(|s| format!(" ({})", s.cyan()))
                            .unwrap_or_default();
                        println!("  {} {}{}", "●".dimmed(), v.version, lts);
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let marker = if Some(&v) == current.as_ref() {
                            "→"
                        } else {
                            " "
                        };
                        println!("  {} {}", marker.green(), v);
                    }
                }
            }
            "python" => {
                let mgr = PythonManager::new();
                if available {
                    println!(
                        "{} Available remote versions (python-build-standalone):",
                        "→".blue()
                    );
                    for v in mgr.list_available().await?.iter().take(20) {
                        println!("  {} {}", "●".dimmed(), v.version);
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let marker = if Some(&v) == current.as_ref() {
                            "→"
                        } else {
                            " "
                        };
                        println!("  {} {}", marker.green(), v);
                    }
                }
            }
            "rust" => {
                let mgr = RustManager::new();
                if available {
                    println!("{} Available remote versions:", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        println!("  {} {} ({})", "●".dimmed(), v.version, v.channel.dimmed());
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let marker = if Some(&v) == current.as_ref() {
                            "→"
                        } else {
                            " "
                        };
                        println!("  {} {}", marker.green(), v);
                    }
                }
            }
            "go" | "golang" => {
                let mgr = GoManager::new();
                if available {
                    println!("{} Available remote versions:", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        let stable = if v.stable { " (stable)" } else { "" };
                        println!("  {} {}{}", "●".dimmed(), v.version, stable.green());
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let marker = if Some(&v) == current.as_ref() {
                            "→"
                        } else {
                            " "
                        };
                        println!("  {} {}", marker.green(), v);
                    }
                }
            }
            "ruby" => {
                let mgr = RubyManager::new();
                if available {
                    println!("{} Available remote versions (ruby-builder):", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        println!("  {} {}", "●".dimmed(), v.version);
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let marker = if Some(&v) == current.as_ref() {
                            "→"
                        } else {
                            " "
                        };
                        println!("  {} {}", marker.green(), v);
                    }
                }
            }
            "java" | "jdk" => {
                let mgr = JavaManager::new();
                if available {
                    println!("{} Available remote versions (Adoptium):", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        let lts = if v.lts { " (LTS)" } else { "" };
                        println!("  {} {}{}", "●".dimmed(), v.version, lts.green());
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let marker = if Some(&v) == current.as_ref() {
                            "→"
                        } else {
                            " "
                        };
                        println!("  {} {}", marker.green(), v);
                    }
                }
            }
            "bun" | "bunjs" => {
                let mgr = BunManager::new();
                if available {
                    println!("{} Available remote versions:", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        let pre = if v.prerelease { " (pre-release)" } else { "" };
                        println!("  {} {}{}", "●".dimmed(), v.version, pre.yellow());
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let marker = if Some(&v) == current.as_ref() {
                            "→"
                        } else {
                            " "
                        };
                        println!("  {} {}", marker.green(), v);
                    }
                }
            }
            _ => {
                if mise_available() {
                    mise_list_versions(rt, available)?;
                } else {
                    println!("  {} Unknown runtime: {}", "✗".red(), rt);
                    println!("  Supported: node, python, rust, go, ruby, java, bun");
                }
            }
        }
    } else {
        // List all installed runtimes
        println!("{} Installed runtime versions:\n", "OMG".cyan().bold());

        let (node_res, py_res, rust_res, go_res, ruby_res, java_res, bun_res) = tokio::join!(
            tokio::task::spawn_blocking(|| NodeManager::new().current_version()),
            tokio::task::spawn_blocking(|| PythonManager::new().current_version()),
            tokio::task::spawn_blocking(|| RustManager::new().current_version()),
            tokio::task::spawn_blocking(|| GoManager::new().current_version()),
            tokio::task::spawn_blocking(|| RubyManager::new().current_version()),
            tokio::task::spawn_blocking(|| JavaManager::new().current_version()),
            tokio::task::spawn_blocking(|| BunManager::new().current_version()),
        );

        if let Ok(Some(v)) = node_res {
            println!("  {} Node.js {}", "●".green(), v);
        }
        if let Ok(Some(v)) = py_res {
            println!("  {} Python {}", "●".green(), v);
        }
        if let Ok(Some(v)) = rust_res {
            println!("  {} Rust {}", "●".green(), v);
        }
        if let Ok(Some(v)) = go_res {
            println!("  {} Go {}", "●".green(), v);
        }
        if let Ok(Some(v)) = ruby_res {
            println!("  {} Ruby {}", "●".green(), v);
        }
        if let Ok(Some(v)) = java_res {
            println!("  {} Java {}", "●".green(), v);
        }
        if let Ok(Some(v)) = bun_res {
            println!("  {} Bun {}", "●".green(), v);
        }

        if mise_available() {
            println!("\n{} Mise runtimes:\n", "OMG".cyan().bold());
            mise_list_all()?;
        }
    }

    Ok(())
}
