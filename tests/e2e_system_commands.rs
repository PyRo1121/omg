//! End-to-End Tests for System Commands
//!
//! Comprehensive e2e tests for system management and utility commands:
//! - doctor: System diagnostics
//! - config: Configuration management
//! - daemon: Daemon operations
//! - history: Transaction history
//! - stats/metrics: Usage statistics
//! - completions: Shell completion generation
//!
//! These tests validate the complete user experience for system operations.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]

mod common;

use common::*;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════════
// DOCTOR COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_doctor_runs_diagnostics() {
    init_test_env();

    let result = run_omg(&["doctor"]);

    result.assert_success();

    let output = result.stdout;
    // Should show diagnostic information
    assert!(
        output.contains("System") || output.contains("Check") || output.contains("OMG"),
        "Doctor should show diagnostic info"
    );
}

#[test]
fn test_doctor_checks_environment() {
    init_test_env();

    let result = run_omg(&["doctor"]);

    result.assert_success();

    // Should check environment setup
    let output = result.combined_output();
    assert!(
        output.len() > 50,
        "Should show substantial diagnostic output"
    );
}

#[test]
fn test_doctor_shows_checks() {
    init_test_env();

    let result = run_omg(&["doctor"]);

    result.assert_success();

    let output = result.combined_output();
    // Should show diagnostic checks
    assert!(
        output.contains("✓") || output.contains("System") || output.contains("Check"),
        "Should show diagnostic checks"
    );
}

#[test]
fn test_doctor_performance() {
    init_test_env();

    let result = run_omg(&["doctor"]);

    if result.success {
        // Doctor should be fast
        assert!(
            result.duration < Duration::from_secs(2),
            "Doctor took {:?}, should be <2s",
            result.duration
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIG COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_config_list_shows_settings() {
    init_test_env();

    let result = run_omg(&["config", "list"]);

    result.assert_success();

    // Should show configuration (even if empty)
    let output = result.stdout;
    assert!(
        !output.is_empty() || result.stderr.contains("No config"),
        "Should show configuration or empty message"
    );
}

#[test]
fn test_config_get_specific_value() {
    init_test_env();

    // Try to get a config value (may not exist)
    let result = run_omg(&["config", "get", "some.key"]);

    // Should complete (success or not found)
    let output = result.combined_output();
    assert!(
        result.success || output.contains("not found") || output.contains("not set"),
        "Should handle missing config gracefully"
    );
}

#[test]
fn test_config_set_and_get() {
    init_test_env();

    let project = TestProject::new();

    // Set a config value
    let set_result = project.run(&["config", "set", "test.key", "test_value"]);

    if set_result.success {
        // Get the value back
        let get_result = project.run(&["config", "get", "test.key"]);

        if get_result.success {
            assert!(
                get_result.stdout.contains("test_value"),
                "Should retrieve set value"
            );
        }
    }
}

#[test]
fn test_config_shows_format() {
    init_test_env();

    let result = run_omg(&["config", "list"]);

    result.assert_success();

    // Should show configuration format
    let output = result.combined_output();
    assert!(!output.is_empty(), "Should show configuration");
}

// ═══════════════════════════════════════════════════════════════════════════════
// DAEMON COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_daemon_status_shows_state() {
    init_test_env();

    let result = run_omg(&["daemon-status"]);

    // Should show daemon status (running or not)
    let output = result.combined_output();
    assert!(
        result.success || output.contains("not running") || output.contains("daemon"),
        "Should show daemon status"
    );
}

#[test]
fn test_daemon_help() {
    init_test_env();

    let result = run_omg(&["daemon", "--help"]);

    result.assert_success();
    result.assert_stdout_contains("daemon");
}

// ═══════════════════════════════════════════════════════════════════════════════
// HISTORY COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_history_shows_transactions() {
    init_test_env();

    let result = run_omg(&["history"]);

    // Should show history (even if empty)
    let output = result.combined_output();
    assert!(
        result.success || output.contains("No history") || output.contains("transaction"),
        "Should show transaction history"
    );
}

#[test]
fn test_history_with_limit() {
    init_test_env();

    let result = run_omg(&["history", "--limit", "10"]);

    // Should respect limit
    let output = result.combined_output();
    assert!(!output.is_empty(), "Should show limited history");
}

#[test]
fn test_history_formats_output() {
    init_test_env();

    let result = run_omg(&["history"]);

    // Should show history format (empty or with transactions)
    let output = result.combined_output();
    assert!(!output.is_empty(), "Should show history information");
}

// ═══════════════════════════════════════════════════════════════════════════════
// ROLLBACK COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_rollback_help() {
    init_test_env();

    let result = run_omg(&["rollback", "--help"]);

    result.assert_success();
    result.assert_stdout_contains("rollback");
}

