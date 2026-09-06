#![expect(clippy::unwrap_used, clippy::expect_used, clippy::pedantic)]
//! Production-Ready Error Handling Tests
//!
//! Tests that errors are handled gracefully with helpful messages.
//! All tests use REAL code paths - NO MOCKS, NO STUBS.
//!
//! Every test asserts an OBSERVABLE outcome on every path: a command either
//! succeeds (with output proving real work) or fails (with an error naming
//! the problem and, where applicable, the remedy). The former
//! `success || contains(...)` chains passed whenever EITHER side held and
//! could never fail; they were rewritten per the audit's vacuous-assertion
//! finding.
//!
//! Run:
//!   cargo test --test error_tests --features arch

#![expect(clippy::missing_panics_doc)]
#![expect(clippy::missing_errors_doc)]

pub mod common;

use common::*;

/// Assert the invocation did not panic: a panic produces `panicked at` on
/// stderr and exit code 101. This is the contract for hostile-input tests.
fn assert_no_panic(result: &CommandResult) {
    let combined = result.combined_output();
    assert!(
        !combined.contains("panicked at"),
        "command must not panic. Got:\n{combined}"
    );
    assert_ne!(result.exit_code, 101, "panic exit code observed");
}

// ═══════════════════════════════════════════════════════════════════════════════
// NON-INTERACTIVE MODE ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

mod non_interactive_errors {
    use super::*;

    #[test]
    fn test_non_interactive_without_yes_shows_helpful_error() {
        // ===== ARRANGE =====
        let env_vars = &[("CI", "true"), ("OMG_NON_INTERACTIVE", "1")];

        // ===== ACT =====
        let result = run_omg_with_env(&["update"], env_vars);

        // ===== ASSERT =====
        // Both outcomes are legitimate depending on privilege state, but each
        // must prove itself: success shows real update work; failure names the
        // --yes remedy.
        let combined = result.combined_output();
        if result.success {
            assert!(
                combined.contains("pdate") || combined.contains("sync"),
                "successful update must show its work. Got:\n{combined}"
            );
        } else {
            assert!(
                combined.contains("--yes") || combined.contains("interactive"),
                "failure in non-interactive mode must suggest --yes. Got:\n{combined}"
            );
        }
    }

