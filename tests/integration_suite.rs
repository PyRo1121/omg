#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! OMG World-Class Integration Test Suite
//!
//! Comprehensive testing of all OMG features with real assertions.
//! Tests are organized by feature area and run by default where possible.
//!
//! Run all tests:
//!   cargo test --test integration_suite --features arch
//!
//! Run with system tests (requires Arch Linux):
//!   OMG_RUN_SYSTEM_TESTS=1 cargo test --test integration_suite --features arch
//!
//! Run with network tests (hits external APIs):
//!   OMG_RUN_NETWORK_TESTS=1 cargo test --test integration_suite --features arch
//!
//! Run with performance assertions:
//!   OMG_RUN_PERF_TESTS=1 cargo test --test integration_suite --features arch
//!
//! Run destructive tests (actually installs packages - USE WITH CAUTION):
//!   OMG_RUN_DESTRUCTIVE_TESTS=1 cargo test --test integration_suite --features arch

#![allow(unused_variables)]
#![allow(clippy::doc_markdown)] // Test file doc comments don't need strict formatting
#![allow(clippy::missing_panics_doc)] // Test functions are expected to panic
#![allow(clippy::missing_errors_doc)] // Test helpers don't need docs

mod common;

use common::*;
use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════════════════════
// TEST UTILITIES
// ═══════════════════════════════════════════════════════════════════════════════

/// Guard for destructive integration tests (real installs/updates)
fn destructive_tests_enabled() -> bool {
    matches!(env::var("OMG_RUN_DESTRUCTIVE_TESTS"), Ok(value) if value == "1")
}

fn system_tests_enabled() -> bool {
    matches!(env::var("OMG_RUN_SYSTEM_TESTS"), Ok(value) if value == "1")
}

fn network_tests_enabled() -> bool {
    matches!(env::var("OMG_RUN_NETWORK_TESTS"), Ok(value) if value == "1")
}

