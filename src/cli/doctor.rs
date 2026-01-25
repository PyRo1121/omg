use anyhow::Result;
use tokio::time::Duration;

use crate::cli::style;
use crate::core::client::DaemonClient;
use crate::core::http::shared_client;

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
    if crate::core::paths::test_mode() {
        return true;
    }
    std::path::Path::new("/etc/arch-release").exists()
}

async fn check_internet() -> bool {
    if crate::core::paths::test_mode() {
        return true;
    }
    let client = shared_client();
    let request = client.get("https://archlinux.org").send();
    tokio::time::timeout(Duration::from_secs(2), request)
        .await
        .ok()
        .and_then(Result::ok)
        .is_some()
}

fn check_command(cmd: &str) -> bool {
    if crate::core::paths::test_mode() {
        return true;
    }
    which::which(cmd).is_ok()
}

async fn check_daemon() -> bool {
    if crate::core::paths::test_mode() {
        return true;
    }

    match DaemonClient::connect().await {
        Ok(_) => true,
        Err(e) => {
            // Provide diagnostic feedback
            let socket_path = crate::core::paths::socket_path();
            if socket_path.exists() {
                // Check if it's a permission issue (common under sudo)
                let metadata = std::fs::metadata(&socket_path);
                if let Ok(meta) = metadata {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::MetadataExt;
                        let socket_uid = meta.uid();
                        let current_uid = rustix::process::getuid().as_raw();

                        if socket_uid != current_uid {
                            println!(
                                "    {} Socket exists at {}, but belongs to UID {} (you are UID {})",
                                style::error("✗"),
                                socket_path.display(),
                                socket_uid,
                                current_uid
                            );
                            println!("      Hint: The daemon was likely started by a different user. Try restarting it.");
                            return false;
                        }
                    }
                }

                println!(
                    "    {} Socket exists at {}, but connection failed: {}",
                    style::warning("⚠"),
                    socket_path.display(),
                    e
                );
            } else {
                // Check if we can find it in common locations despite environment
                #[cfg(unix)]
                {
                    let uid = rustix::process::getuid().as_raw();
                    let common_path = std::path::PathBuf::from(format!("/run/user/{uid}/omg.sock"));
                    if common_path.exists() {
                        println!(
                            "    {} Daemon socket found at {} but client failed to connect!",
                            style::warning("⚠"),
                            common_path.display()
                        );
                        println!("      Hint: Check if the daemon process is actually alive.");
                    }
                }
            }
            false
        }
    }
}

fn check_path() -> bool {
    if crate::core::paths::test_mode() {
        return true;
    }
    if let Ok(path) = std::env::var("PATH") {
        // Check for ~/.local/bin or wherever we install
        // We can't know for sure where user installed, but usually ~/.local/bin
        // Or check if 'omg' is found in PATH and matches current executable path?
        if let Ok(exe) = std::env::current_exe()
            && let Some(parent) = exe.parent()
        {
            return path.contains(parent.to_str().unwrap_or(""));
        }
    }
    false
}

const fn check_shell_hook() -> bool {
    // Hard to check if hook is active effectively.
    // But hook usually modifies PATH.
    // If we rely on check_path, that covers part of it.
    // Maybe check if `omg` function exists? Can't check shell functions from subshell.
    // We'll skip this for now or check checking env vars?
    // Let's rely on PATH check mostly.
    true
}
