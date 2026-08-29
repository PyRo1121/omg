use anyhow::{Context, Result};
use tokio::time::Duration;

use crate::cli::style;
#[cfg(unix)]
use crate::core::client::DaemonClient;
use crate::core::http::shared_client;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_success_non_redirect_mirror_status_is_an_issue() {
        assert!(mirror_status_is_issue(
            reqwest::StatusCode::INTERNAL_SERVER_ERROR
        ));
        assert!(!mirror_status_is_issue(reqwest::StatusCode::OK));
        assert!(!mirror_status_is_issue(
            reqwest::StatusCode::TEMPORARY_REDIRECT
        ));
    }
}

/// Mirror endpoints to test connectivity
const MIRROR_ENDPOINTS: &[(&str, &str)] = &[
    ("Arch Linux", "https://archlinux.org"),
    ("Kernel.org", "https://kernel.org"),
    ("GitHub", "https://github.com"),
    ("AUR", "https://aur.archlinux.org"),
];

fn mirror_status_is_issue(status: reqwest::StatusCode) -> bool {
    !status.is_success() && !status.is_redirection()
}

// EOL data lives in `runtimes::eol` (shared with security.rs).

/// Run all health checks
pub async fn run(network: bool, eol: bool) -> Result<()> {
    println!(
        "{} Checking system health...\n",
        style::header("OMG Doctor")
    );

    let mut issues = 0;

    // 1. OS Check
    if check_os() {
        println!("  {}", style::success("Arch Linux detected"));
    } else {
        println!(
            "  {}",
            style::warning("Non-Arch system detected (some features may fail)")
        );
        issues += 1;
    }

    // 2. Internet Connectivity (basic check)
    if check_internet().await {
        println!("  {}", style::success("Internet connectivity"));
    } else {
        println!("  {}", style::error("No internet connection"));
        issues += 1;
    }

    // 3. Dependencies
    let deps = vec!["git", "curl", "tar", "sudo"];
    for dep in deps {
        if check_command(dep) {
            println!("  {}", style::success(&format!("Found dependency: {dep}")));
        } else {
            println!("  {}", style::error(&format!("Missing dependency: {dep}")));
            issues += 1;
        }
    }

    // 4. Daemon Status
    if check_daemon().await {
        println!("  {}", style::success("Daemon is running"));
    } else {
        println!(
            "  {}",
            style::warning("Daemon is not running (run 'omg daemon')")
        );
        // Not a critical issue
    }

    // 5. PATH Configuration
    if check_path() {
        println!("  {}", style::success("PATH configured correctly"));
    } else {
        println!("  {}", style::error("OMG bin directory not in PATH"));
        issues += 1;
    }

    // 6. Shell Hook
    if check_shell_hook() {
        println!("  {}", style::success("Shell hook active"));
    } else {
        println!(
            "  {}",
            style::warning("Shell hook not found in your login shell's rc file (run 'omg init')")
        );
    }

    // 7. Network diagnostics (if requested)
    if network {
        println!();
        println!("{}", style::header("Network Diagnostics"));
        issues += check_network().await;
    }

    // 8. EOL runtime checks (if requested)
    if eol {
        println!();
        println!("{}", style::header("Runtime EOL Status"));
        issues += check_eol_runtimes()?;
    }

    println!();
    if issues == 0 {
        println!("{}", style::success("System is healthy! Ready to rock."));
    } else {
        println!(
            "{} Found {} issue(s). Please review.",
            style::warning("→"),
            issues
        );
    }

    Ok(())
}

