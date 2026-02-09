//! Docker-based E2E Tests
//!
//! These tests run actual install/remove operations in Docker containers
//! to verify real system integration without modifying the host.
//!
//! Run with: OMG_RUN_DOCKER_TESTS=1 cargo test --test docker_e2e

use std::process::Command;

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

#[test]
fn test_docker_setup() {
    require_docker_tests();

    assert!(docker_available(), "Docker not available");
    assert!(build_docker_image(), "Failed to build Docker image");
}

#[test]
fn test_docker_omg_search() {
    require_docker_tests();

    let (success, stdout, _stderr) = run_in_docker(&["omg", "search", "vim"]);

    assert!(success, "Search should succeed");
    assert!(stdout.contains("vim"), "Should find vim package");
}

#[test]
fn test_docker_omg_info() {
    require_docker_tests();

    let (success, stdout, _stderr) = run_in_docker(&["omg", "info", "bash"]);

    assert!(success, "Info should succeed");
    assert!(stdout.contains("bash"), "Should show bash package info");
    assert!(
        stdout.contains("Version") || stdout.contains("version"),
        "Should show version"
    );
}

#[test]
fn test_docker_real_install() {
    require_docker_tests();

    // Install a small package (ripgrep is ~2MB)
    let (success, stdout, stderr) = run_in_docker(&["sudo", "omg", "install", "-y", "ripgrep"]);

    if !success {
        eprintln!("STDOUT: {stdout}");
        eprintln!("STDERR: {stderr}");
    }

    assert!(success, "Install should succeed");

    // Verify package is installed
    let (verify_success, verify_out, _) = run_in_docker(&["pacman", "-Qi", "ripgrep"]);

    assert!(verify_success, "Package should be installed");
    assert!(
        verify_out.contains("ripgrep"),
        "Should find installed package"
    );
}

#[test]
fn test_docker_real_remove() {
    require_docker_tests();

    // Install first
    run_in_docker(&["sudo", "omg", "install", "-y", "which"]);

    // Remove it
    let (success, _stdout, _stderr) = run_in_docker(&["sudo", "omg", "remove", "-y", "which"]);

    assert!(success, "Remove should succeed");

    // Verify package is removed
    let (verify_success, _, _) = run_in_docker(&["which", "which"]);

    assert!(!verify_success, "Binary should not exist after removal");
}

#[test]
fn test_docker_update_check() {
    require_docker_tests();

    // Update check should work without sudo
    let (success, _stdout, _stderr) = run_in_docker(&["omg", "update", "--check"]);

    assert!(success, "Update check should succeed");
}

#[test]
fn test_docker_explicit_packages() {
    require_docker_tests();

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

    let (success, _stdout, _stderr) = run_in_docker(&["omg", "status"]);

    assert!(success, "Status command should succeed");
}

#[test]
fn test_docker_performance_vs_pacman() {
    require_docker_tests();

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

    // Run multiple search operations concurrently
    use std::thread;

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

    let (success, _stdout, stderr) = run_in_docker(&["omg", "info", "package-does-not-exist-xyz"]);

    assert!(!success, "Should fail for nonexistent package");
    assert!(
        stderr.contains("not found") || stderr.contains("error"),
        "Should show error message"
    );
}

#[test]
fn test_docker_install_removes_work_together() {
    require_docker_tests();

    // Install
    let (install_success, _, _) = run_in_docker(&["sudo", "omg", "install", "-y", "tree"]);
    assert!(install_success, "Install should succeed");

    // Verify installed
    let (verify1_success, _, _) = run_in_docker(&["which", "tree"]);
    assert!(verify1_success, "Binary should exist");

    // Remove
    let (remove_success, _, _) = run_in_docker(&["sudo", "omg", "remove", "-y", "tree"]);
    assert!(remove_success, "Remove should succeed");

    // Verify removed
    let (verify2_success, _, _) = run_in_docker(&["which", "tree"]);
    assert!(!verify2_success, "Binary should not exist after removal");
}
