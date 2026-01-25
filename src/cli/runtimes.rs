use std::process::Command;
use std::sync::LazyLock;

use anyhow::{Context, Result};
use owo_colors::OwoColorize;

use crate::cli::ui;
use crate::runtimes::{
    BunManager, GoManager, JavaManager, MiseManager, NodeManager, PythonManager, RubyManager,
    RustManager, SUPPORTED_RUNTIMES,
};

/// Global mise manager instance
static MISE: LazyLock<MiseManager> = LazyLock::new(MiseManager::new);

pub fn resolve_active_version(runtime: &str) -> Option<String> {
    let versions = crate::hooks::get_active_versions();

    if let Some(version) = versions.get(&runtime.to_lowercase()) {
        return Some(version.clone());
    }

    if MISE.is_available() {
        return MISE.current_version(runtime).ok().flatten();
    }

    None
}

pub fn ensure_active_version(runtime: &str) -> Option<String> {
    if let Some(version) = resolve_active_version(runtime) {
        return Some(version);
    }

    if !MISE.is_available() {
        return None;
    }

    if matches!(MISE.install_runtime(runtime), Ok(true)) {
        return MISE.current_version(runtime).ok().flatten();
    }

    None
}

pub fn known_runtimes() -> Vec<String> {
    let mut runtimes: Vec<String> = SUPPORTED_RUNTIMES
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

    if MISE.is_available()
        && let Ok(extra) = MISE.list_installed()
    {
        runtimes.extend(extra);
    }

    runtimes.sort();
    runtimes.dedup();
    runtimes
}

/// Switch runtime version
pub async fn use_version(runtime: &str, version: Option<&str>) -> Result<()> {
    // SECURITY: Validate runtime name
    if !known_runtimes().contains(&runtime.to_string()) {
        anyhow::bail!("Unknown runtime: {runtime}");
    }

    // Auto-detect version if not provided
    let version = if let Some(v) = version {
        // SECURITY: Validate version string
        crate::core::security::validate_version(v)?;
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

    ui::print_header("OMG", &format!("Switching {} to version {}", runtime, version));
    ui::print_spacer();

    crate::core::usage::track_runtime_switch(runtime);

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
            // Auto-install mise if not available, then use it for non-native runtimes
            if !MISE.is_available() {
                println!(
                    "{} {} is not natively supported, installing mise...\n",
                    "→".blue(),
                    runtime.yellow()
                );
                // We're already in an async context (use_version is async)
                MISE.ensure_installed().await?;
            }
            MISE.use_version(runtime, &version)?;
        }
    }

    Ok(())
}

// Note: mise_available, mise_current_version, mise_install_runtime, mise_installed_runtimes,
// and mise_use_version are now handled by the MISE manager (MiseManager)

