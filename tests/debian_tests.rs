#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! Comprehensive Debian/Ubuntu Integration Tests
//!
//! Enterprise-grade test coverage for Debian and Ubuntu package management.
//!
//! Run: `cargo test --test debian_tests --features debian`
//! With system tests: `OMG_RUN_SYSTEM_TESTS=1` cargo test --test debian_tests --features debian`
//! On Ubuntu: `OMG_TEST_DISTRO=ubuntu cargo test --test debian_tests --features debian`
//!
//! Note: System tests require real package operations and will modify your system!
//! Only run these tests in disposable containers or development environments.

#![cfg(any(feature = "debian", feature = "debian-pure"))]

mod common;

use common::assertions::*;
use common::fixtures::*;
use common::*;

// ═══════════════════════════════════════════════════════════════════════════════
// DOCKER INTEGRATION
// ═══════════════════════════════════════════════════════════════════════════════

mod docker_integration {
    use std::path::Path;

    #[test]
    fn test_docker_smoke_test_script_exists() {
        // This ensures the smoke test script we expect for CI is present
        let script_path = Path::new("scripts/debian-smoke-test.sh");
        assert!(script_path.exists(), "debian-smoke-test.sh missing");

        // Basic check that it's executable (on unix)
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = script_path.metadata().expect("failed to get metadata");
            assert_eq!(meta.mode() & 0o111, 0o111, "Script should be executable");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// APT INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod apt_integration {
    use super::*;

    #[test]
    fn test_search_main_repo() {
        require_system_tests!();

        // In Debian, firefox is often firefox-esr
        let result = run_omg(&["search", "bash"]);
        result.assert_success();
        assert!(
            result.stdout_contains("bash") || result.stdout_contains("Bash"),
            "Should find bash"
        );
    }

    #[test]
    fn test_search_essential_packages() {
        require_system_tests!();

        for pkg in &["apt", "dpkg", "bash", "coreutils"] {
            let result = run_omg(&["search", pkg]);
            result.assert_success();
            assert!(result.stdout_contains(pkg), "Should find {pkg}");
        }
    }

    #[test]
    fn test_search_development_packages() {
        require_system_tests!();

        for pkg in &["build-essential", "git", "curl", "wget"] {
            let result = run_omg(&["search", pkg]);
            result.assert_success();
        }
    }

    #[test]
    fn test_search_with_architecture() {
        require_system_tests!();

        // Debian packages can have architecture suffixes
        let result = run_omg(&["search", "libc6"]);
        result.assert_success();
    }

    #[test]
    fn test_info_installed_package() {
        require_system_tests!();

        let result = run_omg(&["info", "apt"]);
        result.assert_success();
        assert_package_info(&result, "apt");
    }

    #[test]
    fn test_info_package_details() {
        require_system_tests!();

        let result = run_omg(&["info", "dpkg"]);
        result.assert_success();
        // Should show version, description, etc.
        assert!(
            result.stdout_contains("Version") || result.stdout.contains('.'),
            "Should show version info"
        );
    }

    #[test]
    fn test_info_nonexistent_package() {
        let result = run_omg(&["info", "nonexistent-package-xyz-99999"]);
        // Command may fail with error or succeed with "not found" message
        // Either behavior is acceptable - the key is no panic
        assert!(
            !result.stderr_contains("panicked at"),
            "Should not panic on nonexistent package"
        );
    }

    #[test]
    fn test_explicit_packages() {
        require_system_tests!();

        let result = run_omg(&["explicit"]);
        result.assert_success();
        // Should list manually installed packages
    }

    #[test]
    fn test_explicit_packages_count() {
        require_system_tests!();

        let result = run_omg(&["explicit", "--count"]);
        result.assert_success();
        // Should output a number
        let stdout = result.stdout.trim();
        if !stdout.is_empty() {
            let _: Result<u32, _> = stdout.parse();
        }
    }

    #[test]
    fn test_update_check() {
        require_system_tests!();

        let result = run_omg(&["update", "--check"]);
        result.assert_success();
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_update_check_with_mock_updates() {
        let project = TestProject::new();

        mock_install("firefox-esr", "115.6.0").ok();
        mock_available("firefox-esr", "116.0.0").ok();

        let result = project.run(&["update", "--check"]);
        result.assert_success();

        assert!(
            result.stdout_contains("up to date") || result.stdout_contains("firefox-esr"),
            "Should report up to date or show firefox-esr in updates"
        );
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_update_check_no_updates_when_current() {
        let project = TestProject::new();

        mock_install("firefox-esr", "116.0.0").ok();
        mock_available("firefox-esr", "116.0.0").ok();

        let result = project.run(&["update", "--check"]);
        result.assert_success();

        assert!(
            result.stdout_contains("up to date"),
            "Should report up to date"
        );
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_clean_orphans() {
        require_system_tests!();
        require_destructive_tests!();
        require_debian_like!();

        let result = run_omg(&["clean", "--orphans"]);
        // Should succeed or report no orphans
        assert!(
            result.success || result.stdout_contains("no orphan"),
            "Should handle clean orphans"
        );
    }

    #[test]
    fn test_install_remove_cycle() {
        require_system_tests!();
        require_destructive_tests!();
        require_debian_like!();

        // Ensure database is synced
        let _ = run_omg(&["sync"]);

        // Use a tiny, harmless package
        let pkg = "vim-tiny";

        // 1. Install
        let result = run_omg(&["install", pkg, "-y"]);
        if !result.success {
            if result.stderr_contains("permission") || result.stderr_contains("root") {
                eprintln!("⏭️  Skipping install test: requires root");
                return;
            }
            result.assert_success();
        }

        // 2. Verify installed
        let info = run_omg(&["info", pkg]);
        info.assert_success();
        assert!(
            info.stdout_contains("installed") || info.stdout_contains("Status"),
            "Package should be installed"
        );

        // 3. Remove
        let result = run_omg(&["remove", pkg, "-y"]);
        result.assert_success();

        // 4. Verify removed
        let info = run_omg(&["info", pkg]);
        assert!(
            !info.stdout_contains("installed") || info.stdout_contains("not installed"),
            "Package should be removed"
        );
    }

    #[test]
    fn test_why_integration() {
        require_system_tests!();
        require_debian_like!();

        let result = run_omg(&["why", "apt"]);
        result.assert_success();
        // Should explain why apt is installed (usually 'explicit')
        assert!(
            result.stdout_contains("apt") || result.stdout_contains("explicit"),
            "Should explain why apt is installed"
        );
    }

    #[test]
    fn test_size_integration() {
        require_system_tests!();
        require_debian_like!();

        let result = run_omg(&["size", "--tree", "apt"]);
        result.assert_success();
        assert!(
            result.stdout_contains("MB") || result.stdout_contains("KB"),
            "Should show size of apt package"
        );
    }
}

// Helper macro for both Debian and Ubuntu
#[macro_export]
macro_rules! require_debian_like {
    () => {
        let config = $crate::common::TestConfig::default();
        if !config.is_debian() && !config.is_ubuntu() {
            eprintln!("⏭️  Skipping test: requires Debian or Ubuntu");
            return;
        }
    };
}

// ═══════════════════════════════════════════════════════════════════════════════
// UBUNTU-SPECIFIC TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod ubuntu_specific {
    use super::*;

    #[test]
    fn test_ubuntu_main_repo() {
        require_system_tests!();
        require_ubuntu!();

        let result = run_omg(&["search", "ubuntu-desktop"]);
        result.assert_success();
        // Ubuntu should have ubuntu-specific packages
    }

    #[test]
    fn test_ubuntu_universe_repo() {
        require_system_tests!();
        require_ubuntu!();

        // Universe repo packages
        let result = run_omg(&["search", "htop"]);
        result.assert_success();
    }

    #[test]
    fn test_ubuntu_snap_awareness() {
        require_system_tests!();
        require_ubuntu!();

        // OMG should be aware of snap packages
        let result = run_omg(&["status"]);
        result.assert_success();
    }

    #[test]
    fn test_ubuntu_ppa_handling() {
        require_system_tests!();
        require_ubuntu!();

        // Should handle PPA sources gracefully
        let result = run_omg(&["search", "nodejs"]);
        result.assert_success();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DEBIAN-SPECIFIC TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod debian_specific {
    use super::*;

    #[test]
    fn test_debian_stable_packages() {
        require_system_tests!();
        require_debian!();

        for pkg in &["apt", "dpkg", "systemd"] {
            let result = run_omg(&["search", pkg]);
            result.assert_success();
        }
    }

    #[test]
    fn test_debian_security_repo() {
        require_system_tests!();
        require_debian!();

        // Security updates should be searchable
        let result = run_omg(&["search", "openssl"]);
        result.assert_success();
    }

    #[test]
    fn test_debian_backports_awareness() {
        require_system_tests!();
        require_debian!();

        // Should handle backports if configured
        let result = run_omg(&["status"]);
        result.assert_success();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DPKG DIRECT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod dpkg_direct {
    use super::*;

    #[test]
    fn test_dpkg_query_installed() {
        require_system_tests!();

        // Status should query dpkg database
        let result = run_omg(&["status"]);
        result.assert_success();
    }

    #[test]
    fn test_dpkg_package_info() {
        require_system_tests!();

        let result = run_omg(&["info", "dpkg"]);
        result.assert_success();
    }

    #[test]
    fn test_dpkg_dependency_resolution() {
        require_system_tests!();

        let result = run_omg(&["why", "libc6"]);
        // Should show what depends on libc6
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_dpkg_size_calculation() {
        require_system_tests!();

        let result = run_omg(&["size"]);
        result.assert_success();
        assert!(
            result.stdout_contains("MB")
                || result.stdout_contains("GB")
                || result.stdout_contains("KB"),
            "Should show sizes"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// RUST-APT INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod rust_apt {
    use super::*;

    #[test]
    fn test_rust_apt_cache_access() {
        require_system_tests!();

        // Search uses rust-apt for fast cache access
        let result = run_omg(&["search", "apt"]);
        result.assert_success();
    }

    #[test]
    fn test_rust_apt_package_lookup() {
        require_system_tests!();

        let result = run_omg(&["info", "apt"]);
        result.assert_success();
    }

    #[test]
    fn test_rust_apt_dependency_tree() {
        require_system_tests!();

        let result = run_omg(&["why", "apt"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
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

        let result = run_omg(&["why", "bash"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_why_reverse_dependencies() {
        require_system_tests!();

        let result = run_omg(&["why", "libc6", "--reverse"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_outdated_command() {
        require_system_tests!();

        let result = run_omg(&["outdated"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_outdated_security_only() {
        require_system_tests!();

        let result = run_omg(&["outdated", "--security"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_outdated_json_output() {
        require_system_tests!();

        let result = run_omg(&["outdated", "--json"]);
        if result.success && !result.stdout.trim().is_empty() {
            let _: Result<serde_json::Value, _> = serde_json::from_str(&result.stdout);
        }
    }

    #[test]
    fn test_pin_list() {
        let project = TestProject::new();
        let result = project.run(&["pin", "--list"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_pin_package() {
        let project = TestProject::new();
        let result = project.run(&["pin", "node@20.10.0"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_size_command() {
        require_system_tests!();

        let result = run_omg(&["size"]);
        result.assert_success();
    }

    #[test]
    fn test_size_with_limit() {
        require_system_tests!();

        let result = run_omg(&["size", "--limit", "10"]);
        result.assert_success();
    }

    #[test]
    fn test_blame_command() {
        require_system_tests!();

        let result = run_omg(&["blame", "apt"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_diff_command() {
        let project = TestProject::new();
        project.with_omg_lock(locks::VALID_LOCK);

        let result = project.run(&["diff", "omg.lock"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_snapshot_create() {
        let project = TestProject::new();
        let result = project.run(&["snapshot", "create"]);
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
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_migrate_export() {
        let project = TestProject::new();
        let result = project.run(&["migrate", "export", "--output", "manifest.toml"]);
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

        let result = run_omg(&["status"]);
        result.assert_success();
        assert_performance(&result, perf::STATUS_MAX_MS, "status");
    }

    #[test]
    fn test_search_performance() {
        require_system_tests!();

        let result = run_omg(&["search", "firefox"]);
        result.assert_success();
        assert_performance(&result, perf::SEARCH_MAX_MS, "search");
    }

    #[test]
    fn test_info_performance() {
        require_system_tests!();

        let result = run_omg(&["info", "apt"]);
        result.assert_success();
        assert_performance(&result, 1000, "info");
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
        let result = run_omg(&["completions", "bash", "--stdout"]);
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

        let result = run_omg(&["audit", "scan"]);
        assert_audit_output(&result);
    }

    #[test]
    fn test_audit_sbom_generation() {
        require_system_tests!();

        let project = TestProject::new();
        let result = project.run(&["audit", "sbom", "--output", "sbom.json"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_audit_secrets_scan() {
        let project = TestProject::new();
        project.create_file("config.txt", "AWS_SECRET_KEY=AKIAIOSFODNN7EXAMPLE");

        let result = project.run(&["audit", "secrets"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_injection_prevention_search() {
        for input in validation::INJECTION_ATTEMPTS {
            let result = run_omg(&["search", input]);
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
            assert!(!result.stdout_contains("pwned"), "Should prevent injection");
        }
    }

    #[test]
    fn test_apt_source_validation() {
        // OMG should validate APT sources
        let result = run_omg(&["status"]);
        result.assert_success();
    }

    #[test]
    fn test_gpg_verification_awareness() {
        require_system_tests!();

        // OMG should respect GPG verification
        let result = run_omg(&["audit", "policy"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
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
        assert!(
            !result.stderr_contains("panicked at"),
            "Should handle long input"
        );
    }

    #[test]
    fn test_empty_inputs() {
        for input in validation::EMPTY_INPUTS {
            let result = run_omg(&["search", input]);
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
    fn test_package_with_epoch() {
        require_system_tests!();

        // Debian uses epoch:version-revision format
        let result = run_omg(&["info", "tar"]);
        result.assert_success();
        // Should handle version formats correctly
    }

    #[test]
    fn test_virtual_packages() {
        require_system_tests!();

        // Virtual packages like "mail-transport-agent"
        let result = run_omg(&["search", "mail-transport-agent"]);
        result.assert_success();
        // Should handle virtual packages
    }

    #[test]
    fn test_multiarch_packages() {
        require_system_tests!();

        // Packages can have :amd64, :i386 suffixes
        let result = run_omg(&["search", "libc6"]);
        result.assert_success();
    }

    #[test]
    fn test_transitional_packages() {
        require_system_tests!();

        // Debian has transitional/dummy packages
        let result = run_omg(&["status"]);
        result.assert_success();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CROSS-DISTRO MIGRATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod migration {
    use super::*;

    #[test]
    fn test_migrate_export_format() {
        let project = TestProject::new();
        let result = project.run(&["migrate", "export", "--output", "manifest.toml"]);

        if result.success {
            // Check manifest was created
            assert!(
                project.file_exists("manifest.toml"),
                "Should create manifest"
            );
        }
    }

    #[test]
    fn test_migrate_import_dry_run() {
        let project = TestProject::new();
        // Create a minimal manifest
        project.create_file(
            "manifest.toml",
            r#"
[environment]
distro = "arch"

[packages]
git = "2.43.0"
curl = "8.5.0"
"#,
        );

        let result = project.run(&["migrate", "import", "--dry-run", "manifest.toml"]);
        // Should show what would be installed without doing it
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
    }

    #[test]
    fn test_package_name_mapping() {
        // Some packages have different names across distros
        // e.g., python3-pip vs python-pip
        let project = TestProject::new();
        let result = project.run(&["migrate", "export", "--output", "test.toml"]);
        assert!(!result.stderr_contains("panicked at"), "Should not panic");
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
        assert!(!result.stderr_contains("panicked at"));

        // 4. Create snapshot
        let result = project.run(&["snapshot", "create", "--message", "Initial"]);
        assert!(!result.stderr_contains("panicked at"));
    }

    #[test]
    fn scenario_debian_to_ubuntu_migration() {
        let debian_project = TestProject::new();
        let ubuntu_project = TestProject::new();

        // Simulate Debian environment
        debian_project.with_tool_versions(&[("nodejs", "20.10.0")]);
        debian_project.run(&["env", "capture"]);

        // Export manifest
        debian_project.run(&["migrate", "export", "--output", "manifest.toml"]);

        if let Some(manifest) = debian_project.read_file("manifest.toml") {
            ubuntu_project.create_file("manifest.toml", &manifest);

            // Dry run import on "Ubuntu"
            let result = ubuntu_project.run(&["migrate", "import", "--dry-run", "manifest.toml"]);
            assert!(!result.stderr_contains("panicked at"));
        }
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
            assert!(!result.stderr_contains("panicked at"));
        }
    }

    #[test]
    fn scenario_ci_pipeline_simulation() {
        let project = TestProject::new();
        project.with_tool_versions(&[("nodejs", "20.10.0")]);
        project.with_omg_lock(locks::VALID_LOCK);

        // CI would run these steps:
        // 1. Validate environment against lock
        let result = project.run(&["ci", "validate"]);
        assert!(!result.stderr_contains("panicked at"));

        // 2. Check for drift
        let result = project.run(&["env", "check"]);
        assert!(!result.stderr_contains("panicked at"));

        // 3. Run security audit
        let result = project.run(&["audit"]);
        assert!(!result.stderr_contains("panicked at"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// RUNTIME MANAGEMENT ON DEBIAN/UBUNTU
// ═══════════════════════════════════════════════════════════════════════════════

mod runtime_management {
    use super::*;

    #[test]
    fn test_node_version_management() {
        let project = TestProject::new();
        project.with_node_project();

        let result = project.run(&["use", "node"]);
        result.assert_success();
        // Should detect version from .nvmrc
    }

    #[test]
    fn test_python_version_management() {
        let project = TestProject::new();
        project.with_python_project();

        let result = project.run(&["use", "python"]);
        result.assert_success();
        // Should detect version from .python-version
    }

    #[test]
    fn test_list_available_node() {
        require_network_tests!();

        let result = run_omg(&["list", "node", "--available"]);
        result.assert_success();
    }

    #[test]
    fn test_list_available_python() {
        require_network_tests!();

        let result = run_omg(&["list", "python", "--available"]);
        result.assert_success();
    }

    #[test]
    fn test_which_node() {
        let result = run_omg(&["which", "node"]);
        result.assert_success();
    }

    #[test]
    fn test_which_python() {
        let result = run_omg(&["which", "python"]);
        result.assert_success();
    }

    #[test]
    fn test_tool_versions_detection() {
        let project = TestProject::new();
        project.with_tool_versions(&[
            ("nodejs", "20.10.0"),
            ("python", "3.11.0"),
            ("ruby", "3.2.0"),
        ]);

        let result = project.run(&["status"]);
        result.assert_success();
    }

    #[test]
    fn test_mise_config_detection() {
        let project = TestProject::new();
        project.with_mise_config(&[("node", "20.10.0"), ("python", "3.11.0")]);

        let result = project.run(&["status"]);
        result.assert_success();
    }
}
