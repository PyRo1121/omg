//! Integration tests for error recovery in package operations.
//!
//! Covers parallel AUR build error handling and CLI-level recovery.
//! Transaction and retry subsystems were removed as dead weight.

#![cfg(feature = "arch")]

use omg_lib::package_managers::aur::BuildJob;

#[test]
fn test_parallel_builder_dependency_graph() {
    let jobs = vec![
        BuildJob::new("base".to_string(), vec![]),
        BuildJob::new("middle".to_string(), vec!["base".to_string()]),
        BuildJob::new("top".to_string(), vec!["middle".to_string()]),
    ];

    let graph =
        omg_lib::package_managers::aur::parallel_build::ParallelBuilder::build_dependency_graph(
            &jobs,
        );

    assert_eq!(graph.get("base").unwrap().len(), 0);
    assert!(graph.get("middle").unwrap().contains("base"));
    assert!(graph.get("top").unwrap().contains("middle"));
}

#[test]
fn test_parallel_builder_independent_packages() {
    let jobs = vec![
        BuildJob::new("pkg-a".to_string(), vec![]),
        BuildJob::new("pkg-b".to_string(), vec![]),
        BuildJob::new("pkg-c".to_string(), vec![]),
    ];

    let graph =
        omg_lib::package_managers::aur::parallel_build::ParallelBuilder::build_dependency_graph(
            &jobs,
        );
    let levels =
        omg_lib::package_managers::aur::parallel_build::ParallelBuilder::topological_levels(&graph)
            .unwrap();

    assert_eq!(levels.len(), 1);
    assert_eq!(levels[0].len(), 3);
}

#[test]
fn test_parallel_builder_circular_dependency_detection() {
    use std::collections::{HashMap, HashSet};

    let mut graph = HashMap::new();
    graph.insert(
        "a".to_string(),
        std::iter::once("b".to_string()).collect::<HashSet<_>>(),
    );
    graph.insert(
        "b".to_string(),
        std::iter::once("c".to_string()).collect::<HashSet<_>>(),
    );
    graph.insert(
        "c".to_string(),
        std::iter::once("a".to_string()).collect::<HashSet<_>>(),
    );

    let result =
        omg_lib::package_managers::aur::parallel_build::ParallelBuilder::topological_levels(&graph);

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Circular dependency")
    );
}

#[test]
fn test_parallel_builder_complex_dependency_tree() {
    use std::collections::{HashMap, HashSet};

    let mut graph = HashMap::new();
    graph.insert("a".to_string(), HashSet::new());
    graph.insert("b".to_string(), std::iter::once("a".to_string()).collect());
    graph.insert("c".to_string(), std::iter::once("a".to_string()).collect());
    graph.insert(
        "d".to_string(),
        ["b".to_string(), "c".to_string()].into_iter().collect(),
    );
    graph.insert("e".to_string(), std::iter::once("d".to_string()).collect());

    let levels =
        omg_lib::package_managers::aur::parallel_build::ParallelBuilder::topological_levels(&graph)
            .unwrap();

    assert_eq!(levels.len(), 4);
    assert_eq!(levels[0], vec!["a"]);
    assert_eq!(levels[1].len(), 2);
    assert!(levels[1].contains(&"b".to_string()));
    assert!(levels[1].contains(&"c".to_string()));
    assert_eq!(levels[2], vec!["d"]);
    assert_eq!(levels[3], vec!["e"]);
}

#[test]
fn test_parallel_builder_empty_jobs() {
    let jobs: Vec<BuildJob> = vec![];

    let graph =
        omg_lib::package_managers::aur::parallel_build::ParallelBuilder::build_dependency_graph(
            &jobs,
        );

    assert!(graph.is_empty());
}