fn mise_list_versions(runtime: &str, available: bool) -> Result<()> {
    let args = if available {
        vec!["ls-remote", "--", runtime]
    } else {
        vec!["ls", "--", runtime]
    };
    let output = Command::new(MISE.mise_path())
        .args(args)
        .output()
        .context("Failed to run `mise`")?;
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
    let output = Command::new(MISE.mise_path())
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
pub fn list_versions_sync(runtime: Option<&str>) -> Result<()> {
    if let Some(rt) = runtime {
        ui::print_header("OMG", &format!("{} versions", rt));
        ui::print_spacer();

        match rt.to_lowercase().as_str() {
            "node" | "nodejs" => {
                let mgr = NodeManager::new();
                let current = mgr.current_version();
                for v in mgr.list_installed().unwrap_or_default() {
                    let is_active = Some(&v) == current.as_ref();
                    let meta = if is_active { Some("(active)") } else { None };
                    ui::print_list_item(&v, meta);
                }
            }
            "python" => {
                let mgr = PythonManager::new();
                let current = mgr.current_version();
                for v in mgr.list_installed().unwrap_or_default() {
                    let is_active = Some(&v) == current.as_ref();
                    let meta = if is_active { Some("(active)") } else { None };
                    ui::print_list_item(&v, meta);
                }
            }
            "rust" => {
                let mgr = RustManager::new();
                let current = mgr.current_version();
                for v in mgr.list_installed().unwrap_or_default() {
                    let is_active = Some(&v) == current.as_ref();
                    let meta = if is_active { Some("(active)") } else { None };
                    ui::print_list_item(&v, meta);
                }
            }
            "go" | "golang" => {
                let mgr = GoManager::new();
                let current = mgr.current_version();
                for v in mgr.list_installed().unwrap_or_default() {
                    let is_active = Some(&v) == current.as_ref();
                    let meta = if is_active { Some("(active)") } else { None };
                    ui::print_list_item(&v, meta);
                }
            }
            "ruby" => {
                let mgr = RubyManager::new();
                let current = mgr.current_version();
                for v in mgr.list_installed().unwrap_or_default() {
                    let is_active = Some(&v) == current.as_ref();
                    let meta = if is_active { Some("(active)") } else { None };
                    ui::print_list_item(&v, meta);
                }
            }
            "java" | "jdk" => {
                let mgr = JavaManager::new();
                let current = mgr.current_version();
                for v in mgr.list_installed().unwrap_or_default() {
                    let is_active = Some(&v) == current.as_ref();
                    let meta = if is_active { Some("(active)") } else { None };
                    ui::print_list_item(&v, meta);
                }
            }
            "bun" | "bunjs" => {
                let mgr = BunManager::new();
                let current = mgr.current_version();
                for v in mgr.list_installed().unwrap_or_default() {
                    let is_active = Some(&v) == current.as_ref();
                    let meta = if is_active { Some("(active)") } else { None };
                    ui::print_list_item(&v, meta);
                }
            }
            _ => {
                // For mise runtimes, we still use Command::new which is sync
                mise_list_versions(rt, false)?;
            }
        }
    } else {
        // List all installed runtimes
        ui::print_header("OMG", "Installed runtime versions");
        ui::print_spacer();

        if let Some(v) = NodeManager::new().current_version() {
            ui::print_list_item("Node.js", Some(&v));
        }
        if let Some(v) = PythonManager::new().current_version() {
            ui::print_list_item("Python", Some(&v));
        }
        if let Some(v) = RustManager::new().current_version() {
            ui::print_list_item("Rust", Some(&v));
        }
        if let Some(v) = GoManager::new().current_version() {
            ui::print_list_item("Go", Some(&v));
        }
        if let Some(v) = RubyManager::new().current_version() {
            ui::print_list_item("Ruby", Some(&v));
        }
        if let Some(v) = JavaManager::new().current_version() {
            ui::print_list_item("Java", Some(&v));
        }
        if let Some(v) = BunManager::new().current_version() {
            ui::print_list_item("Bun", Some(&v));
        }

        if MISE.is_available() {
            ui::print_spacer();
            ui::print_header("MISE", "Additional Runtimes");
            ui::print_spacer();
            mise_list_all()?;
        }
    }

    ui::print_spacer();
    Ok(())
}

/// List installed versions - PURE NATIVE (no external tools)
pub async fn list_versions(runtime: Option<&str>, available: bool) -> Result<()> {
    if !available {
        return list_versions_sync(runtime);
    }

    if let Some(rt) = runtime {
        ui::print_header("OMG", &format!("{} versions", rt));
        ui::print_spacer();

        match rt.to_lowercase().as_str() {
            "node" | "nodejs" => {
                let mgr = NodeManager::new();
                if available {
                    println!("{} Available remote versions:", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        let lts = crate::runtimes::node::get_lts_name(v)
                            .map(|s| format!(" ({})", s.cyan()))
                            .unwrap_or_default();
                        ui::print_list_item(&v.version, Some(&lts));
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let is_active = Some(&v) == current.as_ref();
                        let meta = if is_active { Some("(active)") } else { None };
                        ui::print_list_item(&v, meta);
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
                        ui::print_list_item(&v.version, None);
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let is_active = Some(&v) == current.as_ref();
                        let meta = if is_active { Some("(active)") } else { None };
                        ui::print_list_item(&v, meta);
                    }
                }
            }
            "rust" => {
                let mgr = RustManager::new();
                if available {
                    println!("{} Available remote versions:", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        ui::print_list_item(&v.version, Some(&v.channel));
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let is_active = Some(&v) == current.as_ref();
                        let meta = if is_active { Some("(active)") } else { None };
                        ui::print_list_item(&v, meta);
                    }
                }
            }
            "go" | "golang" => {
                let mgr = GoManager::new();
                if available {
                    println!("{} Available remote versions:", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        let stable = if v.stable { " (stable)" } else { "" };
                        ui::print_list_item(&v.version, Some(stable));
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let is_active = Some(&v) == current.as_ref();
                        let meta = if is_active { Some("(active)") } else { None };
                        ui::print_list_item(&v, meta);
                    }
                }
            }
            "ruby" => {
                let mgr = RubyManager::new();
                if available {
                    println!("{} Available remote versions (ruby-builder):", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        ui::print_list_item(&v.version, None);
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let is_active = Some(&v) == current.as_ref();
                        let meta = if is_active { Some("(active)") } else { None };
                        ui::print_list_item(&v, meta);
                    }
                }
            }
            "java" | "jdk" => {
                let mgr = JavaManager::new();
                if available {
                    println!("{} Available remote versions (Adoptium):", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        let lts = if v.lts { " (LTS)" } else { "" };
                        ui::print_list_item(&v.version, Some(lts));
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let is_active = Some(&v) == current.as_ref();
                        let meta = if is_active { Some("(active)") } else { None };
                        ui::print_list_item(&v, meta);
                    }
                }
            }
            "bun" | "bunjs" => {
                let mgr = BunManager::new();
                if available {
                    println!("{} Available remote versions:", "→".blue());
                    for v in mgr.list_available().await?.iter().take(20) {
                        let pre = if v.prerelease { " (pre-release)" } else { "" };
                        ui::print_list_item(&v.version, Some(pre));
                    }
                } else {
                    let current = mgr.current_version();
                    for v in mgr.list_installed().unwrap_or_default() {
                        let is_active = Some(&v) == current.as_ref();
                        let meta = if is_active { Some("(active)") } else { None };
                        ui::print_list_item(&v, meta);
                    }
                }
            }
            _ => {
                // Auto-install mise if not available
                if !MISE.is_available() {
                    ui::print_tip(&format!("{} is not natively supported, installing mise...", rt));
                    MISE.ensure_installed().await?;
                }
                mise_list_versions(rt, available)?;
            }
        }
    } else {
        // List all installed runtimes
        ui::print_header("OMG", "Installed runtime versions");
        ui::print_spacer();

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
            ui::print_list_item("Node.js", Some(&v));
        }
        if let Ok(Some(v)) = py_res {
            ui::print_list_item("Python", Some(&v));
        }
        if let Ok(Some(v)) = rust_res {
            ui::print_list_item("Rust", Some(&v));
        }
        if let Ok(Some(v)) = go_res {
            ui::print_list_item("Go", Some(&v));
        }
        if let Ok(Some(v)) = ruby_res {
            ui::print_list_item("Ruby", Some(&v));
        }
        if let Ok(Some(v)) = java_res {
            ui::print_list_item("Java", Some(&v));
        }
        if let Ok(Some(v)) = bun_res {
            ui::print_list_item("Bun", Some(&v));
        }

        if MISE.is_available() {
            ui::print_spacer();
            ui::print_header("MISE", "Additional Runtimes");
            ui::print_spacer();
            mise_list_all()?;
        }
    }

    ui::print_spacer();
    Ok(())
}