/// Check network connectivity to multiple mirrors
async fn check_network() -> usize {
    let client = shared_client();
    let mut issues = 0;

    for (name, url) in MIRROR_ENDPOINTS {
        let start = std::time::Instant::now();
        let result = tokio::time::timeout(Duration::from_secs(5), client.get(*url).send()).await;

        match result {
            Ok(Ok(response)) => {
                let latency = start.elapsed().as_millis();
                let status = response.status();
                if mirror_status_is_issue(status) {
                    println!(
                        "  {} {} (HTTP {})",
                        style::warning("⚠"),
                        name,
                        status.as_u16()
                    );
                    issues += 1;
                } else {
                    println!("  {} {} ({} ms)", style::success("✓"), name, latency);
                }
            }
            Ok(Err(e)) => {
                println!("  {} {} ({})", style::error("✗"), name, e);
                issues += 1;
            }
            Err(_) => {
                println!("  {} {} (timeout)", style::error("✗"), name);
                issues += 1;
            }
        }
    }

    // DNS resolution test
    println!();
    println!("  {}", style::dim("DNS Resolution:"));
    for host in &["archlinux.org", "aur.archlinux.org", "github.com"] {
        match std::net::ToSocketAddrs::to_socket_addrs(&format!("{host}:443")) {
            Ok(addrs) => {
                let count = addrs.count();
                println!("    {} {} ({} addresses)", style::success("✓"), host, count);
            }
            Err(e) => {
                println!("    {} {} ({})", style::error("✗"), host, e);
                issues += 1;
            }
        }
    }

    issues
}

/// Check for end-of-life runtimes
fn check_eol_runtimes() -> Result<usize> {
    let mut issues = 0;
    let now = jiff::Timestamp::now();
    let warning_ts = crate::runtimes::eol::eol_warning_cutoff(now)
        .context("Failed to compute EOL warning window")?;

    // Get installed runtime versions
    let runtimes = ["node", "python", "rust", "go", "ruby", "java", "bun"];

    for runtime in &runtimes {
        if let Some(version) = crate::runtimes::probe_version(runtime) {
            // Check against EOL dates
            let mut eol_warning = None;

            let components = crate::runtimes::eol::version_components(&version);
            if let Some(entry) = crate::runtimes::eol::find_eol_entry(runtime, &components)
                && let Ok(eol_date) = jiff::civil::Date::strptime("%Y-%m-%d", entry.eol_date)
                && let Ok(zoned) = eol_date.at(0, 0, 0, 0).to_zoned(jiff::tz::TimeZone::UTC)
            {
                let eol_timestamp = zoned.timestamp();
                if now > eol_timestamp {
                    eol_warning = Some(format!("EOL since {}", entry.eol_date));
                } else if warning_ts > eol_timestamp {
                    eol_warning = Some(format!("EOL on {}", entry.eol_date));
                }
            }

            if let Some(warning) = eol_warning {
                println!(
                    "  {} {} {} - {}",
                    style::warning("⚠"),
                    style::runtime(runtime),
                    style::version(&version),
                    style::error(&warning)
                );
                issues += 1;
            } else {
                println!(
                    "  {} {} {}",
                    style::success("✓"),
                    style::runtime(runtime),
                    style::version(&version)
                );
            }
        }
    }

    if issues == 0 {
        println!(
            "  {}",
            style::dim("All runtimes are within support period.")
        );
    }

    Ok(issues)
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

    #[cfg(not(unix))]
    {
        // Daemon not supported on Windows
        return false;
    }

    #[cfg(unix)]
    match DaemonClient::connect().await {
        Ok(_) => true,
        Err(e) => {
            // Provide diagnostic feedback
            let socket_path = crate::core::paths::socket_path();
            if socket_path.exists() {
                // Check if it's a permission issue (common under sudo)
                if let Ok(meta) = std::fs::metadata(&socket_path) {
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
                        println!(
                            "      Hint: The daemon was likely started by a different user. Try restarting it."
                        );
                        return false;
                    }
                }

                println!(
                    "    {} Socket exists at {}, but connection failed: {:#}",
                    style::warning("⚠"),
                    socket_path.display(),
                    e
                );
            } else {
                // Check if we can find it in common locations despite environment
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
            false
        }
    }
}

fn check_path() -> bool {
    if crate::core::paths::test_mode() {
        return true;
    }
    let Some(exe_dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf))
    else {
        return false;
    };
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    // Compare PATH entries as whole components via split_paths; a substring
    // check falsely matched lookalike directories (e.g. /usr/local/bin-backup)
    // and vacuously passed when the exe dir was non-UTF-8.
    // https://doc.rust-lang.org/std/env/fn.split_paths.html
    std::env::split_paths(&path_var).any(|dir| dir == exe_dir)
}

