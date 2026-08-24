//! Update Command Integration Tests
//!
//! End-to-end integration tests for the update command,
//! testing the full Elm Architecture workflow.
//!
//! Run: cargo test --test update_integration --features arch
//!
//! Environment variables:
//!   OMG_RUN_SYSTEM_TESTS=1    - Enable tests requiring real system access

#![expect(clippy::unwrap_used)]
#![expect(clippy::pedantic)]

pub mod common;

use common::*;

// ═══════════════════════════════════════════════════════════════════════════════
// CHECK MODE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod check_mode_tests {
    use super::*;

    #[test]
    fn test_check_flag_is_recognized() {
        let result = run_omg(&["update", "--check"]);
        // --check is read-only: it must succeed in a working environment or
        // fail naming the privilege problem — never prompt, never dangle.
        assert_no_password_prompt(&result);
        if !result.success {
            let combined = result.combined_output();
            assert!(
                combined.contains("sudo")
                    || combined.contains("permission")
                    || combined.contains("root")
                    || combined.contains("turbo"),
                "--check failure must name its cause. Got:\n{combined}"
            );
        }
    }

    #[test]
    fn test_check_mode_succeeds() {
        require_system_tests!();

        let result = run_omg(&["update", "--check"]);
        assert_no_password_prompt(&result);

        assert!(
            result.success,
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
    fn test_check_mode_does_not_prompt() {
        require_system_tests!();

        let result = run_omg(&["update", "--check"]);
        assert_no_password_prompt(&result);
    }

    #[test]
    fn test_check_mode_with_yes_flag() {
        require_system_tests!();

        let result = run_omg(&["update", "--check", "--yes"]);
        assert_no_password_prompt(&result);

        assert!(
            result.success,
            "Check with --yes should succeed. Output:\n{}",
            result.combined_output()
        );
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
        assert_runs_without_panic(&result);
        // --yes opts into non-interactive operation; complaining about
        // interactive terminals defeats its purpose.
        let combined = result.combined_output();
        assert!(
            !combined.contains("requires an interactive terminal"),
            "--yes must not demand a TTY. Got:\n{combined}"
        );
    }

    #[test]
    fn test_short_y_flag() {
        let result = run_omg(&["update", "-y"]);
        if result.success {
            assert!(!result.stdout.is_empty(), "-y success shows its work");
        } else {
            let combined = result.combined_output().to_lowercase();
            assert!(
                combined.contains("sudo")
                    || combined.contains("permission")
                    || combined.contains("root")
                    || combined.contains("development"),
                "-y failure must name the blocker. Got:\n{combined}"
            );
        }
    }

    #[test]
    fn test_ci_mode_with_yes() {
        let result = run_omg_with_env(
            &["update", "--yes"],
            &[("CI", "1"), ("OMG_NON_INTERACTIVE", "1")],
        );

        assert_runs_without_panic(&result);
        if !result.success {
            let combined = result.combined_output().to_lowercase();
            assert!(
                combined.contains("sudo")
                    || combined.contains("permission")
                    || combined.contains("root")
                    || combined.contains("development"),
                "CI --yes failure must name the blocker. Got:\n{combined}"
            );
        }
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
        assert_runs_without_panic(&result);
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
        assert_runs_without_panic(&result);
    }

    #[test]
    fn test_missing_daemon_fallback() {
        // We set OMG_DISABLE_DAEMON=1 in run_omg
        let result = run_omg(&["update", "--check"]);
        assert_runs_without_panic(&result);
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
    }

    #[test]
    fn regression_n_flag_fallback_detection() {
        // Regression test for -n flag fallback detection
        // The bug was: sudo -n exit code wasn't properly detected
        let result = run_omg_with_env(&["update", "--yes"], &[("CI", "1")]);

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
        assert_runs_without_panic(&result);
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
            assert_runs_without_panic(&result);
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
        assert_runs_without_panic(&result);
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

/// The real contract behind every former `success || !contains("panicked")`:
/// the process must not panic (no `panicked at`, exit 101) and must render
/// SOME outcome. The old form passed for any output whatsoever — including
/// the panic message itself.
fn assert_runs_without_panic(result: &CommandResult) {
    let combined = result.combined_output();
    assert!(
        !combined.contains("panicked at"),
        "command must not panic. Got:\n{combined}"
    );
    assert_ne!(result.exit_code, 101, "panic exit code observed");
    assert!(
        !result.stdout.is_empty() || !result.stderr.is_empty(),
        "command produced no output at all"
    );
}

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
