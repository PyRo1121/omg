//! Docker-based E2E Tests
//!
//! These tests run actual install/remove operations in Docker containers
//! to verify real system integration without modifying the host.
//!
//! Run with: `OMG_RUN_DOCKER_TESTS=1 cargo test --test docker_e2e`

use std::process::Command;
use std::sync::OnceLock;
use std::thread;

fn require_docker_tests() {
    if std::env::var("OMG_RUN_DOCKER_TESTS") != Ok("1".to_string()) {
        eprintln!("Skipping Docker E2E tests (set OMG_RUN_DOCKER_TESTS=1 to run)");
        std::process::exit(0);
    }
}

fn docker_available() -> bool {
    Command::new("docker")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn build_docker_image() -> bool {
    if !std::path::Path::new("target/release/omg").exists() {
        eprintln!("Binary not found. Run: cargo build --release --features arch");
        return false;
    }

    // Copy binary
    std::fs::copy("target/release/omg", "omg-binary").expect("Failed to copy binary");

    // Build image
    let status = Command::new("docker")
        .args([
            "build",
            "-f",
            "Dockerfile.arch-e2e",
            "-t",
            "omg-arch-e2e",
            ".",
        ])
        .status()
        .expect("Failed to build Docker image");

    // Cleanup
    let _ = std::fs::remove_file("omg-binary");

    status.success()
}

/// Lazily build the Docker image exactly once, regardless of test ordering.
/// Tests run alphabetically, so we can't rely on `test_docker_setup` running first.
static DOCKER_IMAGE_READY: OnceLock<bool> = OnceLock::new();

fn ensure_docker_image() -> bool {
    *DOCKER_IMAGE_READY.get_or_init(|| {
        assert!(docker_available(), "Docker not available");
        build_docker_image()
    })
}

/// Run a single command in a fresh Docker container
fn run_in_docker(cmd: &[&str]) -> (bool, String, String) {
    let output = Command::new("docker")
        .args(["run", "--rm", "omg-arch-e2e"])
        .args(cmd)
        .output()
        .expect("Failed to run Docker command");

    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// Run a shell script in a single Docker container (preserves state between commands)
fn run_script_in_docker(script: &str) -> (bool, String, String) {
    let output = Command::new("docker")
        .args(["run", "--rm", "omg-arch-e2e", "sh", "-c", script])
        .output()
        .expect("Failed to run Docker command");

    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

/// Strip ANSI escape codes from text for reliable string matching
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Skip until we find the end of the escape sequence
            for inner in chars.by_ref() {
                if inner.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            result.push(c);
        }
    }
    result
}

#[test]
fn test_docker_setup() {
    require_docker_tests();

    assert!(ensure_docker_image(), "Failed to build Docker image");
}

#[test]
fn test_docker_omg_search() {
    require_docker_tests();
    assert!(ensure_docker_image(), "Docker image not ready");

    let (success, stdout, _stderr) = run_in_docker(&["omg", "search", "vim"]);

    assert!(success, "Search should succeed");
    let plain = strip_ansi(&stdout);
    assert!(plain.contains("vim"), "Should find vim package");
}

#[test]
fn test_docker_omg_info() {
    require_docker_tests();
    assert!(ensure_docker_image(), "Docker image not ready");

    let (success, stdout, _stderr) = run_in_docker(&["omg", "info", "bash"]);

    assert!(success, "Info should succeed");
    let plain = strip_ansi(&stdout);
    assert!(plain.contains("bash"), "Should show bash package info");
    // Output may be compact ("bash 5.3.9-1") or labeled ("Version: 5.3.9-1")
    assert!(
        plain.contains("Version")
            || plain.contains("version")
            || plain.chars().any(|c| c.is_ascii_digit()),
        "Should show version info, got: {plain}"
    );
}

#[test]
fn test_docker_real_install() {
    require_docker_tests();
    assert!(ensure_docker_image(), "Docker image not ready");

    // Install and verify in a single container (state persists within one docker run)
    let (success, stdout, stderr) =
        run_script_in_docker("sudo omg install -y ripgrep && pacman -Qi ripgrep");

    if !success {
        eprintln!("STDOUT: {stdout}");
        eprintln!("STDERR: {stderr}");
    }

    assert!(success, "Install and verify should succeed");
    assert!(
        stdout.contains("ripgrep"),
        "Should find installed package in pacman -Qi output"
    );
}

#[test]
fn test_docker_real_remove() {
    require_docker_tests();
    assert!(ensure_docker_image(), "Docker image not ready");

    // Install, remove, and verify in a single container
    let (success, stdout, stderr) =
        run_script_in_docker("sudo omg install -y tree && sudo omg remove -y tree && ! which tree");

    if !success {
        eprintln!("STDOUT: {stdout}");
        eprintln!("STDERR: {stderr}");
    }

    assert!(
        success,
        "Install, remove, and verify should succeed in single container"
    );
}

#[test]
fn test_docker_update_check() {
    require_docker_tests();
    assert!(ensure_docker_image(), "Docker image not ready");

    // Update check should work without sudo
    let (success, _stdout, _stderr) = run_in_docker(&["omg", "update", "--check"]);

    assert!(success, "Update check should succeed");
}

#[test]
fn test_docker_explicit_packages() {
    require_docker_tests();
    assert!(ensure_docker_image(), "Docker image not ready");

    let (success, stdout, _stderr) = run_in_docker(&["omg", "explicit"]);

    assert!(success, "Explicit command should succeed");
    // Base system should have some explicitly installed packages
    assert!(
        !stdout.trim().is_empty(),
        "Should list some explicit packages"
    );
}

#[test]
fn test_docker_status() {
    require_docker_tests();
    assert!(ensure_docker_image(), "Docker image not ready");

    let (success, _stdout, _stderr) = run_in_docker(&["omg", "status"]);

    assert!(success, "Status command should succeed");
}

#[test]
fn test_docker_performance_vs_pacman() {
    require_docker_tests();
    assert!(ensure_docker_image(), "Docker image not ready");

    // Compare search performance
    let start = std::time::Instant::now();
    let (omg_success, _omg_out, _) = run_in_docker(&["omg", "search", "firefox"]);
    let omg_duration = start.elapsed();

    assert!(omg_success, "OMG search should succeed");

    let start = std::time::Instant::now();
    let (pacman_success, _pacman_out, _) = run_in_docker(&["pacman", "-Ss", "firefox"]);
    let pacman_duration = start.elapsed();

    assert!(pacman_success, "Pacman search should succeed");

    eprintln!("OMG search: {omg_duration:?}");
    eprintln!("Pacman search: {pacman_duration:?}");

    // Note: This is in a container so times may vary, but OMG should be competitive
}

#[test]
fn test_docker_concurrent_operations() {
    require_docker_tests();
    assert!(ensure_docker_image(), "Docker image not ready");

    // Run multiple search operations concurrently
    let handles: Vec<_> = (0..4)
        .map(|i| {
            thread::spawn(move || {
                let query = match i {
                    0 => "vim",
                    1 => "firefox",
                    2 => "git",
                    _ => "bash",
                };
                run_in_docker(&["omg", "search", query])
            })
        })
        .collect();

    for handle in handles {
        let (success, _, _) = handle.join().unwrap();
        assert!(success, "Concurrent search should succeed");
    }
}

#[test]
fn test_docker_nonexistent_package() {
    require_docker_tests();
    assert!(ensure_docker_image(), "Docker image not ready");

    let (_success, stdout, stderr) = run_in_docker(&["omg", "info", "package-does-not-exist-xyz"]);

    // omg info may exit 0 even for not-found packages; check output instead
    let combined = format!("{stdout}{stderr}");
    let plain = strip_ansi(&combined);
    assert!(
        plain.contains("not found") || plain.contains("Not Found") || plain.contains("error"),
        "Should show 'not found' message, got: {plain}"
    );
}

#[test]
fn test_docker_install_removes_work_together() {
    require_docker_tests();
    assert!(ensure_docker_image(), "Docker image not ready");

    // All operations in a single container to preserve state
    let (success, _stdout, _stderr) = run_script_in_docker(
        "sudo omg install -y tree \
         && which tree \
         && sudo omg remove -y tree \
         && ! which tree",
    );

    assert!(
        success,
        "Install, verify, remove, and verify-removed should all succeed"
    );
}
