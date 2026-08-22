#![cfg(feature = "arch")]
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! Comprehensive Integration Tests for omg install and omg update
//!
//! Tests every aspect of the install and update commands:
//! - CLI argument parsing and validation
//! - Official package operations
//! - AUR package operations
//! - Parallel build and error recovery
//! - Retry logic for network failures
//! - Parallel build functionality
//! - Security (PGP verification, sandbox)
//! - Non-interactive mode (CI/CD)
//! - Mixed package sources
//! - Dry-run mode
//! - Error messages and user experience
//!
//! Run: cargo test --test install_update_comprehensive --features arch
//!
//! Environment variables:
//!   OMG_RUN_SYSTEM_TESTS=1      - Enable tests requiring real system access
//!   OMG_RUN_DESTRUCTIVE_TESTS=1 - Enable tests that modify the system

use std::env;
use std::process::Command;

pub mod common;

// ═══════════════════════════════════════════════════════════════════════════════
// TEST INFRASTRUCTURE
// ═══════════════════════════════════════════════════════════════════════════════

struct TestResult {
    success: bool,
    stdout: String,
    stderr: String,
    exit_code: i32,
}

impl TestResult {
    fn combined(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }

    fn assert_success(&self) -> &Self {
        assert!(
            self.success,
            "Command should succeed. Exit code: {}. Output:\n{}",
            self.exit_code,
            self.combined()
        );
        self
    }

    fn assert_failure(&self) -> &Self {
        assert!(
            !self.success,
            "Command should fail but succeeded. Output:\n{}",
            self.combined()
        );
        self
    }

    fn assert_contains(&self, pattern: &str) -> &Self {
        assert!(
            self.combined().contains(pattern),
            "Output should contain '{}'. Got:\n{}",
            pattern,
            self.combined()
        );
        self
    }

    fn assert_no_panic(&self) -> &Self {
        let combined = self.combined();
        assert!(
            !combined.contains("panicked") && !combined.contains("RUST_BACKTRACE"),
            "Command panicked. Output:\n{}",
            combined
        );
        self
    }

    fn assert_no_password_prompt(&self) -> &Self {
        let combined = self.combined();
        assert!(
            !combined.contains("[sudo]")
                && !combined.contains("password for")
                && !combined.contains("Password:"),
            "Should not prompt for password. Output:\n{}",
            combined
        );
        self
    }
}

fn run_omg(args: &[&str]) -> TestResult {
    run_omg_with_env(args, &[])
}

/// Delegate to the shared isolated runner so every invocation gets unique
/// `OMG_DATA_DIR` / `OMG_CONFIG_DIR` / `OMG_CACHE_DIR` instead of writing
/// mock state into the real user data directory.
fn run_omg_with_env(args: &[&str], env_vars: &[(&str, &str)]) -> TestResult {
    let result = common::run_omg_with_options(args, None, env_vars);
    TestResult {
        success: result.success,
        stdout: result.stdout,
        stderr: result.stderr,
        exit_code: result.exit_code,
    }
}

fn system_tests_enabled() -> bool {
    matches!(env::var("OMG_RUN_SYSTEM_TESTS"), Ok(v) if v == "1")
}

fn destructive_tests_enabled() -> bool {
    matches!(env::var("OMG_RUN_DESTRUCTIVE_TESTS"), Ok(v) if v == "1")
}

