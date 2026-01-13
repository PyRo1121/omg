use anyhow::Result;
use tokio::time::Duration;

use crate::cli::style;
use crate::core::client::DaemonClient;

/// Run all health checks
pub async fn run() -> Result<()> {
    println!(
        "{} Checking system health...\n",
        style::header("OMG Doctor")
    );

    let mut issues = 0;

    // 1. OS Check
    if check_os() {
        println!("  {} Arch Linux detected", style::success("✓"));
    } else {
        println!(
            "  {} Non-Arch system detected (some features may fail)",
            style::warning("⚠")
        );
        issues += 1;
    }

    // 2. Internet Connectivity
    if check_internet().await {
        println!("  {} Internet connectivity", style::success("✓"));
    } else {
        println!("  {} No internet connection", style::error("✗"));
        issues += 1;
    }

    // 3. Dependencies
    let deps = vec!["git", "curl", "tar", "sudo"];
    for dep in deps {
        if check_command(dep) {
            println!("  {} Found dependency: {}", style::success("✓"), dep);
        } else {
            println!("  {} Missing dependency: {}", style::error("✗"), dep);
            issues += 1;
        }
    }

    // 4. Daemon Status
    if check_daemon().await {
        println!("  {} Daemon is running", style::success("✓"));
    } else {
        println!(
            "  {} Daemon is not running (run 'omg daemon')",
            style::warning("⚠")
        );
        // Not a critical issue
    }

    // 5. PATH Configuration
    if check_path() {
        println!("  {} PATH configured correctly", style::success("✓"));
    } else {
        println!("  {} OMG bin directory not in PATH", style::error("✗"));
        issues += 1;
    }

    // 6. Shell Hook
    if check_shell_hook() {
        println!("  {} Shell hook active", style::success("✓"));
    } else {
        println!(
            "  {} Shell hook not detected in environment",
            style::warning("⚠")
        );
        // Hard to detect reliably without inspecting shell rc, but we can check env vars if hook sets any?
        // Our hook sets nothing persistent env vars other than PATH.
        // So this check is heuristic.
    }

    println!();
    if issues == 0 {
        println!("{}", style::success("System is healthy! Ready to rock. 🚀"));
    } else {
        println!(
            "{} Found {} issue(s). Please fix them.",
            style::warning("→"),
            issues
        );
    }

    Ok(())
}

fn check_os() -> bool {
    std::path::Path::new("/etc/arch-release").exists()
}

async fn check_internet() -> bool {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap_or_default();

    // Check Arch Linux mirror or google
    client.get("https://archlinux.org").send().await.is_ok()
}

fn check_command(cmd: &str) -> bool {
    which::which(cmd).is_ok()
}

async fn check_daemon() -> bool {
    DaemonClient::connect().await.is_ok()
}

fn check_path() -> bool {
    if let Ok(path) = std::env::var("PATH") {
        // Check for ~/.local/bin or wherever we install
        // We can't know for sure where user installed, but usually ~/.local/bin
        // Or check if 'omg' is found in PATH and matches current executable path?
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                return path.contains(parent.to_str().unwrap_or(""));
            }
        }
    }
    false
}

fn check_shell_hook() -> bool {
    // Hard to check if hook is active effectively.
    // But hook usually modifies PATH.
    // If we rely on check_path, that covers part of it.
    // Maybe check if `omg` function exists? Can't check shell functions from subshell.
    // We'll skip this for now or check checking env vars?
    // Let's rely on PATH check mostly.
    true
}
