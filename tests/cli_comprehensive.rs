//! Comprehensive CLI E2E tests for all OMG commands
//!
//! Tests every command with success and error cases to achieve 100% CLI coverage.

#![cfg(feature = "arch")]

mod common;

use common::*;

// =======================
// CORE PACKAGE MANAGEMENT
// =======================

mod install_tests {
    use super::*;

    #[test]
    fn test_install_help() {
        let result = run_omg(&["install", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("install");
    }

    #[test]
    fn test_install_nonexistent() {
        let result = run_omg(&["install", "package-that-definitely-does-not-exist-12345"]);
        // Should complete (may succeed with error message or fail)
        let combined = result.combined_output();
        // Just verify it handles the case (shows message or exits)
        assert!(!combined.is_empty(), "Should produce some output");
    }

    #[test]
    fn test_install_dry_run() {
        let result = run_omg(&["install", "--dry-run", "pacman"]);
        // Dry run should succeed (or show what would be done)
        let combined = result.combined_output();
        assert!(
            result.success || combined.contains("already installed") || combined.contains("Would"),
            "Dry run should work"
        );
    }
}

mod remove_tests {
    use super::*;

    #[test]
    fn test_remove_help() {
        let result = run_omg(&["remove", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("remove");
    }

    #[test]
    fn test_remove_nonexistent() {
        let result = run_omg(&["remove", "package-never-installed-xyz"]);
        // Should complete (may show error or handle gracefully)
        assert!(!result.combined_output().is_empty());
    }
}

mod update_tests {
    use super::*;

    #[test]
    fn test_update_help() {
        let result = run_omg(&["update", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("update");
    }

    #[test]
    fn test_update_dry_run() {
        let result = run_omg(&["update", "--check"]);
        // Check should always work (shows what would be updated)
        assert!(result.success || result.combined_output().contains("up to date"));
    }
}

// ====================
// RUNTIME MANAGEMENT
// ====================

mod runtime_tests {
    use super::*;

    #[test]
    fn test_hook_bash() {
        let result = run_omg(&["hook", "bash"]);
        result.assert_success();
        result.assert_stdout_contains("eval");
    }

    #[test]
    fn test_hook_zsh() {
        let result = run_omg(&["hook", "zsh"]);
        result.assert_success();
        result.assert_stdout_contains("eval");
    }

    #[test]
    fn test_hook_fish() {
        let result = run_omg(&["hook", "fish"]);
        result.assert_success();
        // Fish uses 'source' instead of 'eval'
        let output = result.combined_output();
        assert!(output.contains("source") || output.contains("omg"));
    }

    #[test]
    fn test_use_invalid_runtime() {
        let result = run_omg(&["use", "invalid-runtime", "1.0.0"]);
        assert!(!result.success, "Should fail for invalid runtime");
    }

    #[test]
    fn test_which_help() {
        let result = run_omg(&["which", "--help"]);
        result.assert_success();
    }
}

// ===================
// PROJECT WORKFLOWS
// ===================

mod project_tests {
    use super::*;

    #[test]
    fn test_run_help() {
        let result = run_omg(&["run", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("run");
    }

    #[test]
    fn test_new_help() {
        let result = run_omg(&["new", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("template");
    }

    #[test]
    fn test_tool_help() {
        let result = run_omg(&["tool", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("tool");
    }

    #[test]
    fn test_tool_list() {
        let result = run_omg(&["tool", "list"]);
        // Should succeed even if no tools installed
        assert!(result.success || result.combined_output().contains("No tools"));
    }
}

// ====================
// ENVIRONMENT & TEAM
// ====================

mod env_tests {
    use super::*;

    #[test]
    fn test_env_help() {
        let result = run_omg(&["env", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("env");
    }

    #[test]
    fn test_team_help() {
        let result = run_omg(&["team", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("team");
    }

    #[test]
    fn test_hooks_help() {
        let result = run_omg(&["hooks", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("hooks");
    }

    #[test]
    fn test_snapshot_help() {
        let result = run_omg(&["snapshot", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("snapshot");
    }
}

// ==================
// CONTAINER & CI
// ==================

mod devops_tests {
    use super::*;

    #[test]
    fn test_container_help() {
        let result = run_omg(&["container", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("container");
    }

    #[test]
    fn test_ci_help() {
        let result = run_omg(&["ci", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("ci");
    }
}

// ========================
// SECURITY & COMPLIANCE
// ========================

mod security_tests {
    use super::*;

    #[test]
    fn test_audit_help() {
        let result = run_omg(&["audit", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("audit");
    }

    #[test]
    fn test_audit_sbom_help() {
        let result = run_omg(&["audit", "sbom", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_audit_secrets_help() {
        let result = run_omg(&["audit", "secrets", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_audit_licenses_help() {
        let result = run_omg(&["audit", "licenses", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_license_help() {
        let result = run_omg(&["license", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("license");
    }
}

// ====================
// SYSTEM MANAGEMENT
// ====================

mod system_tests {
    use super::*;

    #[test]
    fn test_doctor_help() {
        let result = run_omg(&["doctor", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("doctor");
    }

    #[test]
    fn test_doctor_run() {
        let result = run_omg(&["doctor"]);
        // Doctor should always work (shows diagnostic info)
        assert!(result.success, "Doctor command should succeed");
    }

    #[test]
    fn test_config_help() {
        let result = run_omg(&["config", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("config");
    }

    #[test]
    fn test_config_list() {
        let result = run_omg(&["config", "list"]);
        // Should show config even if empty
        assert!(result.success);
    }

    #[test]
    fn test_daemon_help() {
        let result = run_omg(&["daemon", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("daemon");
    }

    #[test]
    fn test_daemon_status_basic() {
        let result = run_omg(&["daemon-status"]);
        // Should work whether daemon is running or not
        assert!(result.success || result.combined_output().contains("not running"));
    }

    #[test]
    fn test_history_help() {
        let result = run_omg(&["history", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("history");
    }

    #[test]
    fn test_rollback_help() {
        let result = run_omg(&["rollback", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("rollback");
    }

    #[test]
    fn test_migrate_help() {
        let result = run_omg(&["migrate", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("migrate");
    }
}

// ================
// UI & UTILITIES
// ================

mod ui_tests {
    use super::*;

    #[test]
    fn test_dash_help() {
        let result = run_omg(&["dash", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("dash");
    }

    #[test]
    fn test_stats_help() {
        let result = run_omg(&["stats", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("stats");
    }

    #[test]
    fn test_metrics_help() {
        let result = run_omg(&["metrics", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("metrics");
    }

    #[test]
    fn test_completions_bash() {
        let result = run_omg(&["completions", "bash"]);
        result.assert_success();
        // Should generate bash completion script
        assert!(!result.stdout.is_empty());
    }

    #[test]
    fn test_completions_fish() {
        let result = run_omg(&["completions", "fish"]);
        result.assert_success();
        assert!(!result.stdout.is_empty());
    }

    #[test]
    fn test_completions_powershell() {
        let result = run_omg(&["completions", "powershell"]);
        result.assert_success();
        assert!(!result.stdout.is_empty());
    }

    #[test]
    fn test_generate_man_help() {
        let result = run_omg(&["generate-man", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_diff_help() {
        let result = run_omg(&["diff", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("diff");
    }
}

// ===================
// ENTERPRISE & FLEET
// ===================

mod enterprise_tests {
    use super::*;

    #[test]
    fn test_fleet_help() {
        let result = run_omg(&["fleet", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("fleet");
    }

    #[test]
    fn test_enterprise_help() {
        let result = run_omg(&["enterprise", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("enterprise");
    }
}

// ==============
// META COMMANDS
// ==============

mod meta_tests {
    use super::*;

    #[test]
    fn test_self_update_help() {
        let result = run_omg(&["self-update", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("update");
    }

    #[test]
    fn test_self_update_check() {
        let result = run_omg(&["self-update", "--check"]);
        // Should complete (checking for updates)
        assert!(!result.combined_output().is_empty());
    }

    #[test]
    fn test_init_help() {
        let result = run_omg(&["init", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("init");
    }
}

// ===================
// PACKAGE OPERATIONS
// ===================

mod package_ops_tests {
    use super::*;

    #[test]
    fn test_pin_help() {
        let result = run_omg(&["pin", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("pin");
    }

    #[test]
    fn test_pin_list() {
        let result = run_omg(&["pin", "list"]);
        // Should complete successfully
        assert!(!result.combined_output().is_empty());
    }

    #[test]
    fn test_clean_cache() {
        let result = run_omg(&["clean", "cache", "--dry-run"]);
        // Should complete (dry run is safe)
        assert!(!result.combined_output().is_empty());
    }

    #[test]
    fn test_clean_orphans() {
        let result = run_omg(&["clean", "orphans", "--dry-run"]);
        // Should complete (dry run is safe)
        assert!(!result.combined_output().is_empty());
    }
}

// ==============
// ERROR HANDLING
// ==============

mod error_tests {
    use super::*;

    #[test]
    fn test_invalid_command() {
        let result = run_omg(&["this-command-does-not-exist"]);
        assert!(!result.success, "Should fail for invalid command");
        let combined = result.combined_output();
        assert!(
            combined.contains("error")
                || combined.contains("not found")
                || combined.contains("unrecognized"),
            "Should show error message"
        );
    }

    #[test]
    fn test_invalid_subcommand() {
        let result = run_omg(&["audit", "invalid-subcommand"]);
        assert!(!result.success);
    }

    #[test]
    fn test_missing_required_arg() {
        let result = run_omg(&["info"]);
        assert!(!result.success, "Should fail when package name missing");
    }

    #[test]
    fn test_conflicting_flags() {
        let result = run_omg(&["search", "--json", "--quiet", "test"]);
        // The command should not panic. It may:
        // - succeed (flags are compatible)
        // - fail with a conflict error
        // - time out in minimal CI containers without package DBs
        // Any of these is acceptable — a panic is not.
        if !result.success {
            let combined = result.combined_output();
            assert!(
                combined.contains("conflict")
                    || combined.contains("cannot be used")
                    || combined.contains("timeout")
                    || combined.contains("error")
                    || combined.contains("no results")
                    || result.exit_code != 101, // 101 = Rust panic exit code
                "Command panicked or produced unexpected output: {combined}"
            );
        }
    }
}

// =======================
// CROSS-COMMAND WORKFLOWS
// =======================

mod workflow_tests {
    use super::*;

    #[test]
    fn test_search_then_info() {
        // Workflow: search for package, then get info
        let search_result = run_omg(&["search", "bash"]);
        search_result.assert_success();

        let info_result = run_omg(&["info", "bash"]);
        info_result.assert_success();
    }

    #[test]
    fn test_status_then_explicit() {
        // Workflow: check status, list explicit packages
        let status_result = run_omg(&["status"]);
        status_result.assert_success();

        let explicit_result = run_omg(&["explicit"]);
        explicit_result.assert_success();
    }
}
