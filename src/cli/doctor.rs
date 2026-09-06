use anyhow::{Context, Result};
use tokio::time::Duration;

use crate::cli::style;
#[cfg(unix)]
use crate::core::client::DaemonClient;
use crate::core::env::distro::{Distro, detect_distro};
use crate::core::http::shared_client;

/// Mirror endpoints to test connectivity.
/// `ARCH_MIRROR_ENDPOINTS` only applies to the Arch backend (W3-A-02).
const ARCH_MIRROR_ENDPOINTS: &[(&str, &str)] = &[
    ("Arch Linux", "https://archlinux.org"),
    ("Kernel.org", "https://kernel.org"),
    ("GitHub", "https://github.com"),
    ("AUR", "https://aur.archlinux.org"),
];

const GENERIC_MIRROR_ENDPOINTS: &[(&str, &str)] = &[
    ("Kernel.org", "https://kernel.org"),
    ("GitHub", "https://github.com"),
];

const ARCH_DNS_HOSTS: &[&str] = &["archlinux.org", "aur.archlinux.org", "github.com"];
const GENERIC_DNS_HOSTS: &[&str] = &["kernel.org", "github.com"];

fn mirror_status_is_issue(status: reqwest::StatusCode) -> bool {
    !status.is_success() && !status.is_redirection()
}

// EOL data lives in `runtimes::eol` (shared with security.rs).

/// Run all health checks
///
/// Exit contract (W3-A-03): `Ok(())` when the system is healthy (0 issues),
/// `Err` when any issue was found, so automation can detect failure — the
/// process exits 0 healthy / 1 on found issues.
pub async fn run(network: bool, eol: bool) -> Result<()> {
    println!(
        "{} Checking system health...\n",
        style::header("OMG Doctor")
    );

    let mut issues = 0;
    let mut warnings = 0;
    let distro = detected_distro();
    let arch_backend = matches!(distro, Distro::Arch);
    let debian_backend = matches!(distro, Distro::Debian | Distro::Ubuntu);

    // 1. OS Check — every supported backend distro is healthy; only an
    //    unsupported system is an issue (W3-A-02: a supported Debian system
    //    must not be reported as permanently unhealthy).
    if let Some(label) = supported_distro_label(distro) {
        println!("  {}", style::success(label));
    } else {
        println!(
            "  {}",
            style::warning("Unsupported system detected (no package-manager backend)")
        );
        issues += 1;
    }

    // 2. Internet Connectivity (basic check)
    if check_internet(distro).await {
        println!("  {}", style::success("Internet connectivity"));
    } else {
        println!("  {}", style::error("No internet connection"));
        issues += 1;
    }

    // 3. Dependencies (backend-appropriate: the live Debian backend shells
    //    out to `apt-get` via the privilege module; Arch uses makepkg for AUR builds)
    let mut deps = vec!["git", "curl", "tar", "sudo"];
    if debian_backend {
        deps.push("apt-get");
    }
    if arch_backend {
        deps.push("makepkg");
    }
    for dep in deps {
        if check_command(dep) {
            println!("  {}", style::success(&format!("Found dependency: {dep}")));
        } else {
            if dep == "makepkg" {
                println!(
                    "  {} Missing dependency: makepkg (install 'base-devel' package for AUR builds)",
                    style::error("✗")
                );
            } else {
                println!("  {}", style::error(&format!("Missing dependency: {dep}")));
            }
            issues += 1;
        }
    }

    // 3b. Backend-specific infrastructure (what the compiled backend itself
    //     reads — no invented checks).
    if debian_backend {
        issues += check_debian_infra();
    }
    if arch_backend {
        issues += check_arch_infra();
    }

    // 4. Daemon Status. A down daemon only limits speed, never
    // correctness, so it warns without failing the run.
    match check_daemon().await {
        DaemonStatus::Running => {
            println!("  {}", style::success("Daemon is running"));
        }
        DaemonStatus::Down => {
            println!(
                "  {}",
                style::warning("Daemon is not running (run 'omg daemon')")
            );
            warnings += 1;
        }
        DaemonStatus::SocketStale => {
            warnings += 1;
        }
    }

    // 5. PATH Configuration
    if check_path() {
        println!("  {}", style::success("PATH configured correctly"));
    } else {
        println!("  {}", style::error("OMG bin directory not in PATH"));
        issues += 1;
    }

    // 6. Shell Hook. A missing hook only costs shell integration,
    // so it warns without failing the run.
    if check_shell_hook() {
        println!("  {}", style::success("Shell hook active"));
    } else {
        println!(
            "  {}",
            style::warning("Shell hook not found in your login shell's rc file (run 'omg init')")
        );
        warnings += 1;
    }

    // 7. Network diagnostics (if requested)
    if network {
        println!();
        println!("{}", style::header("Network Diagnostics"));
        issues += check_network(arch_backend).await;
    }

    // 8. EOL runtime checks (if requested)
    if eol {
        println!();
        println!("{}", style::header("Runtime EOL Status"));
        issues += check_eol_runtimes()?;
    }

    finish_doctor(issues, warnings)
}