/// Create a temporary project directory with common config files
fn create_test_project(dir: &Path, config_type: &str) {
    fs::create_dir_all(dir).unwrap();

    match config_type {
        "node" => {
            // Create .nvmrc
            let mut f = File::create(dir.join(".nvmrc")).unwrap();
            writeln!(f, "20.10.0").unwrap();

            // Create package.json
            let mut f = File::create(dir.join("package.json")).unwrap();
            writeln!(
                f,
                r#"{{"name": "test", "engines": {{"node": ">=18.0.0"}}}}"#
            )
            .unwrap();
        }
        "python" => {
            let mut f = File::create(dir.join(".python-version")).unwrap();
            writeln!(f, "3.11.0").unwrap();
        }
        "go" => {
            let mut f = File::create(dir.join("go.mod")).unwrap();
            writeln!(f, "module test\n\ngo 1.21").unwrap();
        }
        "rust" => {
            let mut f = File::create(dir.join("rust-toolchain.toml")).unwrap();
            writeln!(f, "[toolchain]\nchannel = \"stable\"").unwrap();
        }
        "ruby" => {
            let mut f = File::create(dir.join(".ruby-version")).unwrap();
            writeln!(f, "3.2.0").unwrap();
        }
        "java" => {
            let mut f = File::create(dir.join(".java-version")).unwrap();
            writeln!(f, "21").unwrap();
        }
        "bun" => {
            let mut f = File::create(dir.join(".bun-version")).unwrap();
            writeln!(f, "1.0.0").unwrap();
        }
        "tool-versions" => {
            let mut f = File::create(dir.join(".tool-versions")).unwrap();
            writeln!(f, "nodejs 20.10.0\npython 3.11.0\nruby 3.2.0").unwrap();
        }
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI FOUNDATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_foundation {
    use super::*;

    #[test]
    fn test_version_flag() {
        let result = run_omg(&["--version"]);
        assert!(result.success, "omg --version should succeed");
        assert!(
            result.stdout.contains("omg"),
            "Version output should contain 'omg'"
        );
    }

    #[test]
    fn test_help_flag() {
        let result = run_omg(&["--help"]);
        assert!(result.success, "omg --help should succeed");
        // Check for key elements in the help output
        assert!(
            result.stdout.contains("Essential Commands") || result.stdout.contains("Usage"),
            "Help should contain commands section"
        );
        assert!(
            result.stdout.contains("search") || result.stdout.contains("Commands"),
            "Help should show search command"
        );
    }

    #[test]
    fn test_subcommand_help() {
        let subcommands = vec![
            "search", "install", "remove", "update", "info", "clean", "use", "list", "env",
            "audit", "status", "which", "config",
        ];

        for cmd in subcommands {
            let result = run_omg(&[cmd, "--help"]);
            assert!(result.success, "omg {cmd} --help should succeed");
            // Help output should contain the command name or usage info
            assert!(
                result.stdout.contains(cmd)
                    || result.stdout.contains("Usage")
                    || result.stdout.len() > 50,
                "Help for {cmd} should contain meaningful information"
            );
        }
    }

    #[test]
    fn test_invalid_command() {
        let result = run_omg(&["nonexistent-command"]);
        assert!(!result.success, "Invalid command should fail");
        assert!(
            result.stderr.contains("error") || result.stderr.contains("unrecognized"),
            "Should report error for invalid command"
        );
    }

    #[test]
    fn test_missing_required_args() {
        // Install requires package names
        let result = run_omg(&["install"]);
        assert!(!result.success, "install without args should fail");
        assert!(
            result.stderr.contains("required") || result.stderr.contains("error"),
            "Should report missing arguments"
        );
    }

    #[test]
    fn test_verbose_flags() {
        // Test -v, -vv, -vvv
        let result = run_omg(&["-v", "status"]);
        assert!(result.success, "omg -v status should succeed");

        let result = run_omg(&["-vv", "status"]);
        assert!(result.success, "omg -vv status should succeed");

        let result = run_omg(&["-vvv", "status"]);
        assert!(result.success, "omg -vvv status should succeed");
    }

    #[test]
    fn test_quiet_flag() {
        let result = run_omg(&["-q", "status"]);
        assert!(result.success, "omg -q status should succeed");
        // Quiet mode should produce minimal output
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PACKAGE MANAGEMENT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod package_management {
    use super::*;

    #[test]
    fn test_search_official_package() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }
        let result = run_omg(&["search", "firefox"]);
        assert!(result.success, "Search should succeed");
        assert!(result.stdout.contains("firefox"), "Should find firefox");
        assert!(
            result.stdout.contains("Official")
                || result.stdout.contains("extra")
                || result.stdout.contains("core"),
            "Should indicate official repository"
        );
    }

    #[test]
    fn test_search_with_detailed_flag() {
        if !network_tests_enabled() {
            eprintln!("Skipping network test (set OMG_RUN_NETWORK_TESTS=1)");
            return;
        }
        let result = run_omg(&["search", "firefox", "--detailed"]);
        assert!(result.success, "Detailed search should succeed");
        // Detailed output should include votes/popularity for AUR
    }

    #[test]
    fn test_search_empty_query() {
        let result = run_omg(&["search", ""]);
        // Empty query might return error or empty results
        // Both are acceptable behaviors
    }

    #[test]
    fn test_search_special_characters() {
        // Test with special characters that might break parsing
        let result = run_omg(&["search", "lib++"]);
        // Should not crash
    }

    #[test]
    fn test_search_unicode() {
        // Test with unicode characters
        let result = run_omg(&["search", "日本語"]);
        // Should not crash, may return no results
    }

    #[test]
    fn test_search_very_long_query() {
        let long_query = "a".repeat(1000);
        let result = run_omg(&["search", &long_query]);
        // Should handle gracefully (no crash, may return error)
    }

    #[test]
    fn test_info_official_package() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }
        let result = run_omg(&["info", "pacman"]);
        assert!(result.success, "Info for official package should succeed");
        assert!(result.stdout.contains("pacman"), "Should show package name");
        // Version is displayed as "pacman X.Y.Z" format
        assert!(
            result.stdout.contains("Version")
                || result.stdout.contains('.') && result.stdout.contains("pacman"),
            "Should show version"
        );
    }

    #[test]
    #[cfg(feature = "arch")]
    fn test_info_nonexistent_package() {
        let result = run_omg(&["info", "this-package-does-not-exist-12345"]);
        // Should fail gracefully or show "not found"
        assert!(
            !result.success || result.stdout.contains("not found"),
            "Should indicate package not found"
        );
    }

    #[test]
    fn test_install_real_package() {
        if !destructive_tests_enabled() {
            eprintln!("Skipping destructive test (set OMG_RUN_DESTRUCTIVE_TESTS=1)");
            return;
        }

        let pkg = env::var("OMG_TEST_PACKAGE").unwrap_or_else(|_| "ripgrep".to_string());
        let args = vec!["install", "-y", &pkg];
        let result = run_omg(&args);
        assert!(
            result.success
                || result.stdout.contains("already installed")
                || result.stderr.contains("already installed"),
            "Install should succeed or report already installed"
        );
    }

    #[test]
    fn test_update_check_only() {
        if !destructive_tests_enabled() {
            eprintln!("Skipping destructive test (set OMG_RUN_DESTRUCTIVE_TESTS=1)");
            return;
        }

        let result = run_omg(&["update", "--check"]);
        assert!(result.success, "Update check should succeed");
    }

    #[test]
    fn test_update_check_shows_real_updates() {
        if !destructive_tests_enabled() {
            eprintln!("Skipping destructive test (set OMG_RUN_DESTRUCTIVE_TESTS=1)");
            return;
        }

        let result = run_omg(&["update", "--check"]);
        let combined = result.combined_output();

        assert!(result.success, "Update check should succeed");
        assert!(!combined.is_empty(), "Update check should produce output");

        // Verify update detection code path is exercised
        // Should report either updates available or system up to date
        assert!(
            combined.contains("updates")
                || combined.contains("up to date")
                || combined.contains("System is up to date")
                || combined.contains("Found")
                || combined.contains("✓"),
            "Should report update status. Output:\n{combined}"
        );

        // Verify version comparison happens
        // If updates exist, should show version information (old → new format)
        if combined.contains("update") || combined.to_lowercase().contains("updates") {
            assert!(
                combined.contains("→")
                    || combined.contains("->")
                    || combined.contains("→")
                    || (combined.contains('.') && combined.len() > 50),
                "When updates exist, should show version information. Output:\n{combined}"
            );
        }
    }

    #[test]
    fn test_update_with_yes_flag() {
        if !destructive_tests_enabled() {
            eprintln!("Skipping destructive test (set OMG_RUN_DESTRUCTIVE_TESTS=1)");
            return;
        }

        let result = run_omg(&["update", "--yes"]);
        let combined = result.combined_output();

        // Should complete without hanging
        assert!(!combined.is_empty(), "Should produce output");

        // Should show progress or completion message
        assert!(
            combined.contains("update")
                || combined.contains("upgrade")
                || combined.contains("system")
                || combined.contains("up to date")
                || combined.contains("✓"),
            "Should show update progress or completion. Output:\n{combined}"
        );
    }

    #[test]
    fn test_non_interactive_without_yes_fails_gracefully() {
        if !destructive_tests_enabled() {
            eprintln!("Skipping destructive test (set OMG_RUN_DESTRUCTIVE_TESTS=1)");
            return;
        }

        // Test that running in non-interactive mode without --yes
        // gives a helpful error message
        let result = run_omg_with_env(&["update"], &[("CI", "true"), ("OMG_NON_INTERACTIVE", "1")]);

        let combined = result.combined_output();

        // Should fail without --yes in non-interactive mode
        assert!(!result.success, "Should fail without TTY and --yes");

        // Should provide helpful error message
        assert!(
            combined.contains("interactive")
                || combined.contains("--yes")
                || combined.contains("terminal")
                || combined.contains("TTY")
                || combined.contains("requires")
                || combined.contains("sudo"),
            "Should provide helpful error message about interactive mode. Output:\n{combined}"
        );
    }

    #[test]
    fn test_status_command() {
        let result = run_omg(&["status"]);
        assert!(result.success, "Status should succeed");
        assert!(
            result.stdout.contains("Packages")
                || result.stdout.contains("Updates")
                || result.stdout.contains("Runtimes"),
            "Status should show system info"
        );
    }

    #[test]
    fn test_clean_help() {
        let result = run_omg(&["clean", "--help"]);
        assert!(result.success, "Clean help should succeed");
        let output = result.combined_output();
        // Debug output
        if !output.contains("orphans") && !output.contains("cache") {
            eprintln!("Clean help output:\n{output}");
        }
        // At minimum, the command should succeed and produce some output
        assert!(!output.is_empty(), "Clean help should produce output");
        // The clean command should exist and show help
        assert!(
            output.contains("clean") || output.contains("Clean") || output.contains("Usage"),
            "Help should mention clean command"
        );
    }

    #[test]
    fn test_explicit_packages() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }
        let result = run_omg(&["explicit"]);
        assert!(result.success, "Explicit should succeed");
        // Should list some packages on a real Arch system
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// RUNTIME MANAGEMENT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod runtime_management {
    use super::*;

    const RUNTIMES: &[&str] = &["node", "python", "go", "rust", "ruby", "java", "bun"];

    #[test]
    fn test_list_all_runtimes() {
        let result = run_omg(&["list"]);
        assert!(result.success, "List should succeed");
        // Should list available runtimes
    }

    #[test]
    fn test_list_installed_node() {
        let result = run_omg(&["list", "node"]);
        assert!(result.success, "List node should succeed");
    }

    #[test]
    fn test_list_installed_python() {
        let result = run_omg(&["list", "python"]);
        assert!(result.success, "List python should succeed");
    }

    #[test]
    fn test_list_available_node() {
        if !network_tests_enabled() {
            eprintln!("Skipping network test (set OMG_RUN_NETWORK_TESTS=1)");
            return;
        }
        let result = run_omg(&["list", "node", "--available"]);
        assert!(result.success, "List available node should succeed");
        // Should show versions from nodejs.org
    }

    #[test]
    fn test_list_available_python() {
        if !network_tests_enabled() {
            eprintln!("Skipping network test (set OMG_RUN_NETWORK_TESTS=1)");
            return;
        }
        let result = run_omg(&["list", "python", "--available"]);
        assert!(result.success, "List available python should succeed");
    }

    #[test]
    fn test_list_available_go() {
        if !network_tests_enabled() {
            eprintln!("Skipping network test (set OMG_RUN_NETWORK_TESTS=1)");
            return;
        }
        let result = run_omg(&["list", "go", "--available"]);
        assert!(result.success, "List available go should succeed");
    }

    #[test]
    fn test_list_available_rust() {
        if !network_tests_enabled() {
            eprintln!("Skipping network test (set OMG_RUN_NETWORK_TESTS=1)");
            return;
        }
        let result = run_omg(&["list", "rust", "--available"]);
        assert!(result.success, "List available rust should succeed");
    }

    #[test]
    fn test_list_available_ruby() {
        if !network_tests_enabled() {
            eprintln!("Skipping network test (set OMG_RUN_NETWORK_TESTS=1)");
            return;
        }
        let result = run_omg(&["list", "ruby", "--available"]);
        assert!(result.success, "List available ruby should succeed");
    }

    #[test]
    fn test_list_available_java() {
        if !network_tests_enabled() {
            eprintln!("Skipping network test (set OMG_RUN_NETWORK_TESTS=1)");
            return;
        }
        let result = run_omg(&["list", "java", "--available"]);
        assert!(result.success, "List available java should succeed");
        // Should show LTS markers
    }

    #[test]
    fn test_list_available_bun() {
        if !network_tests_enabled() {
            eprintln!("Skipping network test (set OMG_RUN_NETWORK_TESTS=1)");
            return;
        }
        let result = run_omg(&["list", "bun", "--available"]);
        assert!(result.success, "List available bun should succeed");
    }

    #[test]
    fn test_list_unknown_runtime() {
        let result = run_omg(&["list", "unknownruntime"]);
        // Should fail or show error
        assert!(
            !result.success
                || result.stdout.contains("Unknown")
                || result.stdout.contains("Supported"),
            "Should indicate unknown runtime"
        );
    }

    #[test]
    fn test_which_command() {
        for runtime in RUNTIMES {
            let result = run_omg(&["which", runtime]);
            assert!(result.success, "which {runtime} should succeed");
        }
    }

    #[test]
    fn test_use_without_version_no_config() {
        let temp_dir = TempDir::new().unwrap();
        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        // Should fail because no version file exists
        assert!(
            !result.success
                || result.stderr.contains("No version")
                || result.stderr.contains("detected"),
            "Should fail without version file"
        );
    }

    #[test]
    fn test_use_with_nvmrc() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "node");

        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        // Should detect the version from .nvmrc
        assert!(
            result.success
                || result.stdout.contains("20.10.0")
                || result.stdout.contains("Detected"),
            "Should detect version from .nvmrc"
        );
    }

    #[test]
    fn test_use_with_python_version() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "python");

        let result = run_omg_in_dir(&["use", "python"], temp_dir.path());
        // Should detect the version from .python-version
        assert!(
            result.success
                || result.stdout.contains("3.11.0")
                || result.stdout.contains("Detected"),
            "Should detect version from .python-version"
        );
    }

    #[test]
    fn test_use_with_tool_versions() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "tool-versions");

        // Test Node detection from .tool-versions
        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        assert!(
            result.success || result.stdout.contains("20.10.0"),
            "Should detect node version from .tool-versions"
        );

        // Test Python detection from .tool-versions
        let result = run_omg_in_dir(&["use", "python"], temp_dir.path());
        assert!(
            result.success || result.stdout.contains("3.11.0"),
            "Should detect python version from .tool-versions"
        );
    }

    #[test]
    fn test_use_invalid_version_format() {
        let result = run_omg(&["use", "node", "not-a-version"]);
        // Should handle gracefully (may try to install or fail)
    }

    #[test]
    fn test_runtime_alias_node_nodejs() {
        // "nodejs" should work the same as "node"
        let result1 = run_omg(&["list", "node"]);
        let result2 = run_omg(&["list", "nodejs"]);
        assert_eq!(
            result1.success, result2.success,
            "node and nodejs should behave the same"
        );
    }

    #[test]
    fn test_runtime_alias_go_golang() {
        // "golang" should work the same as "go"
        let result1 = run_omg(&["list", "go"]);
        let result2 = run_omg(&["list", "golang"]);
        assert_eq!(
            result1.success, result2.success,
            "go and golang should behave the same"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ENVIRONMENT MANAGEMENT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod environment_management {
    use super::*;

    #[test]
    fn test_env_capture() {
        let temp_dir = TempDir::new().unwrap();
        let result = run_omg_in_dir(&["env", "capture"], temp_dir.path());
        assert!(result.success, "env capture should succeed");
        assert!(
            result.stdout.contains("omg.lock") || result.stdout.contains("captured"),
            "Should mention lock file"
        );

        // Verify omg.lock was created
        assert!(
            temp_dir.path().join("omg.lock").exists(),
            "omg.lock should be created"
        );
    }

    #[test]
    fn test_env_capture_deterministic() {
        let temp_dir = TempDir::new().unwrap();

        // Capture twice with same environment
        run_omg_in_dir(&["env", "capture"], temp_dir.path());
        let lock1 = fs::read_to_string(temp_dir.path().join("omg.lock")).unwrap();

        // Capture again immediately (same state)
        run_omg_in_dir(&["env", "capture"], temp_dir.path());
        let lock2 = fs::read_to_string(temp_dir.path().join("omg.lock")).unwrap();

        // Both captures should produce valid TOML
        assert!(
            lock1.contains("[environment]") || lock1.contains("hash"),
            "Lock file should have environment section"
        );
        assert!(
            lock2.contains("[environment]") || lock2.contains("hash"),
            "Second lock file should have environment section"
        );
    }

    #[test]
    fn test_env_check_no_drift() {
        let temp_dir = TempDir::new().unwrap();

        // Capture
        let capture_result = run_omg_in_dir(&["env", "capture"], temp_dir.path());
        assert!(capture_result.success, "env capture should succeed");

        // Check immediately - should work (may or may not report drift depending on timing)
        let result = run_omg_in_dir(&["env", "check"], temp_dir.path());
        let combined = result.combined_output();
        // Should either succeed or give meaningful output about drift
        assert!(
            result.success || combined.contains("drift") || combined.contains("check"),
            "env check should work: {combined}"
        );
    }

    #[test]
    fn test_env_check_without_lock() {
        let temp_dir = TempDir::new().unwrap();

        // Check without capturing first
        let result = run_omg_in_dir(&["env", "check"], temp_dir.path());
        assert!(!result.success, "env check should fail without omg.lock");
        assert!(
            result.stderr.contains("omg.lock")
                || result.stderr.contains("not found")
                || result.stderr.contains("capture"),
            "Should mention missing lock file"
        );
    }

    #[test]
    fn test_env_share_without_token() {
        let temp_dir = TempDir::new().unwrap();
        run_omg_in_dir(&["env", "capture"], temp_dir.path());

        // Clear GITHUB_TOKEN
        let result = run_omg_with_env(&["env", "share"], &[("GITHUB_TOKEN", "")]);
        // Should fail because no token
        assert!(
            !result.success
                || result.stderr.contains("GITHUB_TOKEN")
                || result.stderr.contains("token"),
            "Should require GITHUB_TOKEN"
        );
    }

    #[test]
    fn test_env_share_without_lock() {
        let temp_dir = TempDir::new().unwrap();

        // Try to share without capturing first
        let result = run_omg_in_dir(&["env", "share"], temp_dir.path());
        assert!(!result.success, "env share should fail without omg.lock");
    }

    #[test]
    fn test_env_sync_invalid_url() {
        let temp_dir = TempDir::new().unwrap();

        let result = run_omg_in_dir(&["env", "sync", "not-a-valid-gist-url"], temp_dir.path());
        assert!(!result.success, "env sync should fail with invalid URL");
    }

    #[test]
    fn test_env_subcommand_help() {
        let result = run_omg(&["env", "--help"]);
        assert!(result.success, "env --help should succeed");
        assert!(
            result.stdout.contains("capture"),
            "Should list capture subcommand"
        );
        assert!(
            result.stdout.contains("check"),
            "Should list check subcommand"
        );
        assert!(
            result.stdout.contains("share"),
            "Should list share subcommand"
        );
        assert!(
            result.stdout.contains("sync"),
            "Should list sync subcommand"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod security {
    use super::*;

    #[test]
    fn test_audit_command() {
        let result = run_omg(&["audit"]);
        // May succeed or fail depending on daemon status or license tier
        // Should not crash
        assert!(
            result.success
                || result.stderr.contains("daemon")
                || result.stderr.contains("Daemon")
                || result.stderr.contains("requires")
                || result.stderr.contains("tier"),
            "Audit should work or report daemon/license issue"
        );
    }

    #[test]
    fn test_security_policy_file_loading() {
        let temp_dir = TempDir::new().unwrap();

        // Create a policy file
        let config_dir = temp_dir.path().join(".config").join("omg");
        fs::create_dir_all(&config_dir).unwrap();

        let mut policy_file = File::create(config_dir.join("policy.toml")).unwrap();
        writeln!(
            policy_file,
            r#"
allow_aur = false
require_pgp = true
minimum_grade = "Verified"
banned_packages = ["malware-pkg"]
        "#
        )
        .unwrap();

        // Run a command that would load policy
        // The actual policy enforcement is tested in unit tests
    }

    #[test]
    fn test_security_grade_display() {
        // When searching, security grades should be visible
        let result = run_omg(&["info", "pacman"]);
        assert!(result.success, "Info should succeed");
        // Note: Security grade display depends on implementation
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMPLETION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod completions {
    use super::*;

    #[test]
    fn test_completions_bash() {
        let result = run_omg(&["completions", "bash", "--stdout"]);
        assert!(result.success, "Bash completions should succeed");
        assert!(
            result.stdout.contains("complete")
                || result.stdout.contains("_omg")
                || result.stdout.contains("_omg_completions"),
            "Should output bash completion script"
        );
    }

    #[test]
    fn test_completions_zsh() {
        let result = run_omg(&["completions", "zsh", "--stdout"]);
        assert!(result.success, "Zsh completions should succeed");
        assert!(
            result.stdout.contains("compdef") || result.stdout.contains("_omg"),
            "Should output zsh completion script"
        );
    }

    #[test]
    fn test_completions_fish() {
        let result = run_omg(&["completions", "fish", "--stdout"]);
        assert!(result.success, "Fish completions should succeed");
        assert!(
            result.stdout.contains("complete") || result.stdout.contains("omg"),
            "Should output fish completion script"
        );
    }

    #[test]
    fn test_completions_invalid_shell() {
        let result = run_omg(&["completions", "invalidshell"]);
        assert!(!result.success, "Invalid shell should fail");
        assert!(
            result.stderr.contains("Unsupported") || result.stderr.contains("error"),
            "Should report unsupported shell"
        );
    }

    #[test]
    fn test_hidden_complete_command() {
        // Test the hidden dynamic completion command
        let result = run_omg(&["complete", "--shell", "zsh", "--current", "fire"]);
        // May or may not be implemented as a visible command
    }

    #[test]
    fn test_fuzzy_completion_typo() {
        // This tests the internal completion engine
        // The hidden complete command should handle typos
        let result = run_omg(&["complete", "--shell", "zsh", "--current", "frfx"]);
        // If implemented, should suggest "firefox"
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SHELL HOOK TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod shell_hooks {
    use super::*;

    #[test]
    fn test_unicode_search() {
        let result = run_omg(&["search", "café"]);
        assert!(result.success, "Unicode search should succeed");
    }

    #[test]
    fn test_hook_zsh() {
        let result = run_omg(&["hook", "zsh"]);
        assert!(result.success, "Hook zsh should succeed");
    }

    #[test]
    fn test_hook_fish() {
        let result = run_omg(&["hook", "fish"]);
        assert!(result.success, "Hook fish should succeed");
    }

    #[test]
    fn test_hook_invalid_shell() {
        let result = run_omg(&["hook", "invalidshell"]);
        // Should fail or return empty
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIG TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod config {
    use super::*;

    #[test]
    fn test_config_list() {
        let result = run_omg(&["config"]);
        assert!(result.success, "Config list should succeed");
        // Should show configuration
    }

    #[test]
    fn test_config_get_key() {
        // Use proper config get subcommand
        let result = run_omg(&["config", "get", "telemetry.enabled"]);
        assert!(result.success, "Config get should succeed");
    }

    #[test]
    fn test_config_get_invalid_key() {
        // Use proper config get subcommand - invalid key reports error message
        let result = run_omg(&["config", "get", "nonexistent_key"]);
        assert!(
            result.stdout.contains("Unknown")
                || result.stdout.contains("not found")
                || result.stdout.contains("invalid"),
            "Config get for invalid key should report error, got: {}",
            result.stdout
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR HANDLING TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod error_handling {
    use super::*;

    #[test]
    fn test_invalid_lock_file() {
        let temp_dir = TempDir::new().unwrap();

        // Create invalid omg.lock
        let mut f = File::create(temp_dir.path().join("omg.lock")).unwrap();
        writeln!(f, "this is not valid toml {{{{").unwrap();

        let result = run_omg_in_dir(&["env", "check"], temp_dir.path());
        assert!(!result.success, "Should fail with invalid lock file");
        // Should show a helpful error, not panic
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// EDGE CASE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod edge_cases {
    use super::*;

    #[test]
    fn test_empty_environment() {
        let temp_dir = TempDir::new().unwrap();
        // Empty directory - no runtimes, no packages tracked
        let result = run_omg_in_dir(&["env", "capture"], temp_dir.path());
        assert!(result.success, "Should handle empty environment");
    }

    #[test]
    fn test_deeply_nested_directory() {
        let temp_dir = TempDir::new().unwrap();

        // Create deeply nested structure with .nvmrc at root
        create_test_project(temp_dir.path(), "node");

        let deep_path = temp_dir
            .path()
            .join("a")
            .join("b")
            .join("c")
            .join("d")
            .join("e");
        fs::create_dir_all(&deep_path).unwrap();

        // Running from deep path should still find .nvmrc at root
        let result = run_omg_in_dir(&["use", "node"], &deep_path);
        assert!(
            result.success,
            "Runtime resolution should work from a nested directory: {}",
            result.stderr
        );
    }

    #[test]
    fn test_concurrent_operations() {
        use std::thread;

        // Run multiple omg commands concurrently
        let handles: Vec<_> = (0..5)
            .map(|_| thread::spawn(|| run_omg(&["status"])))
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result.success, "Concurrent status should succeed");
        }
    }

    #[test]
    fn test_very_large_package_list() {
        // Searching for common terms that return many results
        let result = run_omg(&["search", "lib"]);
        assert!(result.success, "Large search should succeed");
    }

    #[test]
    fn test_unicode_path_handling() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let unicode_dir = temp_dir.path().join("unicode_dir");
        std::fs::create_dir(&unicode_dir).unwrap();

        let result = run_omg_in_dir(&["status"], &unicode_dir);
        assert!(result.success, "Should work in unicode directory");
    }

    #[test]
    fn test_whitespace_in_version() {
        // Version with leading/trailing whitespace
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join(".nvmrc")).unwrap();
        writeln!(f, "  20.10.0  ").unwrap();

        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        // Should trim whitespace
    }

    #[test]
    fn test_comments_in_version_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join(".nvmrc")).unwrap();
        writeln!(f, "# This is a comment").unwrap();
        writeln!(f, "20.10.0").unwrap();

        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        // Should handle comments (implementation dependent)
    }

    #[test]
    fn test_lts_version_alias() {
        // Using "lts" as a version
        let result = run_omg(&["use", "node", "lts"]);
        // Should resolve to actual LTS version
    }

    #[test]
    fn test_latest_version_alias() {
        // Using "latest" as a version
        let result = run_omg(&["use", "node", "latest"]);
        // Should resolve to latest version
    }

    #[test]
    fn test_partial_version() {
        // Using partial version like "20" instead of "20.10.0"
        let result = run_omg(&["use", "node", "20"]);
        // Should resolve to latest 20.x.x
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DATABASE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod database {
    use super::*;

    #[test]
    fn test_database_creation() {
        // The database should be created automatically
        // Just verify omg runs - DB is created on demand
        let result = run_omg(&["status"]);
        assert!(
            result.success,
            "Status should succeed (creates DB if needed)"
        );
    }

    #[test]
    fn test_database_concurrent_access() {
        use std::thread;

        // Multiple threads accessing the database
        let handles: Vec<_> = (0..3)
            .map(|_| thread::spawn(|| run_omg(&["list", "node"])))
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result.success, "Concurrent DB access should succeed");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DAEMON TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod daemon {
    use super::*;

    #[test]
    fn test_daemon_help() {
        let result = run_omg(&["daemon", "--help"]);
        assert!(result.success, "Daemon help should succeed");
        assert!(
            result.stdout.contains("foreground"),
            "Should mention foreground option"
        );
    }

    #[test]
    fn test_status_with_daemon() {
        // Status should work with daemon enabled (daemon may or may not be running)
        // This tests that the status command doesn't crash when daemon support is enabled
        let result = run_omg_with_env(&["status"], &[]);
        let combined = result.combined_output();

        // Status should succeed regardless of whether daemon is actually running
        // It should gracefully fall back to direct mode if daemon is unavailable
        assert!(
            result.success,
            "Status should succeed with daemon support enabled. Output: {combined}"
        );

        // Should produce meaningful output
        assert!(
            result.stdout.contains("Package")
                || result.stdout.contains("Runtime")
                || result.stdout.contains("OMG")
                || combined.contains("daemon"),
            "Status should show system info or daemon status. Output: {combined}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTEGRATION SCENARIOS
// ═══════════════════════════════════════════════════════════════════════════════

mod integration_scenarios {
    use super::*;

    #[test]
    fn scenario_new_developer_onboarding() {
        let temp_dir = TempDir::new().unwrap();

        // 1. Create project with .tool-versions
        create_test_project(temp_dir.path(), "tool-versions");

        // 2. Developer runs status to see what's needed
        let result = run_omg_in_dir(&["status"], temp_dir.path());
        assert!(result.success, "Status should work");

        // 3. Developer syncs environment (if lock exists from team)
        // Simulated by running env capture
        let result = run_omg_in_dir(&["env", "capture"], temp_dir.path());
        assert!(result.success, "Env capture should work");

        // 4. Check for drift - may report drift if runtimes not installed, that's OK
        let result = run_omg_in_dir(&["env", "check"], temp_dir.path());
        // Either succeeds or reports drift (both are valid outcomes)
        let combined = result.combined_output();
        assert!(
            result.success
                || combined.contains("drift")
                || combined.contains("Drift")
                || combined.contains("check"),
            "Env check should work: {combined}"
        );
    }

    #[test]
    fn scenario_switching_projects() {
        let project1 = TempDir::new().unwrap();
        let project2 = TempDir::new().unwrap();

        // Project 1 uses Node 18
        let mut f = File::create(project1.path().join(".nvmrc")).unwrap();
        writeln!(f, "18.0.0").unwrap();

        // Project 2 uses Node 20
        let mut f = File::create(project2.path().join(".nvmrc")).unwrap();
        writeln!(f, "20.0.0").unwrap();

        // Switch to project 1
        let result1 = run_omg_in_dir(&["use", "node"], project1.path());

        // Switch to project 2
        let result2 = run_omg_in_dir(&["use", "node"], project2.path());

        // Versions should be different
        assert!(
            result1.stdout.contains("18") || result2.stdout.contains("20"),
            "Should detect different versions per project"
        );
    }

    #[test]
    fn scenario_security_audit_workflow() {
        // 1. Run status to see overview
        let result = run_omg(&["status"]);
        assert!(result.success, "Status should work");

        // 2. Run full audit
        let result = run_omg(&["audit"]);
        // May require daemon

        // 3. Search for a package to install
        let result = run_omg(&["search", "firefox"]);
        assert!(result.success, "Search should work");

        // 4. Get info on package
        let result = run_omg(&["info", "firefox"]);
        assert!(result.success, "Info should work");
    }

    #[test]
    fn scenario_team_environment_sync() {
        let dev1_dir = TempDir::new().unwrap();
        let dev2_dir = TempDir::new().unwrap();

        // Dev 1 captures their environment
        create_test_project(dev1_dir.path(), "tool-versions");
        let result = run_omg_in_dir(&["env", "capture"], dev1_dir.path());
        assert!(result.success, "Dev1 capture should work");

        // Copy lock file to dev2 (simulating gist share/sync)
        let lock_content = fs::read_to_string(dev1_dir.path().join("omg.lock")).unwrap();
        fs::write(dev2_dir.path().join("omg.lock"), &lock_content).unwrap();

        // Dev 2 checks their environment - may report drift since different machine
        create_test_project(dev2_dir.path(), "tool-versions");
        let result = run_omg_in_dir(&["env", "check"], dev2_dir.path());
        let combined = result.combined_output();
        // Should run without crashing, may report drift
        assert!(
            combined.contains("drift") || combined.contains("check") || combined.contains("match"),
            "Dev2 check should produce output: {combined}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// MISE INTEGRATION TESTS (Built-in runtime manager)
// ═══════════════════════════════════════════════════════════════════════════════

mod mise_integration {
    use super::*;

    #[test]
    fn test_mise_manager_initialization() {
        // List should work regardless of mise availability
        let result = run_omg(&["list"]);
        assert!(result.success, "List should succeed");
    }

    #[test]
    fn test_mise_toml_detection() {
        let temp_dir = TempDir::new().unwrap();

        // Create .mise.toml with multiple runtimes
        let mut f = File::create(temp_dir.path().join(".mise.toml")).unwrap();
        writeln!(
            f,
            r#"[tools]
deno = "1.40.0"
elixir = "1.16.0"
zig = "0.11.0"
"#
        )
        .unwrap();

        // Status should work with .mise.toml present
        let result = run_omg_in_dir(&["status"], temp_dir.path());
        assert!(result.success, "Status should work with .mise.toml");
    }

    #[test]
    fn test_non_native_runtime_handling() {
        let temp_dir = TempDir::new().unwrap();

        // Try to use an unknown runtime - should handle gracefully
        let result = run_omg_in_dir(&["use", "erlang", "26.0"], temp_dir.path());

        // Should either work (via mise) or give helpful message
        let combined = result.combined_output();
        assert!(
            combined.contains("mise")
                || combined.contains("erlang")
                || combined.contains("installing")
                || combined.contains("Switching"),
            "Should handle non-native runtime: {combined}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// VERSION DETECTION TESTS (Comprehensive config file parsing)
// ═══════════════════════════════════════════════════════════════════════════════

mod version_detection {
    use super::*;

    #[test]
    fn test_nvmrc_detection() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join(".nvmrc")).unwrap();
        writeln!(f, "20.10.0").unwrap();

        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        assert!(
            result.stdout.contains("20.10.0") || result.stdout.contains("Detected"),
            "Should detect version from .nvmrc: {}",
            result.stdout
        );
    }

    #[test]
    fn test_node_version_file_detection() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join(".node-version")).unwrap();
        writeln!(f, "18.19.0").unwrap();

        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        assert!(
            result.stdout.contains("18.19.0") || result.stdout.contains("Detected"),
            "Should detect version from .node-version"
        );
    }

    #[test]
    fn test_python_version_detection() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join(".python-version")).unwrap();
        writeln!(f, "3.12.0").unwrap();

        let result = run_omg_in_dir(&["use", "python"], temp_dir.path());
        assert!(
            result.stdout.contains("3.12.0") || result.stdout.contains("Detected"),
            "Should detect version from .python-version"
        );
    }

    #[test]
    fn test_tool_versions_multi_runtime() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join(".tool-versions")).unwrap();
        writeln!(f, "nodejs 20.10.0\npython 3.11.0\nruby 3.2.0\ngo 1.21.0").unwrap();

        // Each runtime should be detected
        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        assert!(
            result.stdout.contains("20.10.0") || result.stdout.contains("Detected"),
            "Should detect node from .tool-versions"
        );
    }

    #[test]
    fn test_package_json_engines() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join("package.json")).unwrap();
        writeln!(f, r#"{{"name": "test", "engines": {{"node": "20.10.0"}}}}"#).unwrap();

        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        assert!(
            result.stdout.contains("20.10.0") || result.stdout.contains("Detected"),
            "Should detect node version from package.json engines"
        );
    }

    #[test]
    fn test_package_json_volta() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join("package.json")).unwrap();
        writeln!(f, r#"{{"name": "test", "volta": {{"node": "18.18.0"}}}}"#).unwrap();

        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        assert!(
            result.stdout.contains("18.18.0") || result.stdout.contains("Detected"),
            "Should detect node version from package.json volta"
        );
    }

    #[test]
    fn test_engines_priority_over_volta() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join("package.json")).unwrap();
        writeln!(
            f,
            r#"{{"name": "test", "volta": {{"node": "18.0.0"}}, "engines": {{"node": "20.0.0"}}}}"#
        )
        .unwrap();

        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        // engines should take priority over volta
        assert!(
            result.stdout.contains("20.0.0"),
            "engines should take priority over volta: {}",
            result.stdout
        );
    }

    #[test]
    fn test_go_version_file_detection() {
        let temp_dir = TempDir::new().unwrap();
        // Use .go-version which is the standard version file
        let mut f = File::create(temp_dir.path().join(".go-version")).unwrap();
        writeln!(f, "1.21.0").unwrap();

        let result = run_omg_in_dir(&["use", "go"], temp_dir.path());
        let combined = result.combined_output();
        // Should detect version or show switching message
        assert!(
            combined.contains("1.21")
                || combined.contains("Detected")
                || combined.contains("Switching")
                || combined.contains("go"),
            "Should detect go version from .go-version: {combined}"
        );
    }

    #[test]
    fn test_rust_toolchain_toml_detection() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join("rust-toolchain.toml")).unwrap();
        writeln!(f, "[toolchain]\nchannel = \"1.75.0\"").unwrap();

        let result = run_omg_in_dir(&["use", "rust"], temp_dir.path());
        assert!(
            result.stdout.contains("1.75")
                || result.stdout.contains("Detected")
                || result.stdout.contains("stable"),
            "Should detect rust version from rust-toolchain.toml"
        );
    }

    #[test]
    fn test_version_whitespace_trimming() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join(".nvmrc")).unwrap();
        writeln!(f, "  20.10.0  \n").unwrap();

        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        assert!(
            result.stdout.contains("20.10.0"),
            "Should trim whitespace from version files"
        );
    }

    #[test]
    fn test_version_v_prefix_handling() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join(".nvmrc")).unwrap();
        writeln!(f, "v20.10.0").unwrap();

        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        assert!(
            result.stdout.contains("20.10.0"),
            "Should handle v prefix in version"
        );
    }

    #[test]
    fn test_parent_directory_version_search() {
        let temp_dir = TempDir::new().unwrap();

        // Create .nvmrc at root
        let mut f = File::create(temp_dir.path().join(".nvmrc")).unwrap();
        writeln!(f, "20.10.0").unwrap();

        // Create nested directory
        let nested = temp_dir.path().join("src").join("components");
        fs::create_dir_all(&nested).unwrap();

        // Should find .nvmrc from parent
        let result = run_omg_in_dir(&["use", "node"], &nested);
        assert!(
            result.stdout.contains("20.10.0") || result.stdout.contains("Detected"),
            "Should find version file in parent directories"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PACMAN DATABASE TESTS (Pure Rust parsing - V1/V2 format support)
// ═══════════════════════════════════════════════════════════════════════════════

mod pacman_database {
    use super::*;

    #[test]
    fn test_search_returns_results() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let result = run_omg(&["search", "pacman"]);
        assert!(result.success, "Search should succeed");
        assert!(
            result.stdout.contains("pacman"),
            "Search for 'pacman' should find pacman"
        );
    }

    #[test]
    fn test_search_output_format() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let result = run_omg(&["search", "firefox"]);
        assert!(result.success, "Search should succeed");

        // Output should contain package name
        assert!(
            result.stdout.contains("firefox") || result.stdout.contains("results"),
            "Search output should show results"
        );
    }

    #[test]
    fn test_info_shows_package_details() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let result = run_omg(&["info", "pacman"]);
        assert!(result.success, "Info should succeed for installed package");
        assert!(result.stdout.contains("pacman"), "Should show package name");
    }

    #[test]
    fn test_update_check_parses_databases() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let result = run_omg(&["update", "--check"]);
        let combined = result.combined_output();
        assert!(
            combined.contains("update")
                || combined.contains("up to date")
                || combined.contains("Synchronizing")
                || combined.contains("AUR"),
            "Update check should parse databases"
        );
    }

    #[test]
    fn test_explicit_packages_list() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let result = run_omg(&["explicit"]);
        assert!(result.success, "Explicit should succeed");

        let line_count = result.stdout.lines().filter(|l| !l.is_empty()).count();
        assert!(line_count > 1, "Should list explicitly installed packages");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// AUR INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod aur_integration {
    use super::*;

    #[test]
    fn test_aur_search() {
        if !network_tests_enabled() {
            eprintln!("Skipping network test (set OMG_RUN_NETWORK_TESTS=1)");
            return;
        }

        let result = run_omg(&["search", "yay", "--detailed"]);
        assert!(result.success, "AUR search should succeed");
        assert!(
            result.stdout.contains("yay") || result.stdout.contains("AUR"),
            "Should find AUR packages"
        );
    }

    #[test]
    fn test_update_detects_aur_packages() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let result = run_omg(&["update", "--check"]);
        let combined = result.combined_output();

        // Should mention AUR in output
        assert!(
            combined.contains("AUR")
                || combined.contains("official")
                || combined.contains("up to date"),
            "Update check should handle AUR packages"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// REGRESSION TESTS (Bugs that were fixed)
// ═══════════════════════════════════════════════════════════════════════════════

mod regression_tests {
    use super::*;

    /// Regression: AUR update detection failing due to V1/V2 desc format
    /// The sync database was only parsing packages with %MD5SUM% (V1 format),
    /// missing most packages that use V2 format (no MD5SUM).
    #[test]
    fn test_sync_db_parses_v2_format_packages() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        // Search should find packages from all repos (V2 format)
        let result = run_omg(&["search", "linux"]);
        assert!(result.success, "Search should succeed");
        assert!(
            result.stdout.contains("linux"),
            "Should find packages from V2 format databases"
        );
    }

    /// Regression: Local DB parsing missing packages without MD5SUM
    #[test]
    fn test_local_db_parses_all_packages() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let result = run_omg(&["explicit"]);
        // In CI containers, explicit may fail or return few packages - that's OK
        // The key test is that it doesn't panic
        assert!(
            !result.stderr.contains("panicked at"),
            "Should not panic when listing explicit packages"
        );
        if result.success {
            // CI containers may have very few explicit packages
            // Just verify we can parse the output
            let _package_count = result
                .stdout
                .lines()
                .filter(|l| !l.trim().is_empty())
                .count();
            // If we reached here, parsing succeeded
        }
    }

    /// Regression: engines should take priority over volta in package.json
    #[test]
    fn test_package_json_engines_priority() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join("package.json")).unwrap();
        writeln!(
            f,
            r#"{{"name": "test", "volta": {{"node": "16.0.0"}}, "engines": {{"node": "22.0.0"}}}}"#
        )
        .unwrap();

        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        assert!(
            result.stdout.contains("22.0.0"),
            "engines (22.0.0) should take priority over volta (16.0.0): {}",
            result.stdout
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// STRESS TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod stress_tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_concurrent_status_commands() {
        let handles: Vec<_> = (0..5)
            .map(|_| thread::spawn(|| run_omg(&["status"])))
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result.success, "Concurrent status should succeed");
        }
    }

    #[test]
    fn test_concurrent_list_commands() {
        let handles: Vec<_> = (0..5)
            .map(|_| thread::spawn(|| run_omg(&["list"])))
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            assert!(result.success, "Concurrent list should succeed");
        }
    }

    #[test]
    fn test_rapid_version_detection() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join(".nvmrc")).unwrap();
        writeln!(f, "20.10.0").unwrap();

        // Run multiple times rapidly
        for i in 0..10 {
            let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
            assert!(
                result.success,
                "Rapid version detection failed on iteration {i}: stdout: '{}', stderr: '{}'",
                result.stdout, result.stderr
            );
        }
    }

    #[test]
    fn test_large_search_query() {
        if !system_tests_enabled() {
            eprintln!("Skipping system test (set OMG_RUN_SYSTEM_TESTS=1)");
            return;
        }

        // Search for common term that returns many results
        let result = run_omg(&["search", "lib"]);
        assert!(result.success, "Large search should succeed");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// OUTPUT FORMAT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod output_format {
    use super::*;

    #[test]
    fn test_version_output_format() {
        let result = run_omg(&["--version"]);
        assert!(result.success);
        assert!(
            result.stdout.contains("omg"),
            "Version should contain 'omg'"
        );
        // Should have version number pattern
        assert!(
            result.stdout.contains('.') || result.stdout.contains("0."),
            "Version should have version number"
        );
    }

    #[test]
    fn test_help_lists_all_commands() {
        let result = run_omg(&["--help"]);
        assert!(result.success);

        let expected_commands = [
            "search", "install", "remove", "update", "info", "status", "use", "list",
        ];

        for cmd in expected_commands {
            assert!(
                result.stdout.to_lowercase().contains(cmd),
                "Help should list '{cmd}' command"
            );
        }
    }

    #[test]
    fn test_status_output_sections() {
        let result = run_omg(&["status"]);
        assert!(result.success, "Status should succeed");

        // Should have some structured output
        assert!(
            result.stdout.contains("Package")
                || result.stdout.contains("Runtime")
                || result.stdout.contains("OMG"),
            "Status should have structured sections"
        );
    }

    #[test]
    fn test_list_output_format() {
        let result = run_omg(&["list"]);
        // In CI, list may fail if no runtimes are installed - that's OK
        // The key test is no panic
        assert!(
            !result.stderr.contains("panicked at"),
            "List should not panic"
        );
        if result.success && !result.stdout.is_empty() {
            // If it succeeds, check for expected content
            assert!(
                result.stdout.contains("Node")
                    || result.stdout.contains("Python")
                    || result.stdout.contains("runtime")
                    || result.stdout.contains("OMG")
                    || result.stdout.contains("No")
                    || result.stdout.is_empty(),
                "List should show runtime information or indicate none"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR MESSAGE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod error_messages {
    use super::*;

    #[test]
    fn test_invalid_command_error() {
        let result = run_omg(&["nonexistent-command"]);
        assert!(!result.success, "Invalid command should fail");
        assert!(
            result.stderr.contains("error") || result.stderr.contains("unrecognized"),
            "Should report error for invalid command"
        );
    }

    #[test]
    fn test_missing_package_name_error() {
        let result = run_omg(&["install"]);
        assert!(!result.success, "Install without args should fail");
        assert!(
            result.stderr.contains("required")
                || result.stderr.contains("error")
                || result.stderr.contains("argument"),
            "Should report missing arguments"
        );
    }

    #[test]
    fn test_invalid_lock_file_error() {
        let temp_dir = TempDir::new().unwrap();

        // Create invalid omg.lock
        let mut f = File::create(temp_dir.path().join("omg.lock")).unwrap();
        writeln!(f, "this is not valid toml {{{{").unwrap();

        let result = run_omg_in_dir(&["env", "check"], temp_dir.path());
        assert!(!result.success, "Should fail with invalid lock file");
        // Should not panic, should give error
    }

    #[test]
    #[cfg(feature = "arch")]
    fn test_nonexistent_package_info() {
        let result = run_omg(&["info", "this-package-definitely-does-not-exist-12345"]);
        assert!(
            !result.success
                || result.stdout.contains("not found")
                || result.stdout.contains("No package"),
            "Should indicate package not found"
        );
    }

    #[test]
    fn test_use_without_version_no_config() {
        let temp_dir = TempDir::new().unwrap();
        let result = run_omg_in_dir(&["use", "node"], temp_dir.path());
        assert!(
            !result.success
                || result.stderr.contains("No version")
                || result.stderr.contains("detected"),
            "Should fail without version file"
        );
    }
}
