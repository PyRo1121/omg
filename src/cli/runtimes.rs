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
    versions.get(&runtime.to_lowercase()).cloned().or_else(|| {
        if MISE.is_available() {
            MISE.current_version(runtime).ok().flatten()
        } else {
            None
        }
    })
}

pub fn ensure_active_version(runtime: &str) -> Option<String> {
    resolve_active_version(runtime).or_else(|| {
        if !MISE.is_available() {
            return None;
        }
        if matches!(MISE.install_runtime(runtime), Ok(true)) {
            MISE.current_version(runtime).ok().flatten()
        } else {
            None
        }
    })
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

trait RuntimeInstallUse {
    fn list_installed(&self) -> Result<Vec<String>>;
    fn use_version(&self, version: &str) -> Result<()>;
    async fn install(&self, version: &str) -> Result<()>;
}

macro_rules! impl_runtime_install_use {
    ($($t:ty),+ $(,)?) => {
        $(
            impl RuntimeInstallUse for $t {
                fn list_installed(&self) -> Result<Vec<String>> { self.list_installed() }
                fn use_version(&self, version: &str) -> Result<()> { self.use_version(version) }
                async fn install(&self, version: &str) -> Result<()> { self.install(version).await }
            }
        )+
    };
}

impl_runtime_install_use!(
    NodeManager,
    PythonManager,
    GoManager,
    RubyManager,
    JavaManager,
    BunManager
);

/// Use an already-installed version, or install it first if missing.
async fn install_or_use<M: RuntimeInstallUse + Sync>(mgr: &M, version: &str) -> Result<()> {
    let installed = mgr.list_installed().unwrap_or_default();
    if installed.iter().any(|v| v == version) {
        mgr.use_version(version)?;
    } else {
        mgr.install(version).await?;
    }
    Ok(())
}

pub async fn use_version(runtime: &str, version: Option<&str>) -> Result<()> {
    if !known_runtimes().contains(&runtime.to_string()) {
        anyhow::bail!("Unknown runtime: {runtime}");
    }

    let version = if let Some(v) = version {
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

    ui::print_header("OMG", &format!("Switching {runtime} to version {version}"));
    ui::print_spacer();

    crate::core::usage::track_runtime_switch(runtime);

    match runtime.to_lowercase().as_str() {
        "node" | "nodejs" => {
            install_or_use(&NodeManager::new(), version.trim_start_matches('v')).await?;
        }
        "python" | "python3" => {
            install_or_use(&PythonManager::new(), version.trim_start_matches('v')).await?;
        }
        "rust" => {
            // Rust manager handles toolchains internally; always delegates to install
            RustManager::new().install(&version).await?;
        }
        "go" | "golang" => {
            install_or_use(&GoManager::new(), version.trim_start_matches('v')).await?;
        }
        "ruby" => {
            install_or_use(&RubyManager::new(), version.trim_start_matches('v')).await?;
        }
        "java" | "jdk" | "openjdk" => {
            install_or_use(&JavaManager::new(), &version).await?;
        }
        "bun" | "bunjs" => {
            install_or_use(&BunManager::new(), version.trim_start_matches('v')).await?;
        }
        _ => {
            if !MISE.is_available() {
                println!(
                    "{} {} is not natively supported, installing mise...\n",
                    "→".blue(),
                    runtime.yellow()
                );
                MISE.ensure_installed().await?;
            }
            MISE.use_version(runtime, &version)?;
        }
    }

    Ok(())
}

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
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            tracing::warn!("mise ls failed: {}", stderr);
        }
        // Don't fail - just show nothing instead
        println!("  {} No mise runtimes detected", "-".dimmed());
        return Ok(());
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

fn print_installed_versions(installed: Vec<String>, current: Option<&str>) {
    for v in installed {
        let meta = if current == Some(v.as_str()) {
            Some("(active)")
        } else {
            None
        };
        ui::print_list_item(&v, meta);
    }
}

pub fn list_versions_sync(runtime: Option<&str>) -> Result<()> {
    if let Some(rt) = runtime {
        ui::print_header("OMG", &format!("{rt} versions"));
        ui::print_spacer();

        match rt.to_lowercase().as_str() {
            "node" | "nodejs" => {
                let mgr = NodeManager::new();
                print_installed_versions(
                    mgr.list_installed().unwrap_or_default(),
                    mgr.current_version().as_deref(),
                );
            }
            "python" => {
                let mgr = PythonManager::new();
                print_installed_versions(
                    mgr.list_installed().unwrap_or_default(),
                    mgr.current_version().as_deref(),
                );
            }
            "rust" => {
                let mgr = RustManager::new();
                print_installed_versions(
                    mgr.list_installed().unwrap_or_default(),
                    mgr.current_version().as_deref(),
                );
            }
            "go" | "golang" => {
                let mgr = GoManager::new();
                print_installed_versions(
                    mgr.list_installed().unwrap_or_default(),
                    mgr.current_version().as_deref(),
                );
            }
            "ruby" => {
                let mgr = RubyManager::new();
                print_installed_versions(
                    mgr.list_installed().unwrap_or_default(),
                    mgr.current_version().as_deref(),
                );
            }
            "java" | "jdk" => {
                let mgr = JavaManager::new();
                print_installed_versions(
                    mgr.list_installed().unwrap_or_default(),
                    mgr.current_version().as_deref(),
                );
            }
            "bun" | "bunjs" => {
                let mgr = BunManager::new();
                print_installed_versions(
                    mgr.list_installed().unwrap_or_default(),
                    mgr.current_version().as_deref(),
                );
            }
            _ => {
                mise_list_versions(rt, false)?;
            }
        }
    } else {
        ui::print_header("OMG", "Installed runtime versions");
        ui::print_spacer();

        for (name, mgr_version) in [
            ("Node.js", NodeManager::new().current_version()),
            ("Python", PythonManager::new().current_version()),
            ("Rust", RustManager::new().current_version()),
            ("Go", GoManager::new().current_version()),
            ("Ruby", RubyManager::new().current_version()),
            ("Java", JavaManager::new().current_version()),
            ("Bun", BunManager::new().current_version()),
        ] {
            if let Some(v) = mgr_version {
                ui::print_list_item(name, Some(&v));
            }
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

pub async fn list_versions(runtime: Option<&str>, available: bool) -> Result<()> {
    if !available {
        return list_versions_sync(runtime);
    }

    let Some(rt) = runtime else {
        // List all installed runtimes (parallel probe)
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

        for (name, res) in [
            ("Node.js", node_res),
            ("Python", py_res),
            ("Rust", rust_res),
            ("Go", go_res),
            ("Ruby", ruby_res),
            ("Java", java_res),
            ("Bun", bun_res),
        ] {
            if let Ok(Some(v)) = res {
                ui::print_list_item(name, Some(&v));
            }
        }

        if MISE.is_available() {
            ui::print_spacer();
            ui::print_header("MISE", "Additional Runtimes");
            ui::print_spacer();
            mise_list_all()?;
        }

        ui::print_spacer();
        return Ok(());
    };

    ui::print_header("OMG", &format!("{rt} versions"));
    ui::print_spacer();

    // `!available` already returned above, so all arms here list remote versions
    match rt.to_lowercase().as_str() {
        "node" | "nodejs" => {
            let mgr = NodeManager::new();
            println!("{} Available remote versions:", "→".blue());
            for v in mgr.list_available().await?.iter().take(20) {
                let lts = crate::runtimes::node::get_lts_name(v)
                    .map(|s| format!(" ({})", s.cyan()))
                    .unwrap_or_default();
                ui::print_list_item(&v.version, Some(&lts));
            }
        }
        "python" => {
            let mgr = PythonManager::new();
            println!(
                "{} Available remote versions (python-build-standalone):",
                "→".blue()
            );
            for v in mgr.list_available().await?.iter().take(20) {
                ui::print_list_item(&v.version, None);
            }
        }
        "rust" => {
            let mgr = RustManager::new();
            println!("{} Available remote versions:", "→".blue());
            for v in mgr.list_available().await?.iter().take(20) {
                ui::print_list_item(&v.version, Some(&v.channel));
            }
        }
        "go" | "golang" => {
            let mgr = GoManager::new();
            println!("{} Available remote versions:", "→".blue());
            for v in mgr.list_available().await?.iter().take(20) {
                let stable = if v.stable() { " (stable)" } else { "" };
                ui::print_list_item(v.version(), Some(stable));
            }
        }
        "ruby" => {
            let mgr = RubyManager::new();
            println!("{} Available remote versions (ruby-builder):", "→".blue());
            for v in mgr.list_available().await?.iter().take(20) {
                ui::print_list_item(&v.version, None);
            }
        }
        "java" | "jdk" => {
            let mgr = JavaManager::new();
            println!("{} Available remote versions (Adoptium):", "→".blue());
            for v in mgr.list_available().await?.iter().take(20) {
                let lts = if v.lts { " (LTS)" } else { "" };
                ui::print_list_item(&v.version, Some(lts));
            }
        }
        "bun" | "bunjs" => {
            let mgr = BunManager::new();
            println!("{} Available remote versions:", "→".blue());
            for v in mgr.list_available().await?.iter().take(20) {
                let pre = if v.prerelease { " (pre-release)" } else { "" };
                ui::print_list_item(&v.version, Some(pre));
            }
        }
        _ => {
            if !MISE.is_available() {
                ui::print_tip(&format!(
                    "{rt} is not natively supported, installing mise..."
                ));
                MISE.ensure_installed().await?;
            }
            mise_list_versions(rt, available)?;
        }
    }

    ui::print_spacer();
    Ok(())
}
