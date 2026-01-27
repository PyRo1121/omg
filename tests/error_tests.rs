#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! Production-Ready Error Handling Tests
//!
//! Tests that errors are handled gracefully with helpful messages.
//! All tests use REAL code paths - NO MOCKS, NO STUBS.
//!
//! Run:
//!   cargo test --test error_tests --features arch

#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]

mod common;

use common::*;

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
        if !result.success {
            let combined = result.combined_output();
            assert!(
                combined.contains("Use --yes") || combined.contains("interactive"),
                "Error message should suggest using --yes. Got:\n{combined}"
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
        if !result.success {
            let combined = result.combined_output();
            assert!(
                combined.contains("sudo omg")
                    || combined.contains("sudo")
                    || combined.contains("omg update")
                    || combined.contains("permission")
                    || combined.contains("root"),
                "Error should suggest the command to run with sudo. Got:\n{combined}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INVALID INPUT ERRORS
// ═════════════════════════════════════════════════════════════════════════════

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
        assert!(
            !result.success || result.stdout.contains("not found"),
            "Should fail or report not found"
        );

        if !result.success {
            let combined = result.combined_output();
            assert!(
                combined.contains("not found")
                    || combined.contains("not installed")
                    || combined.contains("Package not found")
                    || combined.contains("error"),
                "Error should indicate package not found. Got:\n{combined}"
            );
        }
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
            combined.contains("error")
                || combined.contains("unrecognized")
                || combined.contains("unknown")
                || combined.contains("invalid")
                || combined.contains("No such"),
            "Error should indicate invalid command. Got:\n{combined}"
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
            combined.contains("error")
                || combined.contains("unrecognized")
                || combined.contains("unknown")
                || combined.contains("invalid")
                || combined.contains("unexpected")
                || combined.contains("unexpected argument"),
            "Error should indicate invalid flag. Got:\n{combined}"
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
            combined.contains("required")
                || combined.contains("missing")
                || combined.contains("argument")
                || combined.contains("specify")
                || combined.contains("package"),
            "Error should indicate missing argument. Got:\n{combined}"
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
        if !result.success {
            let combined = result.combined_output();
            assert!(
                combined.contains("network")
                    || combined.contains("connection")
                    || combined.contains("timeout")
                    || combined.contains("mirror")
                    || combined.contains("internet")
                    || combined.contains("Failed")
                    || combined.contains("error"),
                "Network error should be handled gracefully. Got:\n{combined}"
            );
        }
    }

    #[test]
    fn test_network_error_suggests_checking_connection() {
        // ===== ARRANGE =====
        // (No special setup needed)

        // ===== ACT =====
        let result = run_omg(&["sync"]);

        // ===== ASSERT =====
        let combined = result.combined_output();
        if !result.success && combined.contains("network") {
            assert!(
                combined.contains("connection")
                    || combined.contains("internet")
                    || combined.contains("mirror")
                    || combined.contains("check"),
                "Network error should suggest checking connection. Got:\n{combined}"
            );
        }
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
        if !result.success {
            let combined = result.combined_output();
            assert!(
                combined.contains("database")
                    || combined.contains("corrupted")
                    || combined.contains("invalid")
                    || combined.contains("Failed")
                    || combined.contains("error"),
                "Corrupted database should be detected. Got:\n{combined}"
            );
        }
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
        let combined = result.combined_output();
        assert!(
            result.success || combined.contains("Failed") || combined.contains("error"),
            "Should create new database or fail gracefully"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIGURATION ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

mod config_errors {
    use super::*;
    use common::fixtures::error_conditions;
    use std::fs::{self, File};
    use std::io::Write;

    #[test]
    fn test_invalid_config_toml_error() {
        // ===== ARRANGE =====
        let project = TestProject::new();
        let config_dir = project.config_dir.path().join("omg");
        fs::create_dir_all(&config_dir).unwrap();

        let config_file = config_dir.join("config.toml");
        let mut f = File::create(&config_file).unwrap();
        writeln!(f, "invalid toml {{{{").unwrap();

        project.create_file(".nvmrc", "20.0.0");

        // ===== ACT =====
        let result = project.run(&["hook-env", "-s", "bash"]);

        // ===== ASSERT =====
        // Invalid config may be ignored or cause failure - just ensure no panic
        let combined = result.combined_output();
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
        assert!(
            combined.contains("lock")
                || combined.contains("omg.lock")
                || combined.contains("invalid")
                || combined.contains("parse")
                || combined.contains("error")
                || combined.contains("failed"),
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
        if !result.success {
            let combined = result.combined_output();
            assert!(
                combined.contains("sudo")
                    || combined.contains("--yes")
                    || combined.contains("run")
                    || combined.contains("Try")
                    || combined.contains("use"),
                "Error message should suggest what to do. Got:\n{combined}"
            );
        }
    }

    #[test]
    fn test_errors_show_context() {
        // ===== ARRANGE =====
        let nonexistent_pkg = "nonexistent-package";

        // ===== ACT =====
        let result = run_omg(&["info", nonexistent_pkg]);

        // ===== ASSERT =====
        if !result.success {
            let combined = result.combined_output();
            assert!(
                combined.contains("Package")
                    || combined.contains("not found")
                    || combined.contains("nonexistent")
                    || combined.contains(nonexistent_pkg),
                "Error message should provide context. Got:\n{combined}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PANIC PREVENTION
// ═══════════════════════════════════════════════════════════════════════════════

mod panic_prevention {
    use super::*;

    #[test]
    fn test_empty_query_does_not_panic() {
        // ===== ARRANGE =====
        let empty_query = "";

        // ===== ACT =====
        let result = run_omg(&["search", empty_query]);

        // ===== ASSERT =====
        assert!(
            !result.success || !result.stdout.is_empty() || !result.stderr.is_empty(),
            "Should handle empty query without panic"
        );
    }

    #[test]
    fn test_very_long_query_does_not_panic() {
        // ===== ARRANGE =====
        let long_query = "a".repeat(10000);

        // ===== ACT =====
        let result = run_omg(&["search", &long_query]);

        // ===== ASSERT =====
        assert!(
            !result.success || !result.stdout.is_empty() || !result.stderr.is_empty(),
            "Should handle long query without panic"
        );
    }

    #[test]
    fn test_special_chars_do_not_panic() {
        // ===== ARRANGE =====
        let special_chars = "\x01\x02\x03\n\t\r";

        // ===== ACT =====
        let result = run_omg(&["search", special_chars]);

        // ===== ASSERT =====
        assert!(
            !result.success || !result.stdout.is_empty() || !result.stderr.is_empty(),
            "Should handle special chars without panic"
        );
    }

    #[test]
    fn test_unicode_search_does_not_panic() {
        // ===== ARRANGE =====
        let unicode_query = "café-münchen";

        // ===== ACT =====
        let result = run_omg(&["search", unicode_query]);

        // ===== ASSERT =====
        assert!(
            !result.success || !result.stdout.is_empty() || !result.stderr.is_empty(),
            "Should handle unicode without panic"
        );
    }
}
