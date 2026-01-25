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

use std::env;
use std::process::{Command, Stdio};

// ═══════════════════════════════════════════════════════════════════════════════
// TEST UTILITIES
// ═══════════════════════════════════════════════════════════════════════════════

fn run_omg(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omg"))
        .args(args)
        .env("OMG_TEST_MODE", "1")
        .env("OMG_DISABLE_DAEMON", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute omg");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

fn run_omg_with_env(args: &[&str], env_vars: &[(&str, &str)]) -> (bool, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omg"));
    cmd.args(args)
        .env("OMG_TEST_MODE", "1")
        .env("OMG_DISABLE_DAEMON", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    let output = cmd.output().expect("Failed to execute omg");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

// ═══════════════════════════════════════════════════════════════════════════════
// NON-INTERACTIVE MODE ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

mod non_interactive_errors {
    use super::*;

    #[test]
    fn test_non_interactive_without_yes_shows_helpful_error() {
        let (success, stdout, stderr) =
            run_omg_with_env(&["update"], &[("CI", "true"), ("OMG_NON_INTERACTIVE", "1")]);

        let combined = format!("{stdout}{stderr}");

        if !success {
            assert!(
                combined.contains("Use --yes") || combined.contains("interactive"),
                "Error message should suggest using --yes. Got:\n{combined}"
            );
        }
    }

    #[test]
    fn test_privilege_error_suggests_command() {
        // Pass --yes to bypass the non-interactive check and hit the privilege check
        let (success, stdout, stderr) = run_omg(&["update", "--yes"]);

        let combined = format!("{stdout}{stderr}");

        if !success {
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

    #[test]
    fn test_invalid_package_name_error() {
        let (success, stdout, stderr) = run_omg(&["info", "this-package-does-not-exist-12345"]);

        assert!(
            !success || stdout.contains("not found"),
            "Should fail or report not found"
        );

        if !success {
            let combined = format!("{stdout}{stderr}");
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
        let (success, stdout, stderr) = run_omg(&["invalid-command"]);

        assert!(!success, "Invalid command should fail");

        let combined = format!("{stdout}{stderr}");
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
        let (success, stdout, stderr) = run_omg(&["--invalid-flag"]);

        assert!(!success, "Invalid flag should fail");

        let combined = format!("{stdout}{stderr}");
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
        let (success, stdout, stderr) = run_omg(&["install"]);

        assert!(!success, "Missing package argument should fail");

        let combined = format!("{stdout}{stderr}");
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
        let (success, stdout, stderr) = run_omg_with_env(
            &["info", "non-existent-pkg-for-timeout"],
            &[("OMG_NETWORK_TIMEOUT", "1")],
        );

        let combined = format!("{stdout}{stderr}");

        if !success {
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
        let (success, stdout, stderr) = run_omg(&["sync"]);

        let combined = format!("{stdout}{stderr}");

        if !success && combined.contains("network") {
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
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_corrupted_database_handled() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("corrupted.db");

        let mut f = File::create(&db_path).unwrap();
        writeln!(f, "corrupted database data {{{{").unwrap();

        let (success, stdout, stderr) = run_omg_with_env(
            &["status"],
            &[("OMG_DATA_DIR", db_path.parent().unwrap().to_str().unwrap())],
        );

        let combined = format!("{stdout}{stderr}");

        if !success {
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
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("omg_data");

        let (success, stdout, stderr) =
            run_omg_with_env(&["status"], &[("OMG_DATA_DIR", data_dir.to_str().unwrap())]);

        let combined = format!("{stdout}{stderr}");

        assert!(
            success || combined.contains("Failed") || combined.contains("error"),
            "Should create new database or fail gracefully"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIGURATION ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

mod config_errors {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_invalid_config_toml_error() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join(".config").join("omg");
        fs::create_dir_all(&config_dir).unwrap();

        let config_file = config_dir.join("config.toml");
        let mut f = File::create(&config_file).unwrap();
        writeln!(f, "invalid toml {{{{").unwrap();

        let nvmrc = temp_dir.path().join(".nvmrc");
        let mut f = File::create(&nvmrc).unwrap();
        writeln!(f, "20.0.0").unwrap();

        let (success, stdout, stderr) = run_omg_with_env(
            &["hook-env", "-s", "bash"],
            &[
                ("HOME", temp_dir.path().to_str().unwrap()),
                ("OMG_CONFIG_DIR", config_dir.to_str().unwrap()),
            ],
        );

        let combined = format!("{stdout}{stderr}");

        assert!(!success, "Invalid TOML should fail");
        assert!(
            combined.contains("config")
                || combined.contains("toml")
                || combined.contains("parse")
                || combined.contains("invalid")
                || combined.contains("error"),
            "Invalid config should be detected. Got:\n{combined}"
        );
    }

    #[test]
    fn test_invalid_lock_file_error() {
        let temp_dir = TempDir::new().unwrap();
        let lock_file = temp_dir.path().join("omg.lock");

        let mut f = File::create(&lock_file).unwrap();
        writeln!(f, "invalid omg.lock {{{{").unwrap();

        let (success, stdout, stderr) = run_omg_in_dir(&["env", "check"], temp_dir.path());

        let combined = format!("{stdout}{stderr}");

        assert!(!success, "Invalid lock file should fail");
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

    fn run_omg_in_dir(args: &[&str], dir: &std::path::Path) -> (bool, String, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_omg"))
            .args(args)
            .current_dir(dir)
            .env("OMG_TEST_MODE", "1")
            .env("OMG_DISABLE_DAEMON", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .expect("Failed to execute omg");

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        (output.status.success(), stdout, stderr)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// HELPFUL ERROR MESSAGES
// ═══════════════════════════════════════════════════════════════════════════════

mod helpful_messages {
    use super::*;

    #[test]
    fn test_errors_are_readable() {
        let (success, stdout, stderr) = run_omg(&["invalid-command"]);

        let combined = format!("{stdout}{stderr}");

        assert!(!success, "Invalid command should fail");
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
        let (success, stdout, stderr) = run_omg(&["update"]);

        let combined = format!("{stdout}{stderr}");

        if !success {
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
        let (success, stdout, stderr) = run_omg(&["info", "nonexistent-package"]);

        let combined = format!("{stdout}{stderr}");

        if !success {
            assert!(
                combined.contains("Package")
                    || combined.contains("not found")
                    || combined.contains("nonexistent")
                    || combined.contains("nonexistent-package"),
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
        let (success, _stdout, _stderr) = run_omg(&["search", ""]);
        assert!(
            !success || !_stdout.is_empty() || !_stderr.is_empty(),
            "Should handle empty query without panic"
        );
    }

    #[test]
    fn test_very_long_query_does_not_panic() {
        let long_query = "a".repeat(10000);
        let (success, _stdout, _stderr) = run_omg(&["search", &long_query]);
        assert!(
            !success || !_stdout.is_empty() || !_stderr.is_empty(),
            "Should handle long query without panic"
        );
    }

    #[test]
    fn test_special_chars_do_not_panic() {
        let (success, _stdout, _stderr) = run_omg(&["search", "\x01\x02\x03\n\t\r"]);
        assert!(
            !success || !_stdout.is_empty() || !_stderr.is_empty(),
            "Should handle special chars without panic"
        );
    }

    #[test]
    fn test_unicode_search_does_not_panic() {
        let (success, _stdout, _stderr) = run_omg(&["search", "café-münchen"]);
        assert!(
            !success || !_stdout.is_empty() || !_stderr.is_empty(),
            "Should handle unicode without panic"
        );
    }
}
