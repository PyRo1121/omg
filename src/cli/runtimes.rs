use crate::runtimes::{
    BunManager, GoManager, JavaManager, NodeManager, PythonManager, RubyManager, RustManager,
};
use anyhow::Result;
use colored::Colorize;

/// Switch runtime version
pub async fn use_version(runtime: &str, version: Option<&str>) -> Result<()> {
    // Auto-detect version if not provided
    let version = match version {
        Some(v) => v.to_string(),
        None => {
            let active = crate::hooks::get_active_versions();
            if let Some(v) = active.get(&runtime.to_lowercase()) {
                println!("{} Detected version {} from file", "→".blue(), v.yellow());
                v.clone()
            } else {
                anyhow::bail!(
                    "No version specified and none detected in .tool-versions, .nvmrc, etc."
                );
            }
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
        "bun" => {
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
            println!("{} Unknown runtime: {}", "✗".red(), runtime);
            println!("  Supported: node, python, rust, go, ruby, java, bun");
        }
    }

    Ok(())
}

/// List installed versions - PURE NATIVE (no external tools)
pub async fn list_versions(runtime: Option<&str>, available: bool) -> Result<()> {
    match runtime {
        Some(rt) => {
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
                "bun" => {
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
                    println!("  {} Unknown runtime: {}", "✗".red(), rt);
                    println!("  Supported: node, python, rust, go, ruby, java, bun");
                }
            }
        }
        None => {
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
        }
    }

    Ok(())
}