/// Print the verdict and select the exit outcome (W3-A-03): healthy runs
/// return `Ok(())` (exit 0); found issues return `Err` so the process exits
/// nonzero (1) and automation can detect failure. Warnings never fail the
/// run, but the verdict names them so a warning never hides behind
/// "healthy".
fn finish_doctor(issues: usize, warnings: usize) -> Result<()> {
    println!();
    if issues == 0 {
        if warnings == 0 {
            println!("{}", style::success("System is healthy! Ready to rock."));
        } else {
            println!(
                "{} System is healthy with {} warning(s).",
                style::success("✓"),
                warnings
            );
        }
        Ok(())
    } else {
        println!(
            "{} Found {} issue(s). Please review.",
            style::warning("→"),
            issues
        );
        Err(anyhow::anyhow!("doctor found {issues} health issue(s)"))
    }
}

/// Distro detected for this doctor run.
///
/// Test mode mirrors the mock-backend default (`arch`) when
/// `OMG_TEST_DISTRO` is unset, so doctor output is hermetic regardless of
/// the host OS.
fn detected_distro() -> Distro {
    if crate::core::paths::test_mode() {
        return std::env::var("OMG_TEST_DISTRO")
            .ok()
            .as_deref()
            .map_or(Distro::Arch, parse_test_distro);
    }
    detect_distro()
}

/// Parse an `OMG_TEST_DISTRO` value (same vocabulary as distro detection).
fn parse_test_distro(value: &str) -> Distro {
    match value.to_lowercase().as_str() {
        "arch" => Distro::Arch,
        "debian" => Distro::Debian,
        "ubuntu" => Distro::Ubuntu,
        "fedora" | "rhel" | "centos" | "rocky" | "alma" => Distro::Fedora,
        "macos" | "darwin" => Distro::MacOS,
        _ => Distro::Unknown,
    }
}

/// Success label for a supported distro (`None` = unsupported system).
fn supported_distro_label(distro: Distro) -> Option<&'static str> {
    match distro {
        Distro::Arch => Some("Arch Linux detected"),
        Distro::Debian | Distro::Ubuntu => Some("Debian/Ubuntu detected (apt backend)"),
        Distro::Fedora => Some("Fedora/RHEL detected (dnf backend)"),
        Distro::MacOS => Some("macOS detected (Homebrew backend)"),
        Distro::Unknown => None,
    }
}

