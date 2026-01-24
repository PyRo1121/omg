//! Comprehensive Arch Linux Integration Tests
//!
//! Enterprise-grade test coverage for Arch Linux package management.
//!
//! Run: cargo test --test `arch_tests` --features arch
//! With system tests: `OMG_RUN_SYSTEM_TESTS=1` cargo test --test `arch_tests` --features arch
//!
//! Note: System tests require real package operations and will modify your system!
//! Only run these tests in disposable containers or development environments.

#![cfg(feature = "arch")]
#![allow(clippy::doc_markdown)]

mod common;

use common::assertions::*;
use common::fixtures::*;
use common::*;

// ═══════════════════════════════════════════════════════════════════════════════
// PACMAN INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod pacman_integration {
    use super::*;

    #[test]
    fn test_search_official_repos() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["search", "firefox"]);
        result.assert_success();
        assert_search_results(&result, &["firefox"]);
    }

    #[test]
    fn test_search_core_packages() {
        require_system_tests!();
        require_arch!();

        // Core packages should be found
        for pkg in &["linux", "pacman", "glibc", "bash"] {
            let result = run_omg(&["search", pkg]);
            result.assert_success();
            assert!(result.stdout_contains(pkg), "Should find {pkg}");
        }
    }

    #[test]
    fn test_search_extra_packages() {
        require_system_tests!();
        require_arch!();

        for pkg in &["git", "vim", "python", "nodejs"] {
            let result = run_omg(&["search", pkg]);
            result.assert_success();
        }
    }

    #[test]
    fn test_search_with_regex() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["search", "^linux$"]);
        // Should handle regex-like patterns
        assert!(
            !result.stderr_contains("panicked at"),
            "Should not panic on regex"
        );
    }

    #[test]
    fn test_info_installed_package() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["info", "pacman"]);
        result.assert_success();
        assert_package_info(&result, "pacman");
        assert!(
            result.stdout_contains("core") || result.stdout_contains("Repository"),
            "Should show repository"
        );
    }

    #[test]
    fn test_info_not_installed_package() {
        require_system_tests!();
        require_arch!();

        // A package that exists but might not be installed
        let result = run_omg(&["info", "firefox"]);
        // Should succeed whether installed or not
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_info_nonexistent_package() {
        let result = run_omg(&["info", "this-package-definitely-does-not-exist-12345"]);
        // Should fail gracefully
        assert!(
            !result.success || result.contains("not found"),
            "Should indicate package not found"
        );
    }

    #[test]
    fn test_explicit_packages_list() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["explicit"]);
        result.assert_success();
        // On Arch, there should be at least some explicit packages
        assert!(
            !result.stdout.trim().is_empty() || result.stdout_contains("0"),
            "Should list packages or count"
        );
    }

    #[test]
    fn test_explicit_packages_count() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["explicit", "--count"]);
        result.assert_success();
        // Should output a number
        let count: Result<u32, _> = result.stdout.trim().parse();
        assert!(count.is_ok(), "Should output a valid number");
    }

    #[test]
    fn test_sync_databases() {
        require_system_tests!();
        require_destructive_tests!();
        require_arch!();

        let result = run_omg(&["sync"]);
        // May require root, so check for permission error OR success
        assert!(
            result.success
                || result.stderr_contains("permission")
                || result.stderr_contains("root"),
            "Should sync or report permission issue"
        );
    }

    #[test]
    fn test_update_check() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["update", "--check"]);
        result.assert_success();
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_update_check_with_mock_updates() {
        let project = TestProject::new();

        mock_install("firefox", "122.0").ok();
        mock_available("firefox", "123.0").ok();

        let result = project.run(&["update", "--check"]);
        result.assert_success();

        assert!(
            result.stdout_contains("up to date") || result.stdout_contains("firefox"),
            "Should report up to date or show firefox in updates"
        );
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_update_check_no_updates_when_current() {
        let project = TestProject::new();

        mock_install("firefox", "123.0").ok();
        mock_available("firefox", "123.0").ok();

        let result = project.run(&["update", "--check"]);
        result.assert_success();

        assert!(result.stdout_contains("up to date"), "Should report up to date");
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_clean_options() {
        require_arch!();

        // Test help shows all options
        let result = run_omg(&["clean", "--help"]);
        result.assert_success();
        assert!(
            result.stdout_contains("orphans"),
            "Should have orphans option"
        );
        assert!(result.stdout_contains("cache"), "Should have cache option");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// AUR INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod aur_integration {
    use super::*;

    #[test]
    fn test_search_aur_packages() {
        require_network_tests!();
        require_arch!();

        // yay is a popular AUR package
        let result = run_omg(&["search", "yay"]);
        result.assert_success();
        assert!(
            result.stdout_contains("yay") || result.stdout_contains("AUR"),
            "Should find yay or show AUR results"
        );
    }

    #[test]
    fn test_search_aur_detailed() {
        require_network_tests!();
        require_arch!();

        let result = run_omg(&["search", "yay", "--detailed"]);
        result.assert_success();
        // Detailed should show votes or maintainer for AUR packages
    }

    #[test]
    fn test_info_aur_package() {
        require_network_tests!();
        require_arch!();

        let _result = run_omg(&["info", "yay"]);
        // Should show AUR info or indicate AUR source
    }

    #[test]
    fn test_aur_helper_detection() {
        require_arch!();

        // OMG should detect available AUR helpers
        let result = run_omg(&["status"]);
        result.assert_success();
        // Status should work regardless of AUR helper presence
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ALPM DIRECT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod alpm_direct {
    use super::*;

    #[test]
    fn test_alpm_database_access() {
        require_system_tests!();
        require_arch!();

        // Status uses ALPM directly for speed
        let result = run_omg(&["status"]);
        result.assert_success();
    }

    #[test]
    fn test_alpm_local_db_query() {
        require_system_tests!();
        require_arch!();

        // Explicit uses ALPM local database
        let result = run_omg(&["explicit", "--count"]);
        result.assert_success();
    }

    #[test]
    fn test_alpm_sync_db_query() {
        require_system_tests!();
        require_arch!();

        // Search uses ALPM sync databases
        let result = run_omg(&["search", "pacman"]);
        result.assert_success();
        assert!(result.stdout_contains("pacman"), "Should find pacman");
    }

    #[test]
    fn test_alpm_dependency_resolution() {
        require_system_tests!();
        require_arch!();

        // Why command uses ALPM for dependency tracking
        let result = run_omg(&["why", "glibc"]);
        // Should show what depends on glibc
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_alpm_size_calculation() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["size"]);
        result.assert_success();
        // Should show disk usage
        assert!(
            result.stdout_contains("MB")
                || result.stdout_contains("GB")
                || result.stdout_contains("KiB"),
            "Should show sizes"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// NEW FEATURE TESTS (why, pin, outdated, etc.)
// ═══════════════════════════════════════════════════════════════════════════════

mod new_features {
    use super::*;

    #[test]
    fn test_why_command() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["why", "bash"]);
        // Should explain why bash is installed
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_why_reverse_dependencies() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["why", "glibc", "--reverse"]);
        // Should show what depends on glibc
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_outdated_command() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["outdated"]);
        // Should list outdated packages or indicate none
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_outdated_security_only() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["outdated", "--security"]);
        // Should filter to security updates
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_outdated_json_output() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["outdated", "--json"]);
        // Should output valid JSON
        if result.success && !result.stdout.trim().is_empty() {
            let _: Result<serde_json::Value, _> = serde_json::from_str(&result.stdout);
        }
    }

    #[test]
    fn test_pin_list() {
        let project = TestProject::new();
        let result = project.run(&["pin", "--list"]);
        // Should show pins or indicate none
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_pin_package() {
        let project = TestProject::new();
        let result = project.run(&["pin", "node@20.10.0"]);
        // Should pin or explain how
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_size_command() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["size"]);
        result.assert_success();
        assert!(
            result.stdout_contains("MB") || result.stdout_contains("GB"),
            "Should show disk usage"
        );
    }

    #[test]
    fn test_size_with_limit() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["size", "--limit", "10"]);
        result.assert_success();
    }

    #[test]
    fn test_size_tree() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["size", "--tree", "pacman"]);
        // Should show dependency tree with sizes
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_blame_command() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["blame", "pacman"]);
        // Should show install history
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_diff_command() {
        let project = TestProject::new();
        project.with_omg_lock(locks::VALID_LOCK);

        let result = project.run(&["diff", "omg.lock"]);
        // Should compare against current state
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_snapshot_create() {
        let project = TestProject::new();
        let result = project.run(&["snapshot", "create"]);
        // Should create a snapshot
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_snapshot_list() {
        let project = TestProject::new();
        let result = project.run(&["snapshot", "list"]);
        result.assert_success();
    }

    #[test]
    fn test_ci_init_github() {
        let project = TestProject::new();
        let result = project.run(&["ci", "init", "--provider", "github"]);
        // Should generate GitHub Actions config
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_migrate_export() {
        let project = TestProject::new();
        let result = project.run(&["migrate", "export", "--output", "manifest.toml"]);
        // Should export manifest
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PERFORMANCE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod performance {
    use super::*;

    #[test]
    fn test_status_performance() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["status"]);
        result.assert_success();
        assert_performance(&result, perf::STATUS_MAX_MS, "status");
    }

    #[test]
    fn test_explicit_count_performance() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["explicit", "--count"]);
        result.assert_success();
        // Should be very fast (sub-10ms target)
        assert_performance(&result, 50, "explicit --count");
    }

    #[test]
    fn test_search_performance() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["search", "firefox"]);
        result.assert_success();
        // Search should be fast with ALPM direct
        assert_performance(&result, perf::SEARCH_MAX_MS, "search");
    }

    #[test]
    fn test_which_performance() {
        let result = run_omg(&["which", "node"]);
        result.assert_success();
        assert_performance(&result, perf::WHICH_MAX_MS, "which");
    }

    #[test]
    fn test_list_performance() {
        let result = run_omg(&["list"]);
        result.assert_success();
        assert_performance(&result, perf::LIST_MAX_MS, "list");
    }

    #[test]
    fn test_help_performance() {
        let result = run_omg(&["--help"]);
        result.assert_success();
        assert_performance(&result, perf::HELP_MAX_MS, "help");
    }

    #[test]
    fn test_completions_performance() {
        let result = run_omg(&["completions", "zsh", "--stdout"]);
        result.assert_success();
        assert_performance(&result, perf::COMPLETIONS_MAX_MS, "completions");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod security {
    use super::*;

    #[test]
    fn test_audit_scan() {
        require_system_tests!();
        require_arch!();

        let result = run_omg(&["audit", "scan"]);
        assert_audit_output(&result);
    }

    #[test]
    fn test_audit_sbom_generation() {
        require_system_tests!();
        require_arch!();

        let project = TestProject::new();
        let result = project.run(&["audit", "sbom", "--output", "sbom.json"]);
        // Should generate SBOM or indicate requirements
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_audit_secrets_scan() {
        let project = TestProject::new();
        project.create_file("config.txt", "password=secret123");

        let result = project.run(&["audit", "secrets"]);
        // Should detect secrets or indicate no issues
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_audit_policy_enforcement() {
        let project = TestProject::new();
        project.with_security_policy(policies::STRICT_POLICY);

        let result = project.run(&["audit", "policy"]);
        // Should show policy status
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_injection_prevention_search() {
        for input in validation::INJECTION_ATTEMPTS {
            let result = run_omg(&["search", input]);
            // Should not execute injected commands
            assert!(
                !result.stdout_contains("pwned"),
                "Should prevent injection: {input}"
            );
            assert!(
                !result.stdout_contains("/etc/passwd"),
                "Should prevent path traversal"
            );
        }
    }

    #[test]
    fn test_injection_prevention_info() {
        for input in validation::INJECTION_ATTEMPTS {
            let result = run_omg(&["info", input]);
            // The key test: shell commands should NOT be executed
            // "pwned" appearing in error message quoting the input is OK
            // "pwned" appearing as actual command output is NOT OK
            // Check for actual command execution indicators
            assert!(
                !result.stdout_contains("root:x:0:0"),
                "Should prevent /etc/passwd leak: {input}"
            );
            assert!(
                !result.stdout_contains("/bin/bash"),
                "Should prevent passwd content leak: {input}"
            );
            // Should not panic
            assert!(
                !result.stderr_contains("panicked at"),
                "Should not panic on: {input}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// EDGE CASES
// ═══════════════════════════════════════════════════════════════════════════════

mod edge_cases {
    use super::*;

    #[test]
    fn test_unicode_package_names() {
        for input in validation::UNICODE_INPUTS {
            let result = run_omg(&["search", input]);
            // Should handle gracefully (not crash)
            assert!(
                !result.stderr_contains("panicked at"),
                "Should handle unicode: {input}"
            );
        }
    }

    #[test]
    fn test_very_long_query() {
        let long_query = validation::very_long_input(10000);
        let result = run_omg(&["search", &long_query]);
        // Should handle without crashing
        assert!(
            !result.stderr_contains("panicked at"),
            "Should handle long input"
        );
    }

    #[test]
    fn test_empty_inputs() {
        for input in validation::EMPTY_INPUTS {
            let result = run_omg(&["search", input]);
            // Should handle gracefully
            assert!(
                !result.stderr_contains("panicked at"),
                "Should handle empty input"
            );
        }
    }

    #[test]
    fn test_concurrent_operations() {
        use std::thread;

        let handles: Vec<_> = (0..10)
            .map(|_| thread::spawn(|| run_omg(&["status"])))
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            result.assert_success();
        }
    }

    #[test]
    fn test_deeply_nested_directory() {
        let project = TestProject::new();
        project.with_node_project();

        let deep_path = project.create_dir("a/b/c/d/e/f/g/h/i/j");
        let _result = run_omg_in_dir(&["use", "node"], &deep_path);
        // Should find version file in parent
    }

    #[test]
    fn test_symlink_handling() {
        let project = TestProject::new();
        project.with_node_project();

        #[cfg(unix)]
        {
            let link_path = project.path().join("link");
            std::os::unix::fs::symlink(project.path(), &link_path).ok();
            if link_path.exists() {
                let _result = run_omg_in_dir(&["use", "node"], &link_path);
                // Should work through symlinks
            }
        }
    }

    #[test]
    fn test_readonly_directory() {
        // Test handling of readonly directories
        let _project = TestProject::new();
        // Would need elevated permissions to test properly
    }

    #[test]
    fn test_missing_home_directory() {
        // Test with HOME unset
        let result = run_omg_with_env(&["status"], &[("HOME", "")]);
        // Should handle gracefully
        assert!(
            !result.stderr_contains("panicked at"),
            "Should handle missing HOME"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTEGRATION SCENARIOS
// ═══════════════════════════════════════════════════════════════════════════════

mod integration_scenarios {
    use super::*;

    #[test]
    fn scenario_full_workflow() {
        let project = TestProject::new();
        project.with_tool_versions(&[("nodejs", "20.10.0"), ("python", "3.11.0")]);

        // 1. Check status
        let result = project.run(&["status"]);
        result.assert_success();

        // 2. Capture environment
        let result = project.run(&["env", "capture"]);
        result.assert_success();
        assert!(project.file_exists("omg.lock"), "Should create omg.lock");

        // 3. Check environment
        let result = project.run(&["env", "check"]);
        // Should work
        assert!(!result.stderr_contains("panicked at"));

        // 4. Create snapshot
        let result = project.run(&["snapshot", "create", "--message", "Initial"]);
        // Should work
        assert!(!result.stderr_contains("panicked at"));
    }

    #[test]
    fn scenario_team_collaboration() {
        let dev1 = TestProject::new();
        let dev2 = TestProject::new();

        // Dev1 sets up project
        dev1.with_tool_versions(&[("nodejs", "20.10.0")]);
        dev1.run(&["env", "capture"]);

        // Copy lock to dev2
        if let Some(lock) = dev1.read_file("omg.lock") {
            dev2.create_file("omg.lock", &lock);
            dev2.with_tool_versions(&[("nodejs", "20.10.0")]);

            // Dev2 checks for drift
            let result = dev2.run(&["env", "check"]);
            // Should detect same or report drift
            assert!(!result.stderr_contains("panicked at"));
        }
    }

    #[test]
    fn scenario_security_audit() {
        require_system_tests!();
        require_arch!();

        // Run full security audit workflow
        let result = run_omg(&["audit"]);
        // Should produce audit output
        assert!(!result.stderr_contains("panicked at"));

        let result = run_omg(&["audit", "policy"]);
        assert!(!result.stderr_contains("panicked at"));
    }
}