    #[test]
    fn test_privilege_error_suggests_command() {
        // ===== ARRANGE =====
        // Pass --yes to bypass the non-interactive check and hit the privilege check

        // ===== ACT =====
        let result = run_omg(&["update", "--yes"]);

        // ===== ASSERT =====
        let combined = result.combined_output();
        if result.success {
            assert!(
                combined.contains("pdate") || combined.contains("up to date"),
                "successful update must show its outcome. Got:\n{combined}"
            );
        } else {
            assert!(
                combined.contains("sudo")
                    || combined.contains("permission")
                    || combined.contains("root")
                    || combined.contains("turbo"),
                "privilege failure must name the elevation remedy. Got:\n{combined}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INVALID INPUT ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

mod invalid_input_errors {
    use super::*;
    use common::fixtures::packages::NONEXISTENT;

    #[test]
    fn test_invalid_package_name_error() {
        // ===== ARRANGE =====
        let nonexistent_pkg = NONEXISTENT[0];

        // ===== ACT =====
        let result = run_omg(&["info", nonexistent_pkg]);

        // ===== ASSERT =====
        // `info` on a package that is in neither repos nor AUR must fail with
        // an explicit not-found classification naming the query.
        result.assert_failure();
        let combined = result.combined_output();
        assert!(
            combined.contains("not found") && combined.contains(nonexistent_pkg),
            "info of a missing package must classify the failure AND name it. Got:\n{combined}"
        );
    }

    #[test]
    fn test_invalid_command_error() {
        // ===== ARRANGE =====
        let invalid_command = "invalid-command";

        // ===== ACT =====
        let result = run_omg(&[invalid_command]);

        // ===== ASSERT =====
        result.assert_failure();
        let combined = result.combined_output();
        assert!(
            combined.contains("unrecognized")
                || combined.contains("unknown")
                || combined.contains("No such"),
            "clap must reject the unknown command by name-class. Got:\n{combined}"
        );
    }

    #[test]
    fn test_invalid_flag_error() {
        // ===== ARRANGE =====
        let invalid_flag = "--invalid-flag";

        // ===== ACT =====
        let result = run_omg(&[invalid_flag]);

        // ===== ASSERT =====
        result.assert_failure();
        let combined = result.combined_output();
        assert!(
            combined.contains("unexpected argument")
                || combined.contains("unrecognized")
                || combined.contains("invalid"),
            "clap must reject the unknown flag as an argument error. Got:\n{combined}"
        );
    }

    #[test]
    fn test_missing_required_arg_error() {
        // ===== ARRANGE =====
        let incomplete_command = ["install"];

        // ===== ACT =====
        let result = run_omg(&incomplete_command);

        // ===== ASSERT =====
        result.assert_failure();
        let combined = result.combined_output();
        assert!(
            combined.contains("No packages specified"),
            "bare install must fail with guidance instead of a clap contract error. Got:\n{combined}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// NETWORK ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

mod network_errors {
    use super::*;

    #[test]
    fn test_network_timeout_handled_gracefully() {
        // ===== ARRANGE =====
        let env_vars = &[("OMG_NETWORK_TIMEOUT", "1")];

        // ===== ACT =====
        let result = run_omg_with_env(&["info", "non-existent-pkg-for-timeout"], env_vars);

        // ===== ASSERT =====
        // Hostile input + constrained network: the contract is no panic plus
        // SOME rendered outcome, not silence.
        assert_no_panic(&result);
        assert!(
            !result.stdout.is_empty() || !result.stderr.is_empty(),
            "timeout path must still produce user-visible output"
        );
    }

    #[test]
    fn test_sync_uses_hermetic_backend_without_privilege_prompt() {
        let result = run_omg(&["sync"]);

        result.assert_success();
        let combined = result.combined_output().to_lowercase();
        assert!(
            !combined.contains("privilege elevation")
                && !combined.contains("[sudo]")
                && !combined.contains("password:"),
            "test-mode sync must not contact host privilege machinery. Got:\n{combined}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DATABASE ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

mod database_errors {
    use super::*;
    use common::fixtures::error_conditions;

    #[test]
    fn test_corrupted_database_handled() {
        // ===== ARRANGE =====
        let project = error_conditions::corrupted_database();

        // ===== ACT =====
        let result = project.run(&["status"]);

        // ===== ASSERT =====
        // status against a corrupted store must not panic; it reports or
        // degrades, never aborts.
        assert_no_panic(&result);
    }

    #[test]
    fn test_missing_database_creates_new() {
        // ===== ARRANGE =====
        let project = TestProject::new();
        let data_dir = project.data_dir.path().join("omg_data");
        let data_dir_str = data_dir.to_str().unwrap();

        // ===== ACT =====
        let result = run_omg_with_env(&["status"], &[("OMG_DATA_DIR", data_dir_str)]);

        // ===== ASSERT =====
        // Fresh data dir: status must SUCCEED (empty state is valid), not
        // merely fail politely.
        result.assert_success();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIGURATION ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

mod config_errors {
    use super::*;
    use common::fixtures::error_conditions;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn test_invalid_config_toml_error() {
        // ===== ARRANGE =====
        let project = TestProject::new();
        // `Settings::config_path` reads `$OMG_CONFIG_DIR/config.toml` verbatim
        // (`paths::config_dir()` already ends in `omg`); a nested `omg/`
        // subdirectory would be a file the app never reads.
        let config_dir = project.config_dir.path();

        let config_file = config_dir.join("config.toml");
        let mut f = File::create(&config_file).unwrap();
        writeln!(f, "invalid toml {{{{").unwrap();

        project.create_file(".nvmrc", "20.0.0");

        // ===== ACT =====
        let result = project.run(&["hook-env", "-s", "bash"]);

        // ===== ASSERT =====
        // hook-env must always emit valid shell, so an unreadable config
        // falls back to defaults instead of failing. Pin that contract:
        // success plus no panic. If the fallback ever goes away, both
        // assertions fail loudly.
        let combined = result.combined_output();
        assert!(
            result.success,
            "hook-env must survive bad config. Got:\n{combined}"
        );
        assert!(
            !combined.contains("panicked at"),
            "Should not panic on invalid config. Got:\n{combined}"
        );
    }

    #[test]
    fn test_invalid_lock_file_error() {
        // ===== ARRANGE =====
        let project = error_conditions::invalid_lock_file();

        // ===== ACT =====
        let result = project.run(&["env", "check"]);

        // ===== ASSERT =====
        result.assert_failure();
        let combined = result.combined_output();
        // Must specifically report the lockfile problem, not just any
        // failure text ("error"/"failed" matched everything before wave 2;
        // hash-tamper behavior is pinned in tests/env_lockfile_integrity.rs).
        assert!(
            combined.contains("omg.lock") || combined.contains("parse"),
            "Invalid lock file should be detected. Got:\n{combined}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPFUL ERROR MESSAGES
// ═══════════════════════════════════════════════════════════════════════════════

mod helpful_messages {
    use super::*;

    #[test]
    fn test_errors_are_readable() {
        // ===== ARRANGE =====
        let invalid_cmd = "invalid-command";

        // ===== ACT =====
        let result = run_omg(&[invalid_cmd]);

        // ===== ASSERT =====
        result.assert_failure();
        let combined = result.combined_output();
        assert!(
            !combined.trim().is_empty(),
            "Error message should not be empty. Got:\n{combined}"
        );
        assert!(
            combined.is_ascii(),
            "Error message should be printable ASCII. Got:\n{combined}"
        );
    }

    #[test]
    fn test_errors_contain_actionable_info() {
        // ===== ARRANGE =====
        // (No special setup needed)

        // ===== ACT =====
        let result = run_omg(&["update"]);

        // ===== ASSERT =====
        // Success proves the command ran; failure must contain an actionable
        // hint (--yes / sudo / turbo are the three documented remedies).
        let combined = result.combined_output();
        if !result.success {
            assert!(
                combined.contains("--yes")
                    || combined.contains("sudo")
                    || combined.contains("turbo"),
                "update failure must name a remedy. Got:\n{combined}"
            );
        } else {
            assert!(!combined.is_empty(), "successful update prints its outcome");
        }
    }

    #[test]
    fn test_errors_show_context() {
        let nonexistent_pkg = "nonexistent-package";
        let result = run_omg(&["info", nonexistent_pkg]);

        // info on a missing package deterministically fails with
        // "Package 'X' not found" — the query echo alone is not enough, the
        // error must classify itself as a not-found condition.
        result.assert_failure();
        let combined = result.combined_output();
        assert!(
            combined.contains("not found") && combined.contains(nonexistent_pkg),
            "error must say 'not found' AND name the query. Got:\n{combined}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PANIC PREVENTION
// ═══════════════════════════════════════════════════════════════════════════════

mod panic_prevention {
    use super::*;

    /// The old assertions (`!success || stdout non-empty || stderr non-empty`)
    /// passed whenever ANY output existed at all — including the panic message
    /// itself. The actual contract is: no `panicked at`, no 101.
    #[test]
    fn test_empty_query_does_not_panic() {
        let result = run_omg(&["search", ""]);
        assert_no_panic(&result);
    }

    #[test]
    fn test_very_long_query_does_not_panic() {
        let long_query = "a".repeat(10_000);
        let result = run_omg(&["search", &long_query]);
        assert_no_panic(&result);
    }

    #[test]
    fn test_special_chars_do_not_panic() {
        let special_chars = "\x01\x02\x03\n\t\r";
        let result = run_omg(&["search", special_chars]);
        assert_no_panic(&result);
    }

    #[test]
    fn test_unicode_search_does_not_panic() {
        let unicode_query = "café-münchen";
        let result = run_omg(&["search", unicode_query]);
        assert_no_panic(&result);
    }
}