fn check_shell_hook() -> bool {
    if crate::core::paths::test_mode() {
        return true;
    }
    // A doctor subshell cannot inspect live shell functions, so verify the
    // hook the same way `omg init` installs it: $SHELL's rc file contains the
    // hook line. The previous stub was hard-wired to `true`, which made doctor
    // report "Shell hook active" unconditionally — false confidence in a
    // diagnostics tool.
    crate::cli::init::shell_from_env().is_some_and(crate::cli::init::shell_rc_has_hook)
}

/// Enable turbo mode — SECURE REDESIGN (audit F-01, CRITICAL).
///
/// The old implementation ran `sudo setcap` on the omg binary, granting
/// CAP_DAC_OVERRIDE/CAP_FOWNER/CAP_CHOWN to EVERY local account on the
/// machine: any user could execute omg and exercise root-equivalent file
/// power. File capabilities cannot be scoped per-user, so this mode was a
/// privilege-escalation primitive on multi-user systems.
///
/// The replacement keeps the zero-friction goal without permanent privilege:
/// 1. removes any file capabilities previously granted by older versions,
/// 2. relies on sudo's credential cache + omg's sudoloop for near-zero-prompt
///    operation (the same model as yay/paru),
/// 3. prints opt-in NOPASSWD guidance for users who want truly unattended
///    operation and accept the trade-off themselves in /etc/sudoers.
#[cfg(target_os = "linux")]
pub fn enable_turbo_mode() -> Result<()> {
    use owo_colors::OwoColorize;

    let exe = std::env::current_exe()?;
    let exe_path = exe.display();

    crate::cli::modern_ui::print_phase_header("⚡", "TURBO MODE", "Fast package operations");

    // Step 1: strip any capabilities an older omg version may have granted.
    println!(
        "  {} Removing legacy file capabilities from {}...",
        "→".cyan(),
        exe_path
    );
    let remove = std::process::Command::new("sudo")
        .arg("setcap")
        .arg("-r")
        .arg(&exe)
        .status();
    match remove {
        Ok(status) if status.success() => {
            println!(
                "  {} No file capabilities remain (or none were set)",
                "✓".green()
            );
        }
        _ => {
            println!(
                "  {} Could not run `setcap -r` (no caps were set, or sudo unavailable)",
                "ℹ".blue()
            );
        }
    }
    println!();

    // Step 2: warm the sudo credential cache so subsequent operations are
    // prompt-free for the timestamp window; sudoloop keeps it alive during
    // long AUR builds.
    println!("  {} Turbo now means:", "→".cyan());
    println!(
        "    {} Sudo credential caching (sudoloop) — one prompt per session",
        "•".dimmed()
    );
    println!(
        "    {} Native package-manager execution with exact arguments",
        "•".dimmed()
    );
    println!(
        "    {} No permanent privileges granted to any binary",
        "•".dimmed()
    );
    println!();

    // Step 3: optional NOPASSWD guidance for unattended operation.
    if console::user_attended() {
        let user = whoami::username().unwrap_or_else(|_| "username".to_string());
        println!(
            "  {} For fully unattended operation (YOUR choice, affects only you):",
            "ℹ".blue()
        );
        println!("     sudo visudo -f /etc/sudoers.d/omg-turbo",);
        println!(
            "       {user} ALL=(ALL) NOPASSWD: /usr/bin/pacman, /usr/bin/dnf, /usr/bin/apt-get"
        );
        println!();
        println!(
            "  {} File capabilities are NEVER recommended: they grant privileges to\n\
                 every user on the system.",
            "⚠".yellow()
        );
    }

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn enable_turbo_mode() -> Result<()> {
    use owo_colors::OwoColorize;
    println!();
    println!("  {} Turbo mode is only available on Linux", "ℹ".blue());
    println!();
    println!("  Prompt-light sudo credential caching is only available on Linux.");
    println!();
    Ok(())
}
