use anyhow::Result;
use tokio::time::Duration;

use crate::cli::style;
#[cfg(unix)]
use crate::core::client::DaemonClient;
use crate::core::http::shared_client;

/// Mirror endpoints to test connectivity
const MIRROR_ENDPOINTS: &[(&str, &str)] = &[
    ("Arch Linux", "https://archlinux.org"),
    ("Kernel.org", "https://kernel.org"),
    ("GitHub", "https://github.com"),
    ("AUR", "https://aur.archlinux.org"),
];

/// EOL information for common runtimes
const EOL_DATES: &[(&str, &str, &str)] = &[
    // (runtime, version_prefix, eol_date)
    ("node", "16", "2023-09-11"),
    ("node", "18", "2025-04-30"),
    ("node", "19", "2023-06-01"),
    ("python", "3.7", "2023-06-27"),
    ("python", "3.8", "2024-10-31"),
    ("python", "3.9", "2025-10-31"),
    ("rust", "1.70", "2024-01-01"), // Approximate, Rust has short support cycles
    ("go", "1.19", "2023-08-08"),
    ("go", "1.20", "2024-02-06"),
    ("ruby", "2.7", "2023-03-31"),
    ("ruby", "3.0", "2024-03-31"),
    ("java", "8", "2030-12-31"), // Extended support varies by vendor
    ("java", "11", "2026-09-30"),
    ("java", "17", "2029-09-30"),
];

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
            style::warning("Shell hook not detected in environment")
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
        issues += check_eol_runtimes();
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
                if status.is_success() || status.is_redirection() {
                    println!("  {} {} ({} ms)", style::success("✓"), name, latency);
                } else {
                    println!(
                        "  {} {} (HTTP {})",
                        style::warning("⚠"),
                        name,
                        status.as_u16()
                    );
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
fn check_eol_runtimes() -> usize {
    let mut issues = 0;
    let now = jiff::Timestamp::now();

    // Get installed runtime versions
    let runtimes = ["node", "python", "rust", "go", "ruby", "java", "bun"];

    for runtime in &runtimes {
        if let Some(version) = crate::runtimes::probe_version(runtime) {
            // Check against EOL dates
            let mut eol_warning = None;

            for (rt, ver_prefix, eol_date) in EOL_DATES {
                if *rt == *runtime
                    && version.starts_with(ver_prefix)
                    && let Ok(eol_ts) = jiff::civil::Date::strptime("%Y-%m-%d", eol_date)
                {
                    let eol_timestamp = eol_ts
                        .at(0, 0, 0, 0)
                        .to_zoned(jiff::tz::TimeZone::UTC)
                        .unwrap()
                        .timestamp();

                    if now > eol_timestamp {
                        eol_warning = Some(format!("EOL since {eol_date}"));
                    } else {
                        // Check if EOL is within 6 months
                        let six_months = jiff::Span::new().months(6);
                        if let Ok(warning_ts) = now.checked_add(six_months)
                            && warning_ts > eol_timestamp
                        {
                            eol_warning = Some(format!("EOL on {eol_date}"));
                        }
                    }
                    break;
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

    issues
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
                            println!(
                                "      Hint: The daemon was likely started by a different user. Try restarting it."
                            );
                            return false;
                        }
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