/// Check the Debian/Ubuntu infrastructure the apt backend actually depends
/// on (W3-A-02): the dpkg status database and the APT package indexes that
/// `package_managers::debian_db` parses directly. No other infrastructure is
/// invented; repo reachability for apt is exactly these on-disk indexes.
fn check_debian_infra() -> usize {
    if crate::core::paths::test_mode() {
        // Hermetic like the other checks: report healthy under test mode.
        return 0;
    }

    let mut issues = 0;

    let status = std::path::Path::new("/var/lib/dpkg/status");
    if status.exists() {
        println!(
            "  {}",
            style::success("dpkg package database (/var/lib/dpkg/status)")
        );
    } else {
        println!(
            "  {}",
            style::error("dpkg package database missing (/var/lib/dpkg/status)")
        );
        issues += 1;
    }

    let lists = std::path::Path::new("/var/lib/apt/lists");
    let has_indexes = lists.is_dir()
        && std::fs::read_dir(lists).is_ok_and(|entries| {
            entries
                .filter_map(Result::ok)
                .any(|e| e.file_name().to_string_lossy().ends_with("_Packages"))
        });
    if has_indexes {
        println!(
            "  {}",
            style::success("APT package indexes (/var/lib/apt/lists)")
        );
    } else {
        println!(
            "  {} APT package indexes missing or empty (/var/lib/apt/lists) — run 'sudo apt-get update'",
            style::error("✗")
        );
        issues += 1;
    }

    issues
}

/// Check the Arch Linux infrastructure the ALPM backend depends on:
/// the pacman configuration file (`/etc/pacman.conf`) and the ALPM local
/// package database directory (`/var/lib/pacman/local`).
#[cfg(feature = "arch")]
fn check_arch_infra() -> usize {
    if crate::core::paths::test_mode() {
        return 0;
    }

    let mut issues = 0;

    let conf_path = crate::core::paths::pacman_conf_path();
    let mut db_path: Option<String> = None;
    if conf_path.exists() {
        match crate::core::pacman_conf::PacmanConfig::parse(&conf_path) {
            Ok(config) => {
                println!(
                    "  {} pacman configuration ({}, {} repos configured)",
                    style::success("✓"),
                    conf_path.display(),
                    config.repos.len()
                );
                db_path = config.db_path;
            }
            Err(e) => {
                println!(
                    "  {} invalid pacman configuration ({}): {e}",
                    style::error("✗"),
                    conf_path.display()
                );
                issues += 1;
            }
        }
    } else {
        println!(
            "  {} pacman configuration missing ({})",
            style::error("✗"),
            conf_path.display()
        );
        issues += 1;
    }

    let local_dir = crate::core::paths::pacman_local_dir();
    if local_dir.is_dir() {
        println!(
            "  {} ALPM local package database ({})",
            style::success("✓"),
            local_dir.display()
        );
    } else {
        println!(
            "  {} ALPM local package database missing ({})",
            style::error("✗"),
            local_dir.display()
        );
        issues += 1;
    }

    issues += check_pacman_lock(db_path.as_deref());

    issues
}

/// Check network connectivity to backend-appropriate mirrors
#[cfg(not(feature = "arch"))]
const fn check_arch_infra() -> usize {
    0
}