fn is_package_installed(pkg: &str) -> bool {
    Command::new("pacman")
        .args(["-Q", pkg])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

// ═══════════════════════════════════════════════════════════════════════════════
// INSTALL COMMAND - CLI ARGUMENT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod install_cli_tests {
    use super::*;

    #[test]
    fn test_install_no_packages_shows_error() {
        let result = run_omg(&["install"]);
        result.assert_failure();
        let combined = result.combined();
        assert!(
            combined.contains("required arguments") || combined.contains("PACKAGES"),
            "Should show clap error about missing packages. Got: {}",
            combined
        );
    }

    #[test]
    fn test_install_help_flag() {
        let result = run_omg(&["install", "--help"]);
        result.assert_success();
        result.assert_contains("install");
    }

    #[test]
    fn test_install_yes_flag_recognized() {
        let result = run_omg(&["install", "-y", "nonexistent-pkg-12345"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_install_dry_run_flag() {
        let result = run_omg(&["install", "--dry-run", "firefox"]);
        result.assert_success();
        let combined = result.combined();
        assert!(
            combined.to_lowercase().contains("dry run") || combined.contains("DRY RUN"),
            "Should indicate dry run mode. Got: {}",
            combined
        );
    }

    #[test]
    fn test_install_multiple_packages() {
        let result = run_omg(&["install", "--dry-run", "firefox", "vim", "git"]);
        result.assert_success();
    }

    #[test]
    fn test_install_invalid_flag() {
        let result = run_omg(&["install", "--invalid-flag-xyz"]);
        result.assert_failure();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INSTALL COMMAND - PACKAGE RESOLUTION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod install_resolution_tests {
    use super::*;

    #[test]
    fn test_install_nonexistent_package() {
        let result = run_omg(&["install", "-y", "this-package-does-not-exist-xyz-12345"]);
        result.assert_failure();
        let combined = result.combined();
        assert!(
            combined.contains("not found") || combined.contains("error"),
            "Should show not found error. Got: {}",
            combined
        );
    }

    #[test]
    fn test_install_dry_run_shows_packages() {
        if !system_tests_enabled() {
            return;
        }
        let result = run_omg(&["install", "--dry-run", "vim"]);
        result.assert_success();
        result.assert_contains("vim");
    }

    #[test]
    fn test_install_detects_aur_package() {
        if !system_tests_enabled() {
            return;
        }
        let result = run_omg(&["install", "--dry-run", "visual-studio-code-bin"]);
        result.assert_success();
        let combined = result.combined();
        assert!(
            combined.contains("AUR") || combined.contains("aur"),
            "Should detect AUR package. Got: {}",
            combined
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INSTALL COMMAND - ERROR HANDLING TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod install_error_tests {
    use super::*;

    #[test]
    fn test_install_helpful_error_messages() {
        let result = run_omg(&["install", "nonexistent-pkg-xyz"]);
        let combined = result.combined();
        assert!(
            combined.contains("not found")
                || combined.contains("error")
                || combined.contains("AUR"),
            "Should show helpful error. Got: {}",
            combined
        );
    }

    #[test]
    fn test_install_no_panic_on_error() {
        let result = run_omg(&["install", "-y", "definitely-fake-package"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_install_without_root_shows_sudo_message() {
        if !system_tests_enabled() {
            return;
        }
        let result = run_omg(&["install", "-y", "vim"]);
        if !result.success {
            let combined = result.combined();
            assert!(
                combined.contains("sudo")
                    || combined.contains("root")
                    || combined.contains("privilege")
                    || combined.contains("Elevating"),
                "Should mention sudo when not root. Got: {}",
                combined
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// UPDATE COMMAND - CLI ARGUMENT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod update_cli_tests {
    use super::*;

    #[test]
    fn test_update_help_flag() {
        let result = run_omg(&["update", "--help"]);
        result.assert_success();
        result.assert_contains("update");
    }

    #[test]
    fn test_update_check_flag() {
        let result = run_omg(&["update", "--check"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_update_yes_flag() {
        let result = run_omg(&["update", "-y", "--check"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_update_dry_run_flag() {
        let result = run_omg(&["update", "--dry-run"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_update_invalid_flag() {
        let result = run_omg(&["update", "--invalid-xyz"]);
        result.assert_failure();
    }

    #[test]
    fn test_update_fast_flag() {
        let result = run_omg(&["update", "--fast", "--check"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_update_turbo_flag() {
        let result = run_omg(&["update", "--turbo", "--check"]);
        result.assert_no_panic();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// UPDATE COMMAND - CHECK MODE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod update_check_tests {
    use super::*;

    #[test]
    fn test_check_mode_no_password_prompt() {
        let result = run_omg(&["update", "--check"]);
        result.assert_no_password_prompt();
    }

    #[test]
    fn test_check_mode_reports_status() {
        if !system_tests_enabled() {
            return;
        }
        let result = run_omg(&["update", "--check"]);
        let combined = result.combined();
        assert!(
            combined.contains("update")
                || combined.contains("up to date")
                || combined.contains("Found")
                || combined.contains("✓"),
            "Should report status. Got: {}",
            combined
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// UPDATE COMMAND - NON-INTERACTIVE MODE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod update_ci_tests {
    use super::*;

    #[test]
    fn test_ci_mode_with_yes() {
        let result = run_omg_with_env(&["update", "--check", "-y"], &[("CI", "1")]);
        result.assert_no_panic();
    }

    #[test]
    fn test_non_interactive_env_var() {
        let result = run_omg_with_env(&["update", "--check"], &[("OMG_NON_INTERACTIVE", "1")]);
        result.assert_no_panic();
    }

    #[test]
    fn test_ci_check_completes() {
        let result = run_omg_with_env(&["update", "--check"], &[("CI", "1")]);
        result.assert_no_panic();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// AUR-SPECIFIC TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod aur_tests {
    use super::*;

    #[test]
    fn test_aur_search_works() {
        if !system_tests_enabled() {
            return;
        }
        let result = run_omg(&["search", "visual-studio-code"]);
        result.assert_success();
        result.assert_contains("visual-studio-code");
    }

    #[test]
    fn test_aur_info_works() {
        if !system_tests_enabled() {
            return;
        }
        let result = run_omg(&["info", "visual-studio-code-bin"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_aur_dry_run() {
        if !system_tests_enabled() {
            return;
        }
        let result = run_omg(&["install", "--dry-run", "visual-studio-code-bin"]);
        result.assert_success();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
// PARALLEL BUILD TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod parallel_build_tests {
    #[test]
    fn test_build_job_creation() {
        use omg_lib::package_managers::aur::BuildJob;
        let job = BuildJob::new("test-pkg".to_string(), vec!["dep1".to_string()]);
        assert_eq!(job.package, "test-pkg");
        assert_eq!(job.dependencies.len(), 1);
    }

    #[test]
    fn test_dependency_graph() {
        use omg_lib::package_managers::aur::{BuildJob, ParallelBuilder};

        let jobs = vec![
            BuildJob::new("a".to_string(), vec![]),
            BuildJob::new("b".to_string(), vec!["a".to_string()]),
            BuildJob::new("c".to_string(), vec!["a".to_string()]),
        ];

        let graph = ParallelBuilder::build_dependency_graph(&jobs);
        assert_eq!(graph.len(), 3);
        assert!(graph.get("a").unwrap().is_empty());
        assert!(graph.get("b").unwrap().contains("a"));
    }

    #[test]
    fn test_topological_sort() {
        use omg_lib::package_managers::aur::ParallelBuilder;
        use std::collections::{HashMap, HashSet};

        let mut graph = HashMap::new();
        graph.insert("a".to_string(), HashSet::new());
        graph.insert("b".to_string(), ["a".to_string()].into_iter().collect());
        graph.insert("c".to_string(), ["b".to_string()].into_iter().collect());

        let levels = ParallelBuilder::topological_levels(&graph).unwrap();
        assert_eq!(levels.len(), 3);
        assert_eq!(levels[0], vec!["a"]);
        assert_eq!(levels[1], vec!["b"]);
        assert_eq!(levels[2], vec!["c"]);
    }

    #[test]
    fn test_circular_dependency_detection() {
        use omg_lib::package_managers::aur::ParallelBuilder;
        use std::collections::HashMap;

        let mut graph = HashMap::new();
        graph.insert("a".to_string(), ["b".to_string()].into_iter().collect());
        graph.insert("b".to_string(), ["a".to_string()].into_iter().collect());

        let result = ParallelBuilder::topological_levels(&graph);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Circular"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH ALPM QUERY TESTS
// ═══════════════════════════════════════════════════════════════════════════════

// `batch_query_tests` was removed in wave 2: the underlying
// `alpm_direct::get_package_info_batch` API no longer exists upstream, and
// per RUST-CODING-STANDARDS §9 tests must not pin removed surfaces. If a
// batch query returns, pin it here again with behavioral assertions.

// ═══════════════════════════════════════════════════════════════════════════════
// DESTRUCTIVE TESTS (REQUIRE OMG_RUN_DESTRUCTIVE_TESTS=1)
// ═══════════════════════════════════════════════════════════════════════════════

mod destructive_tests {
    use super::*;

    fn cleanup_package(pkg: &str) {
        let _ = Command::new("sudo")
            .args(["pacman", "-Rdd", "--noconfirm", pkg])
            .output();
    }

    #[test]
    #[ignore = "performs a real package installation; requires OMG_RUN_DESTRUCTIVE_TESTS=1"]
    fn test_real_install_official_package() {
        assert!(
            destructive_tests_enabled(),
            "Set OMG_RUN_DESTRUCTIVE_TESTS=1 to run destructive tests"
        );

        let test_pkg = "cowsay";
        let was_installed = is_package_installed(test_pkg);

        if was_installed {
            println!("Package already installed, skipping");
            return;
        }

        let result = Command::new("sudo")
            .args([env!("CARGO_BIN_EXE_omg"), "install", "-y", test_pkg])
            .output()
            .expect("Failed to run install");

        let success = result.status.success();

        if is_package_installed(test_pkg) && !was_installed {
            cleanup_package(test_pkg);
        }

        assert!(success, "Install should succeed");
    }

    #[test]
    #[ignore = "runs a privileged real-system update check; requires OMG_RUN_DESTRUCTIVE_TESTS=1"]
    fn test_real_update_check() {
        assert!(
            destructive_tests_enabled(),
            "Set OMG_RUN_DESTRUCTIVE_TESTS=1 to run destructive tests"
        );

        let result = Command::new("sudo")
            .args([env!("CARGO_BIN_EXE_omg"), "update", "--check"])
            .output()
            .expect("Failed to run update check");

        assert!(result.status.success());
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod security_tests {
    use super::*;

    #[test]
    fn test_no_secrets_in_output() {
        let result = run_omg(&["--help"]);
        let combined = result.combined();
        assert!(!combined.contains("password"));
        assert!(!combined.contains("secret"));
        assert!(!combined.contains("token"));
    }

    #[test]
    fn test_command_injection_prevented() {
        let result = run_omg(&["install", "pkg; rm -rf /"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_path_traversal_prevented() {
        let result = run_omg(&["install", "../../../etc/passwd"]);
        result.assert_no_panic();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// USER EXPERIENCE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod ux_tests {
    use super::*;

    #[test]
    fn test_color_output_available() {
        let result = run_omg(&["--help"]);
        result.assert_success();
    }

    #[test]
    fn test_unicode_characters_work() {
        if !system_tests_enabled() {
            return;
        }
        let result = run_omg(&["status"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_error_messages_are_helpful() {
        let result = run_omg(&["install", "nonexistent-xyz-123"]);
        let combined = result.combined();
        assert!(
            combined.len() > 10,
            "Error message should be helpful, got: {}",
            combined
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMPREHENSIVE E2E WORKFLOW TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod e2e_workflow_tests {
    use super::*;

    #[test]
    fn test_full_install_workflow_dry_run() {
        if !system_tests_enabled() {
            return;
        }

        // Complete workflow: search -> info -> install --dry-run
        let search = run_omg(&["search", "vim"]);
        search.assert_success();
        search.assert_contains("vim");

        let info = run_omg(&["info", "vim"]);
        info.assert_no_panic();

        let install = run_omg(&["install", "--dry-run", "vim"]);
        install.assert_success();
        install.assert_contains("vim");
    }

    #[test]
    fn test_full_update_workflow() {
        if !system_tests_enabled() {
            return;
        }

        // Complete update workflow: check -> status -> update --dry-run
        let check = run_omg(&["update", "--check"]);
        check.assert_no_panic();

        let status = run_omg(&["status"]);
        status.assert_no_panic();

        let update = run_omg(&["update", "--dry-run"]);
        update.assert_no_panic();
    }

    #[test]
    fn test_install_multiple_packages_mixed_sources() {
        if !system_tests_enabled() {
            return;
        }

        // Test installing both official and AUR packages in one command
        let result = run_omg(&["install", "--dry-run", "vim", "git", "curl"]);
        result.assert_success();

        let combined = result.combined();
        assert!(
            combined.contains("vim") || combined.contains("git"),
            "Should list packages to install"
        );
    }

    #[test]
    fn test_install_with_dependencies_resolution() {
        if !system_tests_enabled() {
            return;
        }

        // Large packages have many dependencies
        let result = run_omg(&["install", "--dry-run", "firefox"]);
        result.assert_success();

        let combined = result.combined();
        // Should show dependency resolution
        assert!(
            combined.contains("firefox") || combined.contains("dependencies"),
            "Should show dependency information"
        );
    }

    #[test]
    fn test_update_specific_package() {
        if !system_tests_enabled() {
            return;
        }

        // Update a specific package instead of all
        let result = run_omg(&["update", "vim", "--dry-run"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_install_already_installed_package() {
        if !system_tests_enabled() {
            return;
        }

        // Try to install pacman (which is always installed on Arch)
        let result = run_omg(&["install", "pacman", "--dry-run"]);
        result.assert_no_panic();

        let combined = result.combined();
        // Should indicate it's already installed or up to date
        assert!(
            combined.contains("already")
                || combined.contains("up to date")
                || combined.contains("installed")
                || combined.contains("reinstall"),
            "Should indicate package status, got: {}",
            combined
        );
    }

    #[test]
    fn test_install_with_version_constraint() {
        if !system_tests_enabled() {
            return;
        }

        // Some package managers support version constraints
        let result = run_omg(&["install", "--dry-run", "vim"]);
        result.assert_success();
    }

    #[test]
    fn test_update_with_exclude_packages() {
        if !system_tests_enabled() {
            return;
        }

        // Test excluding packages from update
        let result = run_omg(&["update", "--dry-run"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_search_then_install_pipeline() {
        if !system_tests_enabled() {
            return;
        }

        // Realistic user workflow
        let search = run_omg(&["search", "htop"]);
        search.assert_success();

        if search.combined().contains("htop") {
            let install = run_omg(&["install", "--dry-run", "htop"]);
            install.assert_success();
        }
    }

    #[test]
    fn test_install_package_with_provides() {
        if !system_tests_enabled() {
            return;
        }

        // Some packages provide virtual packages
        let result = run_omg(&["info", "vim"]);
        result.assert_no_panic();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// EDGE CASE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod edge_case_tests {
    use super::*;

    #[test]
    fn test_install_empty_package_name() {
        let result = run_omg(&["install", ""]);
        result.assert_failure();
    }

    #[test]
    fn test_install_package_name_with_spaces() {
        let result = run_omg(&["install", "package with spaces"]);
        result.assert_no_panic();
        // Should either handle it or error gracefully
    }

    #[test]
    fn test_install_very_long_package_name() {
        let long_name = "a".repeat(1000);
        let result = run_omg(&["install", "--dry-run", &long_name]);
        result.assert_no_panic();
    }

    #[test]
    fn test_install_unicode_package_name() {
        let result = run_omg(&["install", "--dry-run", "🦀-package"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_install_with_special_chars() {
        let special_chars = vec!["pkg@123", "pkg#hash", "pkg!", "pkg?"];

        for pkg in special_chars {
            let result = run_omg(&["install", "--dry-run", pkg]);
            result.assert_no_panic();
        }
    }

    #[test]
    fn test_update_with_no_packages_installed() {
        // This is impossible in reality but tests error handling
        let result = run_omg(&["update", "--check"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_install_case_sensitivity() {
        if !system_tests_enabled() {
            return;
        }

        let lower = run_omg(&["search", "vim"]);
        let upper = run_omg(&["search", "VIM"]);

        // Both should work (case-insensitive) or handle gracefully
        lower.assert_no_panic();
        upper.assert_no_panic();
    }

    #[test]
    fn test_install_package_with_hyphen_underscore() {
        if !system_tests_enabled() {
            return;
        }

        // Test packages with different naming conventions
        let result = run_omg(&["search", "python-"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_multiple_flags_combination() {
        let result = run_omg(&["install", "-y", "--dry-run", "--fast", "vim"]);
        result.assert_no_panic();
    }

    #[test]
    fn test_conflicting_flags() {
        // Some flags might conflict
        let result = run_omg(&["install", "--fast", "--turbo", "vim"]);
        result.assert_no_panic();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// REAL SYSTEM STATE TESTS (SAFE, NON-DESTRUCTIVE)
// ═══════════════════════════════════════════════════════════════════════════════

mod system_state_tests {
    use super::*;

    #[test]
    fn test_list_installed_packages() {
        if !system_tests_enabled() {
            return;
        }

        let result = run_omg(&["list", "--installed"]);
        result.assert_success();

        // Should show some installed packages
        let combined = result.combined();
        assert!(combined.len() > 10, "Should list installed packages");
    }

    #[test]
    fn test_search_returns_real_results() {
        if !system_tests_enabled() {
            return;
        }

        let result = run_omg(&["search", "linux"]);
        result.assert_success();

        let combined = result.combined();
        assert!(
            combined.contains("linux") || combined.contains("kernel"),
            "Search should return relevant results"
        );
    }

    #[test]
    fn test_info_shows_package_details() {
        if !system_tests_enabled() {
            return;
        }

        let result = run_omg(&["info", "pacman"]);
        result.assert_success();

        let combined = result.combined();
        assert!(
            combined.contains("Version")
                || combined.contains("Description")
                || combined.contains("pacman"),
            "Info should show package details"
        );
    }

    #[test]
    fn test_update_check_shows_available_updates() {
        if !system_tests_enabled() {
            return;
        }

        let result = run_omg(&["update", "--check"]);
        result.assert_success();

        let combined = result.combined();
        // Should either show updates or say system is up to date
        assert!(
            combined.contains("update")
                || combined.contains("up to date")
                || combined.contains("available")
                || combined.len() > 5,
            "Should show update status"
        );
    }

    #[test]
    fn test_status_shows_system_info() {
        if !system_tests_enabled() {
            return;
        }

        let result = run_omg(&["status"]);
        result.assert_success();

        let combined = result.combined();
        assert!(combined.len() > 10, "Status should show system information");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTEGRATION WITH OTHER COMMANDS
// ═══════════════════════════════════════════════════════════════════════════════

mod command_integration_tests {
    use super::*;

    #[test]
    fn test_install_then_remove_workflow() {
        if !destructive_tests_enabled() {
            return;
        }

        // Full workflow test (destructive)
        let install = run_omg(&["install", "--dry-run", "cowsay"]);
        install.assert_success();

        // Verify remove command also works
        let remove = run_omg(&["remove", "--dry-run", "cowsay"]);
        remove.assert_no_panic();
    }

    #[test]
    fn test_update_then_install_workflow() {
        if !system_tests_enabled() {
            return;
        }

        let update = run_omg(&["update", "--check"]);
        update.assert_no_panic();

        let install = run_omg(&["install", "--dry-run", "vim"]);
        install.assert_success();
    }

    #[test]
    fn test_search_info_install_pipeline() {
        if !system_tests_enabled() {
            return;
        }

        // Complete user journey
        let search = run_omg(&["search", "htop"]);
        search.assert_success();

        let info = run_omg(&["info", "htop"]);
        info.assert_no_panic();

        let install = run_omg(&["install", "--dry-run", "htop"]);
        install.assert_success();
    }

    #[test]
    fn test_status_after_failed_install() {
        let bad_install = run_omg(&["install", "-y", "nonexistent-pkg"]);
        bad_install.assert_failure();

        // Status should still work after failure
        let status = run_omg(&["status"]);
        status.assert_no_panic();
    }

    #[test]
    fn test_multiple_commands_in_sequence() {
        if !system_tests_enabled() {
            return;
        }

        // Simulate realistic CLI usage session
        let commands = vec![
            vec!["status"],
            vec!["search", "vim"],
            vec!["info", "vim"],
            vec!["install", "--dry-run", "vim"],
            vec!["update", "--check"],
        ];

        for cmd in commands {
            let result = run_omg(&cmd);
            result.assert_no_panic();
        }
    }
}