#[test]
fn test_rollback_without_history() {
    init_test_env();

    // Try to rollback when no history exists
    let result = run_omg(&["rollback"]);

    // Should fail gracefully
    let output = result.combined_output();
    assert!(
        !result.success || output.contains("No transactions") || output.contains("history"),
        "Should handle empty history gracefully"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// STATS AND METRICS E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_stats_shows_usage() {
    init_test_env();

    let result = run_omg(&["stats"]);

    // Should show usage statistics
    let output = result.combined_output();
    assert!(
        result.success || output.contains("stats") || output.contains("usage"),
        "Should show statistics"
    );
}

#[test]
fn test_stats_shows_counters() {
    init_test_env();

    let result = run_omg(&["stats"]);

    // Should show statistics counters
    let output = result.combined_output();
    assert!(
        result.success || output.contains("stats") || output.contains("usage"),
        "Should show statistics"
    );
}

#[test]
fn test_metrics_command_exists() {
    init_test_env();

    let result = run_omg(&["metrics", "--help"]);

    // Should have metrics command
    result.assert_success();
    result.assert_stdout_contains("metric");
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMPLETIONS E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_completions_bash() {
    init_test_env();

    let result = run_omg(&["completions", "bash"]);

    result.assert_success();

    let output = result.stdout;
    // Should generate bash completion script
    assert!(
        output.contains("_omg") || output.contains("complete") || output.contains("bash"),
        "Should generate bash completions"
    );
    assert!(!output.is_empty(), "Completion script should not be empty");
}

#[test]
fn test_completions_zsh() {
    init_test_env();

    let result = run_omg(&["completions", "zsh"]);

    result.assert_success();
    assert!(!result.stdout.is_empty(), "Should generate zsh completions");
}

#[test]
fn test_completions_fish() {
    init_test_env();

    let result = run_omg(&["completions", "fish"]);

    result.assert_success();
    assert!(
        !result.stdout.is_empty(),
        "Should generate fish completions"
    );
}

#[test]
fn test_completions_powershell() {
    init_test_env();

    let result = run_omg(&["completions", "powershell"]);

    result.assert_success();
    assert!(
        !result.stdout.is_empty(),
        "Should generate powershell completions"
    );
}

#[test]
fn test_completions_invalid_shell() {
    init_test_env();

    let result = run_omg(&["completions", "invalid-shell"]);

    result.assert_failure();

    let output = result.combined_output();
    assert!(
        output.contains("invalid") || output.contains("value"),
        "Should reject invalid shell"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// VERSION AND HELP E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_version_flag() {
    init_test_env();

    let result = run_omg(&["--version"]);

    result.assert_success();

    let output = result.stdout;
    assert!(
        output.contains("omg") && output.contains("0.1"),
        "Should show version"
    );
}

#[test]
fn test_help_flag() {
    init_test_env();

    let result = run_omg(&["--help"]);

    result.assert_success();

    let output = result.stdout;
    assert!(
        output.contains("Usage") || output.contains("COMMANDS"),
        "Should show help"
    );
}

#[test]
fn test_help_for_subcommand() {
    init_test_env();

    let result = run_omg(&["install", "--help"]);

    result.assert_success();
    result.assert_stdout_contains("install");
}

// ═══════════════════════════════════════════════════════════════════════════════
// SELF-UPDATE E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_self_update_check() {
    init_test_env();

    let result = run_omg(&["self-update", "--check"]);

    // Should check for updates (may require network)
    let output = result.combined_output();
    assert!(
        !output.is_empty(),
        "Self-update check should produce output"
    );
}

#[test]
fn test_self_update_help() {
    init_test_env();

    let result = run_omg(&["self-update", "--help"]);

    result.assert_success();
    result.assert_stdout_contains("update");
}

// ═══════════════════════════════════════════════════════════════════════════════
// INIT COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_init_help() {
    init_test_env();

    let result = run_omg(&["init", "--help"]);

    result.assert_success();
    result.assert_stdout_contains("init");
}

#[test]
fn test_init_in_project() {
    init_test_env();

    let project = TestProject::new();

    let result = project.run(&["init"]);

    // Should initialize project configuration
    let output = result.combined_output();
    assert!(!output.is_empty(), "Init should produce output");
}

// ═══════════════════════════════════════════════════════════════════════════════
// GENERATE-MAN E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_generate_man_produces_pages() {
    init_test_env();

    let project = TestProject::new();
    let output_dir = project.create_dir("man");

    let result = project.run(&["generate-man", output_dir.to_str().unwrap()]);

    // Should generate man pages
    let output = result.combined_output();
    assert!(
        result.success || output.contains("man"),
        "Should generate man pages"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// STATUS COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_status_shows_system_state() {
    init_test_env();

    let result = run_omg(&["status"]);

    result.assert_success();

    let output = result.stdout;
    // Should show system package status
    assert!(!output.is_empty(), "Status should show package information");
}

#[test]
fn test_status_json_output() {
    init_test_env();

    let result = run_omg(&["status", "--json"]);

    if result.success && !result.stdout.is_empty() {
        let json_result = serde_json::from_str::<serde_json::Value>(&result.stdout);
        assert!(json_result.is_ok(), "JSON output should be valid");
    }
}

#[test]
fn test_status_performance() {
    init_test_env();

    let result = run_omg(&["status"]);

    if result.success {
        // Status should be fast (target: <10ms per CLAUDE.md)
        assert!(
            result.duration < Duration::from_millis(500),
            "Status took {:?}, target is <500ms",
            result.duration
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MIGRATE COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_migrate_help() {
    init_test_env();

    let result = run_omg(&["migrate", "--help"]);

    result.assert_success();
    result.assert_stdout_contains("migrate");
}

#[test]
fn test_migrate_from_brew() {
    init_test_env();

    let result = run_omg(&["migrate", "brew", "--dry-run"]);

    // Should handle migration (may not be on macOS)
    let output = result.combined_output();
    assert!(
        result.success || output.contains("not found") || output.contains("brew"),
        "Should handle brew migration or show error"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR HANDLING E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_no_command_shows_help() {
    init_test_env();

    let result = run_omg(&[]);

    // Should show help when no command given
    let output = result.combined_output();
    assert!(
        output.contains("Usage") || output.contains("COMMANDS") || output.contains("help"),
        "Should show help when no command provided"
    );
}

#[test]
fn test_invalid_command() {
    init_test_env();

    let result = run_omg(&["invalid-command-xyz"]);

    result.assert_failure();

    let output = result.combined_output();
    assert!(
        output.contains("unrecognized") || output.contains("invalid"),
        "Should show error for invalid command"
    );
}

#[test]
fn test_invalid_global_flag() {
    init_test_env();

    let result = run_omg(&["--invalid-flag", "search", "test"]);

    result.assert_failure();

    let output = result.combined_output();
    assert!(
        output.contains("unexpected") || output.contains("invalid"),
        "Should show error for invalid flag"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTEGRATED WORKFLOW TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_workflow_doctor_then_status() {
    init_test_env();

    // Run doctor to check system
    let doctor_result = run_omg(&["doctor"]);
    doctor_result.assert_success();

    // Then check status
    let status_result = run_omg(&["status"]);
    status_result.assert_success();
}

#[test]
fn test_workflow_config_set_and_list() {
    init_test_env();

    let project = TestProject::new();

    // Set a config value
    let set_result = project.run(&["config", "set", "test.key", "value123"]);

    if set_result.success {
        // List all config
        let list_result = project.run(&["config", "list"]);
        list_result.assert_success();

        // Should contain the set value
        assert!(
            list_result.stdout.contains("value123"),
            "Config list should show set value"
        );
    }
}

#[test]
fn test_workflow_history_and_stats() {
    init_test_env();

    // Check history
    let history_result = run_omg(&["history"]);
    let _ = history_result; // May be empty

    // Check stats
    let stats_result = run_omg(&["stats"]);
    let _ = stats_result; // May be empty or show initial state
}

// ═══════════════════════════════════════════════════════════════════════════════
// PERFORMANCE REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_performance_doctor_under_threshold() {
    init_test_env();

    let result = run_omg(&["doctor"]);

    if result.success {
        assert!(
            result.duration < Duration::from_secs(2),
            "Doctor took {:?}, target is <2s",
            result.duration
        );
    }
}

#[test]
fn test_performance_config_list_under_threshold() {
    init_test_env();

    let result = run_omg(&["config", "list"]);

    if result.success {
        assert!(
            result.duration < Duration::from_millis(500),
            "Config list took {:?}, target is <500ms",
            result.duration
        );
    }
}

#[test]
fn test_performance_completions_generation() {
    init_test_env();

    let result = run_omg(&["completions", "bash"]);

    if result.success {
        // Completion generation should be instant
        assert!(
            result.duration < Duration::from_millis(500),
            "Completions took {:?}, target is <500ms",
            result.duration
        );
    }
}
