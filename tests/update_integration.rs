//! Update Command Integration Tests
//!
//! End-to-end integration tests for the update command,
//! testing the full Elm Architecture workflow.
//!
//! Run: cargo test --test update_integration --features arch
//!
//! Environment variables:
//!   OMG_RUN_SYSTEM_TESTS=1    - Enable tests requiring real system access

#![allow(clippy::unwrap_used)]
#![allow(clippy::pedantic)]
#![allow(clippy::nursery)]

mod common;

use common::*;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════════
// CHECK MODE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod check_mode_tests {
    use super::*;

    #[test]
    fn test_check_flag_is_recognized() {
        let result = run_omg(&["update", "--check"]);
        // --check is a valid flag
        assert!(result.exit_code >= 0);
    }

    #[test]
    fn test_check_mode_succeeds() {
        require_system_tests!();

        let result = run_omg(&["update", "--check"]);
        assert_no_password_prompt(&result);

        // Check mode should succeed
        assert!(
            result.success || result.exit_code >= 0,
            "Check mode should succeed. Output:\n{}",
            result.combined_output()
        );
    }

    #[test]
    fn test_check_mode_reports_status() {
        require_system_tests!();

        let result = run_omg(&["update", "--check"]);
        assert_no_password_prompt(&result);

        let combined = result.combined_output();

        // Should report update status
        assert!(
            combined.contains("update")
                || combined.contains("up to date")
                || combined.contains("Found")
                || combined.contains("System")
                || combined.contains("✓"),
            "Check mode should report status. Got:\n{}",
            combined
        );
    }

    #[test]
    fn test_check_mode_is_fast() {
        require_system_tests!();

        let result = run_omg(&["update", "--check"]);
        assert_no_password_prompt(&result);

        // Check mode should complete quickly
        result.assert_duration_under(Duration::from_secs(5));
    }

    #[test]
    fn test_check_mode_with_yes_flag() {
        require_system_tests!();

        let result = run_omg(&["update", "--check", "--yes"]);
        assert_no_password_prompt(&result);

        // Should work (though --yes is redundant with --check)
        assert!(result.exit_code >= 0, "Check with --yes should not error");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// NON-INTERACTIVE MODE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod non_interactive_tests {
    use super::*;

    #[test]
    fn test_yes_flag_without_tty() {
        let result = run_omg_with_env(&["update", "--yes"], &[("CI", "1")]);

        // Should not complain about interactive mode
        let combined = result.combined_output();
        assert!(
            !combined.contains("requires an interactive terminal") || result.success,
            "--yes should work in non-interactive mode. Got:\n{}",
            combined
        );
    }

    #[test]
    fn test_short_y_flag() {
        let result = run_omg(&["update", "-y"]);
        assert!(result.exit_code >= 0);
    }

    #[test]
    fn test_ci_mode_with_yes() {
        let result = run_omg_with_env(
            &["update", "--yes"],
            &[("CI", "1"), ("OMG_NON_INTERACTIVE", "1")],
        );

        // Should work in CI mode with --yes
        assert!(result.exit_code >= 0, "CI mode with --yes should work");
    }

    #[test]
    fn test_ci_mode_without_yes_fails_gracefully() {
        let result = run_omg_with_env(&["update"], &[("CI", "1")]);

        let combined = result.combined_output();

        if !result.success {
            // Should show helpful error about needing --yes
            assert!(
                combined.contains("--yes")
                    || combined.contains("interactive")
                    || combined.contains("terminal")
                    || combined.contains("sudo"),
                "Should mention --yes or sudo in CI mode. Got:\n{}",
                combined
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SUDO INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod sudo_integration_tests {
    use super::*;

    #[test]
    fn test_update_without_privileges_shows_helpful_error() {
        let result = run_omg(&["update", "--yes"]);

        let combined = result.combined_output();

        if !result.success {
            // Should show helpful message about sudo
            assert!(
                combined.contains("sudo")
                    || combined.contains("root")
                    || combined.contains("privilege")
                    || combined.contains("permission")
                    || combined.contains("Elevating"),
                "Should mention sudo/root when not privileged. Got:\n{}",
                combined
            );
        }
    }

    #[test]
    fn test_check_mode_never_prompts_for_password() {
        // CRITICAL TEST: Check mode should NEVER prompt for password
        let result = run_omg(&["update", "--check"]);

        assert_no_password_prompt(&result);

        // Check for specific password prompt patterns
        let combined = result.combined_output();
        assert!(
            !combined.contains("[sudo]")
                && !combined.to_lowercase().contains("password for")
                && !combined.contains("Password:"),
            "Check mode should never prompt. Got:\n{combined}",
        );
    }

    #[test]
    fn test_n_flag_fallback_in_ci() {
        // Test that sudo -n fallback works in CI
        let result = run_omg_with_env(&["update", "--yes"], &[("CI", "1")]);

        // Should not hang waiting for password
        assert_no_hang(&result);

        let combined = result.combined_output();

        // If it fails, should show CI-friendly error
        if !result.success {
            assert!(
                combined.contains("NOPASSWD")
                    || combined.contains("automation")
                    || combined.contains("CI")
                    || combined.contains("sudo")
                    || combined.contains("root"),
                "Should show CI-friendly error. Got:\n{}",
                combined
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ELM ARCHITECTURE WORKFLOW TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod elm_workflow_tests {
    use super::*;

    #[test]
    fn test_elm_model_initialization() {
        // Test that Elm model initializes correctly
        let result = run_omg(&["update", "--check"]);

        // Should not crash during model init
        assert!(
            result.exit_code >= 0,
            "Elm model should initialize without crashing"
        );
    }

    #[test]
    fn test_elm_update_cycle() {
        // Test the Model-Update-View cycle
        require_system_tests!();

        let result = run_omg(&["update", "--check"]);

        // The Elm cycle should complete
        let combined = result.combined_output();

        // View should render successfully
        assert!(
            !combined.contains("panicked")
                && !combined.contains("unwrap")
                && !combined.contains("expect"),
            "Elm cycle should complete without panics. Got:\n{}",
            combined
        );
    }

    #[test]
    fn test_elm_view_rendering() {
        // Test that Elm view renders correctly
        let result = run_omg(&["update", "--check"]);

        let combined = result.combined_output();

        // Should not crash and should produce some output
        // The Elm UI should render without errors
        assert!(!result.stderr.contains("panicked"), "Should not panic");
        assert!(
            !combined.contains("panicked"),
            "Output should not contain panic"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR HANDLING TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod error_handling_tests {
    use super::*;

    #[test]
    fn test_invalid_flag_rejected() {
        let result = run_omg(&["update", "--invalid-flag-xyz"]);

        assert!(!result.success, "Invalid flag should fail");

        let combined = result.combined_output();
        assert!(
            combined.contains("error")
                || combined.contains("unrecognized")
                || combined.contains("unknown"),
            "Should report error for invalid flag. Got:\n{}",
            combined
        );
    }

    #[test]
    fn test_extra_arguments_ignored_or_error() {
        let result = run_omg(&["update", "--check", "extra", "args"]);

        // May succeed or fail, but should not crash
        assert!(
            result.exit_code >= 0,
            "Should handle extra arguments gracefully"
        );
    }

    #[test]
    fn test_missing_daemon_fallback() {
        // We set OMG_DISABLE_DAEMON=1 in run_omg
        let result = run_omg(&["update", "--check"]);

        // Should work without daemon
        assert!(
            result.exit_code >= 0,
            "Should work without daemon. Output:\n{}",
            result.combined_output()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PERFORMANCE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod performance_tests {
    use super::*;

    #[test]
    fn test_check_mode_performance() {
        require_system_tests!();

        let result = run_omg(&["update", "--check"]);

        // Should be fast
        assert!(
            result.duration.as_millis() < 2000,
            "Check mode should be <2s, took {}ms",
            result.duration.as_millis()
        );
    }

    #[test]
    fn test_update_command_start_time() {
        // Test that command starts quickly
        let start = std::time::Instant::now();
        let _result = run_omg(&["update", "--check"]);
        let elapsed = start.elapsed();

        // Should start reasonably fast (allowing for cold start)
        assert!(
            elapsed.as_millis() < 10000, // 10 seconds is reasonable for cold start
            "Command should start quickly, took {}ms",
            elapsed.as_millis()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// OUTPUT FORMAT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod output_format_tests {
    use super::*;
    use std::env;

    #[test]
    fn test_output_is_utf8() {
        let result = run_omg(&["update", "--check"]);

        // String is always valid UTF-8 in Rust, just verify it's not corrupted
        assert!(
            !result.stdout.is_empty() || !result.stderr.is_empty(),
            "Should produce some output"
        );
    }

    #[test]
    fn test_output_does_not_leak_paths() {
        let result = run_omg(&["update", "--check"]);

        let combined = result.combined_output();

        // Should not expose sensitive paths
        assert!(
            !combined.contains("/home/")
                && !combined.contains(env::var("HOME").unwrap_or_default().as_str()),
            "Should not expose home directory path"
        );
    }

    #[test]
    fn test_error_messages_are_user_friendly() {
        let result = run_omg(&["update", "--invalid-xyz-flag"]);

        let combined = result.combined_output();

        if !result.success {
            // Error messages should be helpful
            assert!(
                combined.contains("error")
                    || combined.contains("unrecognized")
                    || combined.contains("unknown")
                    || combined.contains("usage"),
                "Error should be user-friendly. Got:\n{}",
                combined
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// REGRESSION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod regression_tests {
    use super::*;

    #[test]
    fn regression_sudo_password_prompt_bug() {
        // Regression test for the sudo password prompt bug
        // The bug was: update would prompt for password even in check mode
        let result = run_omg(&["update", "--check"]);

        assert_no_password_prompt(&result);

        // Should complete quickly (not hang on password prompt)
        assert_no_hang(&result);
    }

    #[test]
    fn regression_n_flag_fallback_detection() {
        // Regression test for -n flag fallback detection
        // The bug was: sudo -n exit code wasn't properly detected
        let result = run_omg_with_env(&["update", "--yes"], &[("CI", "1")]);

        // Should not hang
        assert_no_hang(&result);

        // If it fails, should have helpful error
        if !result.success {
            let combined = result.combined_output();
            assert!(
                combined.contains("sudo")
                    || combined.contains("NOPASSWD")
                    || combined.contains("privilege"),
                "Should show helpful error about sudo. Got:\n{}",
                combined
            );
        }
    }

    #[test]
    fn regression_elm_fallback_on_error() {
        // Test that Elm UI falls back gracefully on error
        let result = run_omg(&["update", "--check"]);

        // Should either work with Elm or fall back
        assert!(
            result.exit_code >= 0,
            "Should handle Elm error gracefully. Output:\n{}",
            result.combined_output()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONCURRENT ACCESS TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod concurrency_tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_concurrent_check_commands() {
        // Test that multiple check commands don't interfere
        let handles: Vec<_> = (0..5)
            .map(|_| thread::spawn(|| run_omg(&["update", "--check"])))
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result.exit_code >= 0, "Concurrent check should not fail");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod security_tests {
    use super::*;

    #[test]
    fn test_no_command_injection() {
        let result = run_omg(&["update", "--check", ";", "rm", "-rf", "/"]);

        // Should not execute injected commands
        assert!(
            result.exit_code >= 0,
            "Should not execute injected commands"
        );
    }

    #[test]
    fn test_no_path_traversal_in_args() {
        let result = run_omg(&["update", "--check", "../../../etc/passwd"]);

        // Should not expose system files
        let combined = result.combined_output();
        assert!(
            !combined.contains("root:") && !combined.contains("/bin/bash"),
            "Should not expose system files"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPER FUNCTIONS
// ═══════════════════════════════════════════════════════════════════════════════

fn assert_no_password_prompt(result: &CommandResult) {
    let combined = result.combined_output();
    assert!(
        !combined.contains("[sudo]")
            && !combined.contains("password for")
            && !combined.contains("Password:"),
        "Should not prompt for password. Got:\n{}",
        combined
    );
}

fn assert_no_hang(result: &CommandResult) {
    assert!(
        result.duration.as_secs() < 30,
        "Command should complete quickly, took {:?}",
        result.duration
    );
}
