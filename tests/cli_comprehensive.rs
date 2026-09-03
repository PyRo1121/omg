//! Broad CLI smoke and behavior contracts for OMG commands.

#![cfg(feature = "arch")]

pub mod common;

use clap::CommandFactory;
use common::*;
use omg_lib::cli::Cli;

fn command_paths() -> Vec<Vec<String>> {
    fn collect(command: &clap::Command, prefix: &mut Vec<String>, paths: &mut Vec<Vec<String>>) {
        for subcommand in command.get_subcommands() {
            prefix.push(subcommand.get_name().to_string());
            paths.push(prefix.clone());
            collect(subcommand, prefix, paths);
            prefix.pop();
        }
    }

    let command = Cli::command();
    let mut paths = Vec::new();
    collect(&command, &mut Vec::new(), &mut paths);
    paths
}

#[test]
fn every_declared_command_renders_binary_help() {
    let paths = command_paths();
    assert!(
        paths.len() >= 75,
        "unexpectedly small command tree: {paths:?}"
    );

    for path in paths {
        let mut args: Vec<&str> = path.iter().map(String::as_str).collect();
        args.push("--help");
        let result = run_omg(&args);
        assert!(
            result.success && result.stdout.contains("Usage:"),
            "`omg {}` did not render help\nstdout: {}\nstderr: {}",
            args.join(" "),
            result.stdout,
            result.stderr
        );
    }
}

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

    // Contract: installing a package that exists nowhere must FAIL and name the
    // cause (observed: "Error: Package not found in official repos"). The old
    // `!success || ...` disjunction also passed when install wrongly succeeded.
    #[test]
    fn test_install_nonexistent() {
        let result = run_omg(&[
            "install",
            "--yes",
            "package-that-definitely-does-not-exist-12345",
        ]);
        result.assert_failure();
        let combined = result.combined_output();
        assert!(
            combined.to_lowercase().contains("not found"),
            "Failure must name the missing package cause: {combined}"
        );
    }

    // Contract: dry-run exits 0 and explicitly promises no changes
    // (observed: "No changes will be made (dry run)").
    #[test]
    fn test_install_dry_run() {
        let result = run_omg(&["install", "--dry-run", "pacman"]);
        result.assert_success();
        let combined = result.combined_output();
        assert!(
            combined
                .to_lowercase()
                .contains("no changes will be made (dry run)"),
            "Dry run must state that no changes are made: {combined}"
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
        let combined = result.combined_output();
        assert!(
            (result.success && combined.to_lowercase().contains("remov"))
                || (!result.success
                    && (combined.to_lowercase().contains("not found")
                        || combined.to_lowercase().contains("not installed")
                        || combined.to_lowercase().contains("error"))),
            "Nonexistent removal should report an idempotent removal or explain the error: {combined}"
        );
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

    // Contract: --check mode exits 0 and announces check-only operation
    // (observed: "Checking for updates (no sync)").
    #[test]
    fn test_update_dry_run() {
        let result = run_omg(&["update", "--check"]);
        result.assert_success();
        result.assert_stdout_contains("Checking for updates");
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
        // Fish hook generation in src/hooks/mod.rs uses `source` instead of
        // the `eval` emitted for POSIX shells.
        result.assert_stdout_contains("source");
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
        let output = result.combined_output();
        assert!(
            result.success || output.to_lowercase().contains("no tools"),
            "Tool list should succeed or explain that no tools are installed: {output}"
        );
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
    fn test_account_help() {
        let result = run_omg(&["account", "--help"]);
        result.assert_success();
        result.assert_stdout_contains("link");
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
        // Should render the configuration header with real settings
        result.assert_success();
        result.assert_stdout_contains("OMG Configuration");
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
        // On Unix, daemon-status always exits 0 and prints its header,
        // whether the daemon is reachable or not (daemon_status.rs:17-90).
        result.assert_success();
        result.assert_stdout_contains("Daemon Status");
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

    // REMOVED (WRONG-CONTRACT): `self-update --check` — there is no --check flag
    // (src/cli/args.rs:484-491), so this gated test failed whenever network tests
    // were enabled. The downgrade-protection replacement lives in
    // e2e_system_commands.rs::test_self_update_downgrade_protection.

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

    // WRONG-CONTRACT FIX: `clean` takes flags (--cache/--orphans), not positional
    // subcommands (src/cli/args.rs:172-189). The old invocations were clap errors
    // whose message happened to contain "cache"/"orphans", so they passed vacuously.
    #[test]
    fn test_clean_cache_dry_run() {
        let result = run_omg(&["clean", "--cache", "--dry-run"]);
        result.assert_success();
        let output = result.stdout;
        assert!(
            output.contains("Would clear package cache"),
            "Dry run must preview the cache cleanup: {output}"
        );
        assert!(
            output.contains("No changes made (dry run)"),
            "Dry run must promise no mutations: {output}"
        );
    }

    #[test]
    fn test_clean_orphans_dry_run() {
        let result = run_omg(&["clean", "--orphans", "--dry-run"]);
        result.assert_success();
        let output = result.stdout;
        assert!(
            output.contains("Would remove") && output.to_lowercase().contains("orphan"),
            "Dry run must preview orphan removal: {output}"
        );
        assert!(
            output.contains("No changes made (dry run)"),
            "Dry run must promise no mutations: {output}"
        );
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

    // RE-CONTRACTED: --json/--quiet are GLOBAL args (src/cli/args.rs:24-29), not
    // conflicting search flags — the invocation is valid and must exit 0 with
    // machine-readable JSON on stdout.
    #[test]
    fn test_global_json_flag_emits_json() {
        let result = run_omg(&["search", "--json", "--quiet", "test"]);
        result.assert_success();
        let parsed: serde_json::Value =
            serde_json::from_str(result.stdout.trim()).expect("search --json must emit valid JSON");
        assert!(
            parsed.is_array(),
            "search --json must emit a JSON array, got: {parsed}"
        );
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
        let search_result = run_omg(&["search", "git"]);
        search_result.assert_success();

        let info_result = run_omg(&["info", "git"]);
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