async fn check_network(arch_backend: bool) -> usize {
    let client = shared_client();
    let mut issues = 0;

    // Arch-only mirrors (archlinux.org, AUR) do not apply to other backends.
    let endpoints: &[(&str, &str)] = if arch_backend {
        ARCH_MIRROR_ENDPOINTS
    } else {
        GENERIC_MIRROR_ENDPOINTS
    };

    for (name, url) in endpoints {
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
    let dns_hosts: &[&str] = if arch_backend {
        ARCH_DNS_HOSTS
    } else {
        GENERIC_DNS_HOSTS
    };
    for host in dns_hosts {
        // A dead resolver blocks ToSocketAddrs forever and would hang the
        // whole doctor run, so resolve off the executor with a hard timeout.
        let lookup = format!("{host}:443");
        let resolved = tokio::time::timeout(
            Duration::from_secs(5),
            tokio::task::spawn_blocking(move || {
                std::net::ToSocketAddrs::to_socket_addrs(lookup.as_str())
                    .map(std::iter::Iterator::count)
            }),
        )
        .await;
        match resolved {
            Ok(Ok(Ok(count))) => {
                println!("    {} {} ({} addresses)", style::success("✓"), host, count);
            }
            Ok(Ok(Err(e))) => {
                println!("    {} {} ({})", style::error("✗"), host, e);
                issues += 1;
            }
            Ok(Err(e)) => {
                println!(
                    "    {} {} (resolver task failed: {e})",
                    style::error("✗"),
                    host
                );
                issues += 1;
            }
            Err(_) => {
                println!("    {} {} (DNS timeout)", style::error("✗"), host);
                issues += 1;
            }
        }
    }

    issues
}

/// A package-manager process holding the ALPM database lock, by binary name.
/// Kept small and exact: anything else holding db.lck is either a wrapper
/// around these or a stale lock from a crashed run.
#[cfg(feature = "arch")]
const DB_LOCK_HOLDERS: &[&str] = &["pacman", "yay", "paru", "pikaur", "omg"];

/// Whether any package-manager process is currently running, by binary
/// name. Shared by the lock check and its test so both agree on liveness.
#[cfg(feature = "arch")]
fn package_manager_running() -> bool {
    std::fs::read_dir("/proc")
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            let name = entry.file_name();
            let Some(pid) = name
                .to_str()
                .filter(|name| name.bytes().all(|b| b.is_ascii_digit()))
            else {
                return false;
            };
            let comm = std::fs::read_to_string(format!("/proc/{pid}/comm"))
                .map(|comm| comm.trim().to_string())
                .unwrap_or_default();
            DB_LOCK_HOLDERS.contains(&comm.as_str())
        })
}

/// Report a stale pacman database lock. A live lock means a manager is
/// mid-transaction and doctor stays quiet; a lock with no manager behind
/// it blocks every future transaction until removed.
#[cfg(feature = "arch")]
fn check_pacman_lock(db_path: Option<&str>) -> usize {
    let lock = std::path::Path::new(db_path.unwrap_or("/var/lib/pacman")).join("db.lck");
    if !lock.exists() {
        return 0;
    }
    if package_manager_running() {
        println!(
            "  {} Database lock held by a running package manager ({})",
            style::success("✓"),
            lock.display()
        );
        return 0;
    }
    println!(
        "  {} Stale database lock with no package manager running ({})",
        style::error("✗"),
        lock.display()
    );
    println!(
        "    {} Remove it: sudo rm {}",
        style::dim("→"),
        lock.display()
    );
    1
}

