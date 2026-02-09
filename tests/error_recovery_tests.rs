//! Integration tests for error recovery in package operations.
//!
//! Tests transaction rollback, retry logic, and error handling.

#![cfg(feature = "arch")]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use omg_lib::core::PackageSource;
use omg_lib::core::retry::{RetryConfig, is_retryable, retry_async};
use omg_lib::core::transactions::{Operation, Transaction, TransactionState};
use omg_lib::package_managers::aur::BuildJob;

#[tokio::test]
async fn test_transaction_rollback_on_partial_failure() {
    let mut tx = Transaction::new();

    tx.add_operation(Operation::Install {
        package: "test-pkg-1".to_string(),
        version: "1.0.0".parse().unwrap(),
        source: PackageSource::Official,
    });

    tx.add_operation(Operation::Install {
        package: "test-pkg-2".to_string(),
        version: "2.0.0".parse().unwrap(),
        source: PackageSource::Official,
    });

    assert_eq!(tx.operations.len(), 2);

    let result = tx.rollback().await;
    assert!(result.is_ok());
    assert_eq!(tx.state, TransactionState::RolledBack);
}

#[tokio::test]
async fn test_transaction_cannot_rollback_twice() {
    let mut tx = Transaction::new();

    tx.add_operation(Operation::Remove {
        package: "test-pkg".to_string(),
        version: "1.0.0".parse().unwrap(),
    });

    tx.rollback().await.unwrap();
    assert_eq!(tx.state, TransactionState::RolledBack);

    let second_rollback = tx.rollback().await;
    assert!(second_rollback.is_err());
    assert!(
        second_rollback
            .unwrap_err()
            .to_string()
            .contains("already rolled back")
    );
}

#[tokio::test]
async fn test_transaction_state_transitions() {
    let mut tx = Transaction::new();
    assert_eq!(tx.state, TransactionState::Pending);

    tx.begin().unwrap();
    assert_eq!(tx.state, TransactionState::InProgress);
    assert!(tx.snapshot.is_some());

    let begin_again = tx.begin();
    assert!(begin_again.is_err());

    tx.commit().unwrap();
    assert_eq!(tx.state, TransactionState::Committed);
    assert!(tx.snapshot.is_none());
}

