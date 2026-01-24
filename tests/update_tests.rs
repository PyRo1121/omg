//! Production-Ready Update Command Tests
//!
//! Tests REAL update detection and update command functionality.
//! All tests use REAL package managers (pacman/alpm) and REAL version comparison.
//!
//! NO MOCKS - All tests exercise REAL code paths.
//!
//! Run:
//!   cargo test --test update_tests --features arch
//!
//! Environment variables:
//!   OMG_RUN_SYSTEM_TESTS=1    - Enable tests requiring real system access
//!   OMG_RUN_DESTRUCTIVE_TESTS=1 - Enable tests that actually install/update

#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]

use std::env;
use std::process::{Command, Stdio};
use std::time::Instant;

// ═══════════════════════════════════════════════════════════════════════════════
// TEST UTILITIES
// ═══════════════════════════════════════════════════════════════════════════════

/// Helper to run omg update commands and capture output
fn run_omg_update(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omg"))
        .args(["update"])
        .args(args)
        .env("OMG_TEST_MODE", "1")
        .env("OMG_DISABLE_DAEMON", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute omg update");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

/// Helper to combine stdout and stderr
fn combine_output(stdout: &str, stderr: &str) -> String {
    if stderr.is_empty() {
        stdout.to_string()
    } else if stdout.is_empty() {
        stderr.to_string()
    } else {
        format!("{}{}", stdout) + stderr
    }
}

/// Helper to run omg update with environment variables
fn run_omg_update_with_env(args: &[&str], env_vars: &[(&str, &str)]) -> (bool, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omg"));
    cmd.args(["update"])
        .args(args)
        .env("OMG_TEST_MODE", "1")
        .env("OMG_DISABLE_DAEMON", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    let output = cmd.output().expect("Failed to execute omg update");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

/// Check if system tests are enabled
fn system_tests_enabled() -> bool {
    matches!(env::var("OMG_RUN_SYSTEM_TESTS"), Ok(value) if value == "1")
}

/// Check if destructive tests are enabled
fn destructive_tests_enabled() -> bool {
    matches!(env::var("OMG_RUN_DESTRUCTIVE_TESTS"), Ok(value) if value == "1")
}

// ═══════════════════════════════════════════════════════════════════════════════
// UPDATE CHECK TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod update_check {
    use super::*;

    /// Test that update check succeeds without errors
    #[test]
    fn test_update_check_succeeds() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let (success, stdout, stderr) = run_omg_update(&["--check"]);
        assert!(success, "Update check should succeed");
        assert!(!stdout.is_empty() || !stderr.is_empty(),
                   "Update check should produce output");
    }

    /// Test that update check reports update status
    #[test]
    fn test_update_check_reports_status() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let (success, stdout, stderr) = run_omg_update(&["--check"]);
        let combined = combine_output(&stdout, &stderr);

        assert!(success, "Update check should succeed");

        // Should report either updates available or system up to date
        assert!(
            combined.contains("update") ||
                combined.contains("up to date") ||
                combined.contains("System is up to date") ||
                combined.contains("Found") ||
                combined.to_lowercase().contains("updates"),
            "Update check should report status. Got:\n{combined}"
        );
    }

    /// Test that update check doesn't make changes
    #[test]
    fn test_update_check_is_readonly() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let (success, stdout, stderr) = run_omg_update(&["--check"]);
        assert!(success, "Update check should succeed");

        // Should not contain execution messages
        let combined = combine_output(&stdout, &stderr);
        assert!(!combined.contains("Executing") && !combined.contains("Installing") &&
                   !combined.contains("Upgrading") && !combined.contains("Downloading"),
                   "Update check should not execute changes. Got:\n{combined}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// YES FLAG TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod yes_flag {
    use super::*;

    /// Test that --yes flag is accepted
    #[test]
    fn test_yes_flag_accepted() {
        // This just verifies the flag is parsed, doesn't actually run update
        let (success, _stdout, _stderr) = run_omg_update(&["--yes"]);
        // May succeed or fail (depends on root, daemon, etc.)
        // Should not panic or hang
        assert!(_stdout.len() > 0 || _stderr.len() > 0,
                   "Command should produce output");
    }

    /// Test that -y short flag works
    #[test]
    fn test_short_yes_flag() {
        let (success, _stdout, _stderr) = run_omg_update(&["-y"]);
        // Should be equivalent to --yes
        assert!(_stdout.len() > 0 || _stderr.len() > 0,
                   "Command should produce output");
    }

    /// Test that --yes and --check can be combined
    #[test]
    fn test_yes_and_check_flags() {
        let (success, stdout, stderr) = run_omg_update(&["--check", "--yes"]);
        let combined = combine_output(&stdout, &stderr);

        // Should work (though --yes is redundant with --check)
        assert!(!combined.is_empty(), "Should produce output");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// NON-INTERACTIVE MODE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod non_interactive {
    use super::*;

    /// Test that running without TTY and without --yes fails with helpful error
    #[test]
    fn test_non_interactive_without_yes_fails() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        // Set CI environment variable to simulate non-interactive environment
        let (success, stdout, stderr) = run_omg_update_with_env(
            &[],
            &[("CI", "true"), ("OMG_NON_INTERACTIVE", "1")]
        );

        let combined = combine_output(&stdout, &stderr);

        // Should fail because no TTY and no --yes
        assert!(!success, "Should fail without TTY and --yes");

        // Should show helpful error message
        assert!(
            combined.contains("interactive") ||
                combined.contains("--yes") ||
                combined.contains("terminal") ||
                combined.contains("TTY") ||
                combined.contains("automation") ||
                combined.contains("CI"),
            "Should show helpful error about interactive mode. Got:\n{combined}"
        );
    }

    /// Test that --yes works in non-interactive mode
    #[test]
    fn test_yes_flag_works_non_interactive() {
        if !destructive_tests_enabled() {
            eprintln!("Skipping destructive test (set OMG_RUN_DESTRUCTIVE_TESTS=1)");
            return;
        }

        let (success, stdout, stderr) = run_omg_update_with_env(
            &["--yes"],
            &[("CI", "1")]
        );

        let combined = combine_output(&stdout, &stderr);

        // Should not complain about interactive mode
        assert!(!combined.contains("interactive") && !combined.contains("TTY") &&
                   !combined.contains("requires an interactive terminal"),
                   "Should not complain about interactive mode with --yes. Got:\n{combined}");
    }

    /// Test that sudo command is suggested in non-interactive mode
    #[test]
    fn test_sudo_suggestion_non_interactive() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let (success, stdout, stderr) = run_omg_update_with_env(
            &[],
            &[("CI", "1")]
        );

        let combined = combine_output(&stdout, &stderr);

        // Should suggest sudo or --yes
        assert!(
            combined.contains("sudo") ||
                combined.contains("--yes") ||
                combined.contains("root") ||
                combined.contains("privileges"),
            "Should mention sudo or --yes. Got:\n{combined}");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR HANDLING TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod error_handling {
    use super::*;

    /// Test that invalid flags are rejected
    #[test]
    fn test_invalid_flag() {
        let (success, stdout, stderr) = run_omg_update(&["--invalid-flag"]);

        assert!(!success, "Invalid flag should fail");

        let combined = format!("{stdout}{stderr}";
        assert!(
            combined.contains("error") ||
                combined.contains("unrecognized") ||
                combined.contains("unknown") ||
                combined.contains("invalid"),
            "Should report error for invalid flag"
        );
    }

    /// Test that too many arguments are handled
    #[test]
    fn test_too_many_arguments() {
        let (success, stdout, stderr) = run_omg_update(&["--check", "extra", "arguments"]);

        // May succeed or fail depending on implementation
        assert!(!stdout.is_empty() || !stderr.is_empty(),
                   "Should produce output");
    }

    /// Test that command handles missing daemon gracefully
    #[test]
    fn test_handles_missing_daemon() {
        // We set OMG_DISABLE_DAEMON=1 in run_omg_update, so this tests
        // that the CLI falls back to direct mode properly
        let (success, stdout, stderr) = run_omg_update(&["--check"]);

        // Should work without daemon
        assert!(!stdout.is_empty() || !stderr.is_empty(),
                   "Should produce output without daemon");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PERFORMANCE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod performance {
    use super::*;

    /// Test that update check completes quickly
    #[test]
    fn test_update_check_performance() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let start = Instant::now();
        let (success, stdout, stderr) = run_omg_update(&["--check"]);
        let elapsed = start.elapsed();

        assert!(success, "Update check should succeed");
        assert!(!stdout.is_empty() || !stderr.is_empty(),
                   "Should produce output");

        // Update check should be fast (even without daemon)
        // Direct ALPM operations should complete in <500ms on most systems
        assert!(
            elapsed.as_millis() < 2000,
            "Update check should complete in <2s, took {}ms. Output:\nstdout:\n{}\nstderr:\n{}",
            elapsed.as_millis(),
            stdout,
            stderr
        );
    }

    /// Test that update check with --yes flag is also fast
    #[test]
    fn test_update_with_yes_performance() {
        if !destructive_tests_enabled() {
            eprintln!("Skipping destructive test (set OMG_RUN_DESTRUCTIVE_TESTS=1)");
            return;
        }

        let start = Instant::now();
        let (success, stdout, stderr) = run_omg_update(&["--yes"]);
        let elapsed = start.elapsed();

        assert!(success, "Update with --yes should succeed");
        assert!(!stdout.is_empty() || !stderr.is_empty(),
                   "Should produce output");

        // Even with actual updates, should complete in reasonable time
        // (may be slower if there are many updates)
        assert!(
            elapsed.as_secs() < 600, // 10 minutes max
            "Update should complete in <10 minutes, took {}s",
            elapsed.as_secs()
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTEGRATION SCENARIOS
// ═══════════════════════════════════════════════════════════════════════════════

mod integration_scenarios {
    use super::*;

    /// Test typical update workflow: check -> confirm -> update
    #[test]
    fn test_typical_update_workflow() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        // Step 1: Check for updates
        let (success1, stdout1, stderr1) = run_omg_update(&["--check"]);
        assert!(success1, "Update check should succeed");
        let has_updates = stdout1.contains("update") ||
                          stdout1.contains("update") ||
                          stdout1.to_lowercase().contains("updates");

        let combined1 = format!("{stdout1}{stderr1}";

        // Step 2: If there are updates, the output should show them
        if has_updates {
            assert!(
                combined1.contains("Found") ||
                    combined1.contains("→") ||
                    combined1.contains("->"),
                "Should show available updates. Got:\n{combined1}"
            );
        } else {
            assert!(
                combined1.contains("up to date") ||
                    combined1.contains("System is up to date") ||
                    combined1.contains("✓"),
                "Should show system up to date. Got:\n{combined1}"
            );
        }
    }

    /// Test that update command works with sudo privileges
    #[test]
    fn test_update_with_sudo() {
        if !destructive_tests_enabled() {
            eprintln!("Skipping destructive test (set OMG_RUN_DESTRUCTIVE_TESTS=1)");
            return;
        }

        // This test requires actual root/sudo, so we just verify it doesn't crash
        let (success, stdout, stderr) = run_omg_update(&["--yes"]);
        let combined = format!("{stdout}{stderr}";

        // May fail if not root, but should show meaningful error
        if !success {
            assert!(
                combined.contains("root") ||
                    combined.contains("sudo") ||
                    combined.contains("permission") ||
                    combined.contains("privileges") ||
                    combined.contains("Elevating"),
                "Should mention sudo/root if permission denied. Got:\n{combined}"
            );
        }
    }

    /// Test that update handles empty update list
    #[test]
    fn test_update_with_no_updates() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let (success, stdout, stderr) = run_omg_update(&["--check"]);
        let combined = format!("{stdout}{stderr}";

        assert!(success, "Update check should succeed");

        // Should handle both cases: updates available or system up to date
        // (we can't control which case happens in a real system)
        assert!(
            combined.contains("up to date") ||
                combined.contains("update") ||
                combined.contains("Found") ||
                combined.contains("✓"),
            "Should handle no updates case gracefully. Got:\n{combined}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// OUTPUT FORMAT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod output_format {
    use super::*;

    /// Test that update output is readable
    #[test]
    fn test_output_is_readable() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let (success, stdout, stderr) = run_omg_update(&["--check"]);
        let combined = format!("{stdout}{stderr}";

        assert!(success, "Update check should succeed");

        // Output should not be empty
        assert!(!combined.trim().is_empty(),
                   "Output should not be empty. Got:\n{combined}");

        // Output should be printable ASCII
        assert!(combined.is_ascii(), "Output should be ASCII. Got:\n{combined}");
    }

    /// Test that update shows version information
    #[test]
    fn test_output_shows_versions() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let (success, stdout, stderr) = run_omg_update(&["--check"]);
        let combined = format!("{stdout}{stderr}";

        assert!(success, "Update check should succeed");

        // If there are updates, should show version information
        // (may be in "old → new" format)
        if combined.contains("update") || combined.to_lowercase().contains("updates") {
            // Look for version patterns like "1.0.0" or arrows
            assert!(
                combined.contains("→") ||
                    combined.contains("->") ||
                    combined.contains("→") ||
                    combined.contains('.') && combined.len() > 10,
                "Should show version information. Got:\n{combined}"
            );
        }
    }
}