/// Check for end-of-life runtimes
fn check_eol_runtimes() -> Result<usize> {
    let mut issues = 0;
    let mut probed = 0;
    let now = jiff::Timestamp::now();
    let warning_ts = crate::runtimes::eol::eol_warning_cutoff(now)
        .context("Failed to compute EOL warning window")?;

    // Get installed runtime versions
    let runtimes = [
        "node", "python", "rust", "go", "ruby", "java", "bun", "deno",
    ];

    for runtime in &runtimes {
        if let Some(version) = crate::runtimes::probe_version(runtime) {
            probed += 1;
            // Check against EOL dates. The canonical table is application
            // data, so malformed dates are a defect and must not silently
            // classify an unsupported runtime as healthy.
            let mut eol_warning = None;

            let components = crate::runtimes::eol::version_components(&version);
            if let Some(entry) = crate::runtimes::eol::find_eol_entry(runtime, &components) {
                let eol_date = jiff::civil::Date::strptime("%Y-%m-%d", entry.eol_date)
                    .with_context(|| {
                        format!(
                            "Invalid EOL date {:?} for {runtime} in the canonical runtime table",
                            entry.eol_date
                        )
                    })?;
                let zoned = eol_date
                    .at(0, 0, 0, 0)
                    .to_zoned(jiff::tz::TimeZone::UTC)
                    .context("Failed to convert runtime EOL date to UTC")?;
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

    if probed == 0 {
        println!("  {}", style::dim("No managed runtimes were detected."));
    } else if issues == 0 {
        println!(
            "  {}",
            style::dim("All detected runtimes are within support period.")
        );
    }

    Ok(issues)
}

async fn check_internet(distro: Distro) -> bool {
    if crate::core::paths::test_mode() {
        return true;
    }
    // Backend-appropriate probe target: only the Arch backend has a fixed
    // upstream host (archlinux.org) in this codebase; other backends get
    // their repos from system configuration, so probe GitHub — already a
    // doctor mirror endpoint — as the neutral connectivity target.
    let url = if matches!(distro, Distro::Arch) {
        "https://archlinux.org"
    } else {
        "https://github.com"
    };
    let client = shared_client();
    let request = client.get(url).send();
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

/// Daemon reachability. `Down` means no socket at all; `SocketStale`
/// means a socket file exists but no daemon answers behind it. Both warn
/// without failing the run: the daemon only accelerates reads.
#[derive(Debug, PartialEq, Eq)]
enum DaemonStatus {
    Running,
    Down,
    SocketStale,
}

async fn check_daemon() -> DaemonStatus {
    if crate::core::paths::test_mode() {
        return DaemonStatus::Running;
    }

    #[cfg(not(unix))]
    {
        // Daemon not supported on Windows
        return DaemonStatus::Down;
    }

    #[cfg(unix)]
    match DaemonClient::connect().await {
        Ok(_) => DaemonStatus::Running,
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
                        return DaemonStatus::SocketStale;
                    }
                }

                println!(
                    "    {} Socket exists at {}, but connection failed: {:#}",
                    style::warning("⚠"),
                    socket_path.display(),
                    e
                );
                DaemonStatus::SocketStale
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
                    DaemonStatus::SocketStale
                } else {
                    DaemonStatus::Down
                }
            }
        }
    }
}