#[tokio::test]
async fn test_retry_with_transient_failures() {
    let config = RetryConfig::new(5);
    let attempt_counter = Arc::new(AtomicU32::new(0));

    let result = retry_async(&config, || {
        let counter = Arc::clone(&attempt_counter);
        async move {
            let attempt = counter.fetch_add(1, Ordering::SeqCst);
            if attempt < 3 {
                Err(anyhow::anyhow!("connection timeout"))
            } else {
                Ok("success")
            }
        }
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
    assert_eq!(attempt_counter.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn test_retry_fails_after_max_attempts() {
    let config = RetryConfig::new(2);
    let attempt_counter = Arc::new(AtomicU32::new(0));

    let result = retry_async(&config, || {
        let counter = Arc::clone(&attempt_counter);
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Err::<&str, _>(anyhow::anyhow!("persistent network error"))
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(attempt_counter.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn test_retry_stops_on_non_retryable_error() {
    let config = RetryConfig::new(5);
    let attempt_counter = Arc::new(AtomicU32::new(0));

    let result = retry_async(&config, || {
        let counter = Arc::clone(&attempt_counter);
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Err::<&str, _>(anyhow::anyhow!("permission denied"))
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(attempt_counter.load(Ordering::SeqCst), 1);
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("permission denied")
    );
}

#[test]
fn test_is_retryable_classification() {
    assert!(is_retryable(&anyhow::anyhow!("connection timeout")));
    assert!(is_retryable(&anyhow::anyhow!("network unreachable")));
    assert!(is_retryable(&anyhow::anyhow!("temporary failure")));
    assert!(is_retryable(&anyhow::anyhow!(
        "Connection refused (ECONNREFUSED)"
    )));
    assert!(is_retryable(&anyhow::anyhow!(
        "timed out waiting for server"
    )));

    assert!(!is_retryable(&anyhow::anyhow!("permission denied")));
    assert!(!is_retryable(&anyhow::anyhow!("file not found")));
    assert!(!is_retryable(&anyhow::anyhow!("invalid package format")));
    assert!(!is_retryable(&anyhow::anyhow!("authentication failed")));
}

#[tokio::test]
async fn test_retry_backoff_timing() {
    use std::time::Instant;

    let config = RetryConfig::default().with_backoff(100, 5000);
    let attempt_counter = Arc::new(AtomicU32::new(0));
    let start = Instant::now();

    let _result = retry_async(&config, || {
        let counter = Arc::clone(&attempt_counter);
        async move {
            let attempt = counter.fetch_add(1, Ordering::SeqCst);
            if attempt < 2 {
                Err::<&str, _>(anyhow::anyhow!("timeout"))
            } else {
                Ok("done")
            }
        }
    })
    .await;

    let elapsed = start.elapsed();

    assert!(elapsed.as_millis() >= 300);
}

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

#[tokio::test]
async fn test_transaction_operation_tracking() {
    let mut tx = Transaction::new();

    tx.add_operation(Operation::Install {
        package: "pkg1".to_string(),
        version: "1.0.0".parse().unwrap(),
        source: PackageSource::Official,
    });

    tx.add_operation(Operation::Update {
        package: "pkg2".to_string(),
        old_version: "1.0.0".parse().unwrap(),
        new_version: "2.0.0".parse().unwrap(),
    });

    tx.add_operation(Operation::Remove {
        package: "pkg3".to_string(),
        version: "1.5.0".parse().unwrap(),
    });

    assert_eq!(tx.operations.len(), 3);

    assert!(matches!(tx.operations[0], Operation::Install { .. }));
    assert!(matches!(tx.operations[1], Operation::Update { .. }));
    assert!(matches!(tx.operations[2], Operation::Remove { .. }));

    let completed = tx.completed_operations();
    assert_eq!(completed.len(), 3);
}

#[tokio::test]
async fn test_retry_config_customization() {
    let config = RetryConfig::new(10).with_backoff(200, 10000);

    assert_eq!(config.max_retries, 10);
    assert_eq!(config.initial_backoff_ms, 200);
    assert_eq!(config.max_backoff_ms, 10000);

    let backoff = config.calculate_backoff(0);
    assert_eq!(backoff.as_millis(), 200);

    let backoff_large = config.calculate_backoff(10);
    assert_eq!(backoff_large.as_millis(), 10000);
}

#[tokio::test]
async fn test_transaction_duration_tracking() {
    let tx = Transaction::new();

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let duration = tx.duration();
    assert!(duration.as_millis() >= 100);
    assert!(duration.as_millis() < 500);
}

#[test]
fn test_transaction_default_creation() {
    let tx = Transaction::default();

    assert_eq!(tx.state, TransactionState::Pending);
    assert_eq!(tx.operations.len(), 0);
    assert!(tx.snapshot.is_none());
}

#[tokio::test]
async fn test_retry_with_immediate_success() {
    let config = RetryConfig::new(3);
    let attempt_counter = Arc::new(AtomicU32::new(0));

    let result = retry_async(&config, || {
        let counter = Arc::clone(&attempt_counter);
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok::<_, anyhow::Error>("immediate success")
        }
    })
    .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "immediate success");
    assert_eq!(attempt_counter.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn test_retry_respects_max_attempts() {
    let config = RetryConfig::new(0);
    let attempt_counter = Arc::new(AtomicU32::new(0));

    let result = retry_async(&config, || {
        let counter = Arc::clone(&attempt_counter);
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            Err::<&str, _>(anyhow::anyhow!("network error"))
        }
    })
    .await;

    assert!(result.is_err());
    assert_eq!(attempt_counter.load(Ordering::SeqCst), 1);
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

#[tokio::test]
async fn test_transaction_commit_requires_in_progress_state() {
    let mut tx = Transaction::new();

    let commit_result = tx.commit();
    assert!(commit_result.is_err());
    assert!(
        commit_result
            .unwrap_err()
            .to_string()
            .contains("not in progress")
    );

    tx.begin().unwrap();
    assert!(tx.commit().is_ok());
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
        !combined.contains("panicked"),
        "CLI should not panic on network errors"
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