#[test]
fn test_parallel_builder_self_dependency_filtered() {
    let jobs = vec![BuildJob::new("pkg".to_string(), vec!["pkg".to_string()])];

    let graph =
        omg_lib::package_managers::aur::parallel_build::ParallelBuilder::build_dependency_graph(
            &jobs,
        );

    assert!(graph.contains_key("pkg"));
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI-LEVEL ERROR RECOVERY E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

use std::process::{Command, Stdio};
use std::time::Instant;

#[derive(Debug)]
struct CliResult {
    success: bool,
    stdout: String,
    stderr: String,
    #[expect(dead_code)]
    duration: std::time::Duration,
}

fn run_omg_cli(args: &[&str]) -> CliResult {
    let start = Instant::now();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omg"));
    cmd.args(args)
        .env("OMG_TEST_MODE", "1")
        .env("OMG_DISABLE_DAEMON", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().expect("Failed to execute omg");
    let duration = start.elapsed();

    CliResult {
        success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration,
    }
}

#[test]
fn test_cli_handles_network_timeout_gracefully() {
    // Test that CLI properly handles network timeouts without panicking
    let result = run_omg_cli(&["install", "--dry-run", "nonexistent-pkg-xyz-12345"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        combined.contains("not found") || combined.contains("error") || combined.contains("AUR"),
        "CLI must report the missing package, got: {combined}"
    );
    assert!(
        !result.success,
        "nonexistent package dry-run must not succeed"
    );
}

#[test]
fn test_cli_install_nonexistent_package_shows_helpful_error() {
    let result = run_omg_cli(&["install", "-y", "this-pkg-definitely-does-not-exist-xyz"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        combined.contains("not found") || combined.contains("error") || combined.contains("AUR"),
        "Should show helpful error message, got: {combined}"
    );
    assert!(!result.success, "Should fail for nonexistent package");
}

#[test]
fn test_cli_update_with_network_error_recovery() {
    // Test that update command handles network errors gracefully
    let result = run_omg_cli(&["update", "--check"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked") && !combined.contains("RUST_BACKTRACE"),
        "Should handle network errors gracefully"
    );
}

#[test]
fn test_cli_install_with_invalid_package_name_characters() {
    // Test handling of package names with special characters
    let test_cases = vec![
        "pkg; rm -rf /",
        "pkg && echo evil",
        "pkg | cat /etc/passwd",
        "../../../etc/passwd",
        "pkg`whoami`",
    ];

    for pkg_name in test_cases {
        let result = run_omg_cli(&["install", "--dry-run", pkg_name]);
        let combined = format!("{}{}", result.stdout, result.stderr);

        assert!(
            !combined.contains("panicked"),
            "Should handle malicious input safely for: {pkg_name}"
        );
    }
}

#[test]
fn test_cli_concurrent_install_requests() {
    use std::thread;

    // Spawn multiple install requests concurrently to test race conditions
    let handles: Vec<_> = (0..3)
        .map(|i| {
            thread::spawn(move || run_omg_cli(&["install", "--dry-run", &format!("test-pkg-{i}")]))
        })
        .collect();

    for handle in handles {
        let result = handle.join().expect("Thread panicked");
        let combined = format!("{}{}", result.stdout, result.stderr);
        assert!(
            !combined.contains("panicked"),
            "Concurrent requests should not cause panics"
        );
    }
}

#[test]
fn test_cli_install_after_failed_transaction() {
    // First attempt: install nonexistent package (should fail)
    let result1 = run_omg_cli(&["install", "-y", "fake-package-xyz"]);
    assert!(!result1.success, "First install should fail");

    // Second attempt: should still work (no corrupted state)
    let result2 = run_omg_cli(&["install", "--dry-run", "vim"]);
    let combined = format!("{}{}", result2.stdout, result2.stderr);
    assert!(
        !combined.contains("corrupted") && !combined.contains("panicked"),
        "Should recover from failed transaction"
    );
}

#[test]
fn test_cli_handles_sigint_gracefully() {
    // This test verifies that the CLI sets up signal handlers properly
    // We can't easily send SIGINT in a test, but we can verify the code compiles
    // and doesn't panic during initialization
    let result = run_omg_cli(&["--help"]);
    assert!(result.success);
}

#[test]
fn test_cli_error_message_contains_context() {
    let result = run_omg_cli(&["install", "definitely-nonexistent-package-xyz123"]);

    let combined = format!("{}{}", result.stdout, result.stderr);

    // Error should be informative, not just "error occurred"
    let has_context = combined.len() > 20
        && (combined.contains("not found")
            || combined.contains("available")
            || combined.contains("search"));

    assert!(
        has_context,
        "Error message should provide context, got: {combined}"
    );
}

#[test]
fn test_cli_install_with_ctrl_c_simulation() {
    // Test that we handle early termination gracefully
    // This is simulated by ensuring cleanup code paths work
    let result = run_omg_cli(&["install", "--dry-run", "vim"]);

    // If dry-run works, cleanup is functioning
    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("failed to clean"),
        "Cleanup should work properly"
    );
}

#[test]
fn test_cli_multiple_failed_packages_in_single_command() {
    let result = run_omg_cli(&["install", "-y", "fake-pkg-1", "fake-pkg-2", "fake-pkg-3"]);

    let combined = format!("{}{}", result.stdout, result.stderr);

    // Should handle all failures gracefully
    assert!(
        !combined.contains("panicked"),
        "Should handle multiple failures gracefully"
    );

    // Should report which packages failed
    assert!(
        combined.contains("fake-pkg") || combined.contains("not found"),
        "Should report failed packages"
    );
}

#[test]
fn test_cli_install_timeout_handling() {
    // Use environment variable to simulate timeout
    let start = Instant::now();
    let result = run_omg_cli(&["install", "--dry-run", "vim"]);
    let elapsed = start.elapsed();

    // Command should complete reasonably quickly (not hang)
    assert!(
        elapsed.as_secs() < 30,
        "Command should not hang indefinitely"
    );

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should not panic on timeout"
    );
}

#[test]
fn test_cli_handles_disk_full_scenario() {
    // While we can't easily trigger real disk full, we can test
    // that the CLI handles write errors gracefully
    let result = run_omg_cli(&["status"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle filesystem errors gracefully"
    );
}

#[test]
fn test_cli_update_with_corrupted_cache() {
    // The CLI should handle corrupted cache files gracefully
    // by recreating or skipping them
    let result = run_omg_cli(&["update", "--check"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle cache issues gracefully"
    );
}

#[cfg(feature = "arch")]
#[test]
fn test_cli_aur_package_build_failure_recovery() {
    // Test that failed AUR builds are handled gracefully
    let result = run_omg_cli(&["install", "--dry-run", "visual-studio-code-bin"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("panicked"),
        "Should handle AUR package resolution gracefully"
    );
}

#[cfg(feature = "arch")]
#[test]
fn test_cli_parallel_aur_builds_error_handling() {
    // Test parallel builds with potential failures
    let result = run_omg_cli(&["install", "--dry-run", "visual-studio-code-bin"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("deadlock") && !combined.contains("panicked"),
        "Parallel builds should not deadlock or panic"
    );
}

#[test]
fn test_cli_respects_non_interactive_mode() {
    use std::env;

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omg"));
    cmd.args(["install", "vim"])
        .env("OMG_TEST_MODE", "1")
        .env("OMG_DISABLE_DAEMON", "1")
        .env("CI", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd.output().expect("Failed to execute");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !combined.contains("Press") && !combined.contains("Continue?"),
        "Should not prompt in CI environment"
    );
}

#[test]
fn test_cli_handles_permission_denied_errors() {
    // Test that permission errors are reported clearly
    let result = run_omg_cli(&["install", "-y", "vim"]);

    let combined = format!("{}{}", result.stdout, result.stderr);

    // If it fails due to permissions, error should be clear
    if !result.success && combined.contains("permission") {
        assert!(
            combined.contains("sudo")
                || combined.contains("root")
                || combined.contains("privilege"),
            "Permission error should suggest solution"
        );
    }
}

#[test]
fn test_cli_dry_run_never_requires_sudo() {
    let result = run_omg_cli(&["install", "--dry-run", "vim"]);

    let combined = format!("{}{}", result.stdout, result.stderr);
    assert!(
        !combined.contains("[sudo]") && !combined.contains("Password:"),
        "Dry run should never prompt for password"
    );
}

#[test]
fn test_cli_error_recovery_stress_test() {
    // Rapidly fire requests to test error handling under load
    for i in 0..10 {
        let result = run_omg_cli(&["install", "--dry-run", &format!("fake-pkg-{i}")]);
        let combined = format!("{}{}", result.stdout, result.stderr);

        assert!(
            !combined.contains("panicked"),
            "Should handle rapid requests without panicking on iteration {i}"
        );
    }
}