fn check_path() -> bool {
    if crate::core::paths::test_mode() {
        return true;
    }
    let Some(path_var) = std::env::var_os("PATH") else {
        return false;
    };
    let paths: Vec<std::path::PathBuf> = std::env::split_paths(&path_var).collect();
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(std::path::Path::to_path_buf));
    let data_bin = crate::core::paths::data_dir().join("bin");
    let home_bin = dirs::home_dir().map(|h| h.join(".local/bin"));

    // Compare PATH entries as whole components via split_paths; a substring
    // check falsely matched lookalike directories (e.g. /usr/local/bin-backup)
    // and vacuously passed when the exe dir was non-UTF-8.
    // Accept if either the running executable directory, OMG's managed data
    // bin directory (~/.local/share/omg/bin), or user bin (~/.local/bin) is in PATH.
    paths.iter().any(|dir| {
        exe_dir.as_ref() == Some(dir) || dir == &data_bin || home_bin.as_ref() == Some(dir)
    })
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
/// 3. explains that package operations retain their normal sudo authorization.
#[cfg(target_os = "linux")]
pub fn enable_turbo_mode() -> Result<()> {
    let exe = std::env::current_exe()?;
    let exe_path = exe.display();

    crate::cli::modern_ui::print_phase_header("⚡", "TURBO MODE", "Fast package operations");

    // Strip capabilities an older omg version may have granted. This runs
    // a privileged command, so ask first in an attended terminal.
    println!(
        "  {} Removing legacy file capabilities from {}...",
        crate::cli::style::accent("→"),
        exe_path
    );
    let cleanup_done = if console::user_attended()
        && !dialoguer::Confirm::new()
            .with_prompt("Run `sudo setcap -r` on the omg binary?")
            .default(true)
            .interact()?
    {
        println!(
            "  {} Skipped capability cleanup",
            crate::cli::style::info("ℹ")
        );
        false
    } else {
        true
    };
    if cleanup_done {
        let remove = crate::core::privilege::system_command("sudo")?
            .arg("setcap")
            .arg("-r")
            .arg(&exe)
            .status();
        match remove {
            Ok(status) if status.success() => {
                println!(
                    "  {} No file capabilities remain (or none were set)",
                    crate::cli::style::positive("✓")
                );
            }
            Ok(status) => {
                println!(
                    "  {} `setcap -r` exited with code {}",
                    crate::cli::style::caution("⚠"),
                    status.code().unwrap_or(-1)
                );
            }
            Err(error) => {
                println!(
                    "  {} Could not run `setcap -r`: {error}",
                    crate::cli::style::caution("⚠")
                );
            }
        }
    }
    println!();

    // Warm the sudo credential cache so subsequent operations are
    // prompt-free for the timestamp window; sudoloop keeps it alive during
    // long AUR builds.
    println!("  {} Turbo now means:", crate::cli::style::accent("→"));
    println!(
        "    {} Sudo credential caching (sudoloop) — one prompt per session",
        crate::cli::style::dim("•")
    );
    println!(
        "    {} Native package-manager execution with exact arguments",
        crate::cli::style::dim("•")
    );
    println!(
        "    {} No permanent privileges granted to any binary",
        crate::cli::style::dim("•")
    );
    println!();

    println!(
        "  Authenticate when prompted. Broad passwordless package-manager rules grant unrestricted root authority."
    );

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn enable_turbo_mode() -> Result<()> {
    println!();
    println!(
        "  {} Turbo mode is only available on Linux",
        crate::cli::style::info("ℹ")
    );
    println!();
    println!("  Prompt-light sudo credential caching is only available on Linux.");
    println!();
    Ok(())
}

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

    /// A lock file with no package manager behind it is stale and counts
    /// as an issue; a missing lock is healthy. The live-manager branch is
    /// not unit-tested: it depends on real process state.
    #[cfg(feature = "arch")]
    #[test]
    fn stale_lock_without_a_manager_is_an_issue() {
        // Host-state dependent: a genuinely running manager means the lock
        // is live, so the stale assertion only runs on quiet machines.
        if package_manager_running() {
            return;
        }
        let dir = tempfile::TempDir::new().expect("isolated db dir");
        let dir_str = dir.path().to_string_lossy().into_owned();
        assert_eq!(check_pacman_lock(Some(&dir_str)), 0);
        std::fs::write(dir.path().join("db.lck"), b"").expect("stale lock");
        assert_eq!(check_pacman_lock(Some(&dir_str)), 1);
    }

    // W3-A-02: every supported backend distro must get a healthy OS verdict;
    // only an unsupported system is an issue.
    #[test]
    fn supported_distros_are_healthy_and_unknown_is_an_issue() {
        assert_eq!(
            supported_distro_label(Distro::Arch),
            Some("Arch Linux detected")
        );
        assert!(supported_distro_label(Distro::Debian).is_some());
        assert!(supported_distro_label(Distro::Ubuntu).is_some());
        assert!(supported_distro_label(Distro::Fedora).is_some());
        assert!(supported_distro_label(Distro::MacOS).is_some());
        assert_eq!(supported_distro_label(Distro::Unknown), None);
    }

    #[test]
    fn test_distro_vocabulary_matches_mock_backend() {
        assert_eq!(parse_test_distro("arch"), Distro::Arch);
        assert_eq!(parse_test_distro("debian"), Distro::Debian);
        assert_eq!(parse_test_distro("ubuntu"), Distro::Ubuntu);
        assert_eq!(parse_test_distro("rhel"), Distro::Fedora);
        assert_eq!(parse_test_distro("darwin"), Distro::MacOS);
        assert_eq!(parse_test_distro("nonsense"), Distro::Unknown);
    }

    // W3-A-03: exit contract — 0 issues is Ok (exit 0); any issue count is
    // Err (exit 1) so automation can detect failure. Warnings never fail.
    #[test]
    fn zero_issues_is_ok_and_found_issues_are_err() {
        assert!(finish_doctor(0, 0).is_ok());
        assert!(finish_doctor(0, 2).is_ok());
        let err = finish_doctor(3, 0).expect_err("issues must produce Err");
        assert!(err.to_string().contains('3'), "err: {err}");
    }
}
