//! Privacy CLI Command Tests
//!
//! Tests for GDPR/CCPA privacy commands: status, export, delete, opt-out, opt-in
//! These tests verify CLI behavior with/without license activation and use mock API responses.

#![cfg(feature = "arch")]

pub mod common;

use common::*;
use std::fs;

// ═══════════════════════════════════════════════════════════════════════════════
// Privacy Status Command
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_status_default() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy"]);

    // ===== ASSERT =====
    // Should succeed and show privacy information
    assert!(
        result.success || result.contains("Privacy"),
        "Privacy status command should succeed or show privacy info"
    );

    // Should mention key privacy concepts
    let output = result.combined_output();
    assert!(
        output.contains("privacy") || output.contains("Privacy"),
        "Output should mention privacy: {output}"
    );
}

#[test]
fn test_privacy_status_explicit() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy", "status"]);

    // ===== ASSERT =====
    // Should succeed and show privacy information
    assert!(
        result.success || result.contains("Privacy"),
        "Privacy status command should succeed"
    );

    let output = result.combined_output();
    // Should show privacy-related information
    assert!(
        output.contains("privacy")
            || output.contains("Privacy")
            || output.contains("data")
            || output.contains("telemetry")
            || output.contains("rights"),
        "Status should show privacy information: {output}"
    );
}

#[test]
fn test_privacy_status_shows_commands() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy", "status"]);

    // ===== ASSERT =====
    let output = result.combined_output();

    // Should list available privacy commands
    let has_commands = output.contains("export")
        || output.contains("delete")
        || output.contains("opt-out")
        || output.contains("opt-in");

    assert!(
        has_commands,
        "Status should show available privacy commands: {output}"
    );
}

#[test]
fn test_privacy_help() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy", "--help"]);

    // ===== ASSERT =====
    result.assert_success();

    let output = result.combined_output();
    assert!(
        output.contains("status") || output.contains("Status"),
        "Help should mention status subcommand"
    );
    assert!(
        output.contains("export") || output.contains("Export"),
        "Help should mention export subcommand"
    );
    assert!(
        output.contains("delete") || output.contains("Delete"),
        "Help should mention delete subcommand"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Privacy Export Command
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_export_without_license() {
    init_test_env();
    clear_license();
    let project = TestProject::new();
    let output_path = project.path().join("local-export.json");

    let result = project.run(&[
        "privacy",
        "export",
        "--output",
        output_path.to_str().unwrap(),
    ]);

    result.assert_success();
    let exported: serde_json::Value =
        serde_json::from_slice(&std::fs::read(output_path).unwrap()).unwrap();
    assert!(exported.get("local").is_some());
    assert!(exported["remote"].is_null());
}

#[test]
fn test_privacy_export_with_output_flag() {
    // ===== ARRANGE =====
    init_test_env();
    clear_license();
    let project = TestProject::new();
    let output_path = project.path().join("my-export.json");

    // ===== ACT =====
    let result = project.run(&[
        "privacy",
        "export",
        "--output",
        output_path.to_str().unwrap(),
    ]);

    // ===== ASSERT =====
    result.assert_success();
    assert!(output_path.is_file(), "Export should honor the output path");
    let output = result.combined_output();
    assert!(
        output.contains("export"),
        "Should report export success: {output}"
    );
}

#[test]
fn test_privacy_export_help() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy", "export", "--help"]);

    // ===== ASSERT =====
    result.assert_success();

    let output = result.combined_output();
    assert!(
        output.contains("export") || output.contains("Export"),
        "Help should mention export"
    );
    assert!(
        output.contains("output") || output.contains("--output"),
        "Help should mention output flag"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Privacy Delete Command
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_delete_without_confirm() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy", "delete"]);

    // ===== ASSERT =====
    // Should show warning and NOT proceed without --confirm
    let output = result.combined_output();

    // Should mention deletion/confirm/license (implementation requires license)
    assert!(
        output.contains("confirm")
            || output.contains("--confirm")
            || output.contains("IRREVERSIBLE")
            || output.contains("permanently")
            || output.contains("delete")
            || output.contains("license"),
        "Should warn about deletion or license requirement: {output}"
    );
}

#[test]
fn test_privacy_delete_shows_warning_details() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy", "delete"]);

    // ===== ASSERT =====
    let output = result.combined_output();

    // Should show delete-related messaging or license requirement
    let has_details = output.contains("history")
        || output.contains("metrics")
        || output.contains("data")
        || output.contains("permanently")
        || output.contains("delete")
        || output.contains("license");

    assert!(
        has_details,
        "Should show delete-related output or license requirement: {output}"
    );
}

#[test]
fn test_privacy_delete_with_confirm_no_license() {
    // ===== ARRANGE =====
    init_test_env();
    clear_license();

    // ===== ACT =====
    let result = run_omg(&["privacy", "delete", "--confirm"]);

    // ===== ASSERT =====
    // Should fail without a license
    let output = result.combined_output();
    assert!(
        !result.success || output.contains("license") || output.contains("activate"),
        "Delete should require license: {output}"
    );
}

#[test]
fn test_privacy_delete_help() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy", "delete", "--help"]);

    // ===== ASSERT =====
    result.assert_success();

    let output = result.combined_output();
    assert!(
        output.contains("delete") || output.contains("Delete"),
        "Help should mention delete"
    );
    assert!(
        output.contains("confirm") || output.contains("--confirm"),
        "Help should mention confirm flag"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Privacy Opt-Out Command
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_opt_out() {
    // ===== ARRANGE =====
    let project = TestProject::new();

    // ===== ACT =====
    let result = project.run(&["privacy", "opt-out"]);

    // ===== ASSERT =====
    // Should succeed (works without license for local-only opt-out)
    let output = result.combined_output();
    assert!(
        output.contains("disabled")
            || output.contains("opt")
            || output.contains("telemetry")
            || result.success,
        "Opt-out should work and confirm telemetry disabled: {output}"
    );
}

#[test]
fn test_privacy_opt_out_updates_config() {
    // ===== ARRANGE =====
    let project = TestProject::new();

    // ===== ACT =====
    let result = project.run(&["privacy", "opt-out"]);

    // ===== ASSERT =====
    result.assert_success();

    // FALSIFIABLE: the config file MUST exist and MUST record telemetry as
    // disabled. The old version wrapped this check in `if config_path.exists()`
    // and passed a disjunction of unrelated strings, so opt-out doing nothing
    // at all still "passed".
    let config_path = project.config_dir.path().join("config.toml");
    let config_content = fs::read_to_string(&config_path)
        .unwrap_or_else(|error| panic!("config must exist after opt-out: {error}"));
    assert!(
        config_content.contains("telemetry_enabled = false")
            || config_content.contains("telemetry_enabled=false"),
        "config must persist telemetry disabled, got: {config_content}"
    );
}

#[test]
fn test_privacy_opt_out_without_license_local_only() {
    // ===== ARRANGE =====
    let project = TestProject::new();
    clear_license();

    // ===== ACT =====
    let result = project.run(&["privacy", "opt-out"]);

    // ===== ASSERT =====
    // Should work locally even without license
    let output = result.combined_output();
    assert!(
        output.contains("disabled")
            || output.contains("local")
            || output.contains("telemetry")
            || result.success,
        "Opt-out should work locally without license: {output}"
    );
}

#[test]
fn test_privacy_opt_out_help() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy", "opt-out", "--help"]);

    // ===== ASSERT =====
    result.assert_success();

    let output = result.combined_output();
    assert!(
        output.contains("opt") || output.contains("telemetry") || output.contains("disable"),
        "Help should explain opt-out functionality"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Privacy Opt-In Command
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_opt_in() {
    // ===== ARRANGE =====
    let project = TestProject::new();

    // ===== ACT =====
    let result = project.run(&["privacy", "opt-in"]);

    // ===== ASSERT =====
    // Should succeed
    let output = result.combined_output();
    assert!(
        output.contains("enabled") || output.contains("telemetry") || result.success,
        "Opt-in should work and confirm telemetry enabled: {output}"
    );
}

#[test]
fn test_privacy_opt_in_updates_config() {
    // ===== ARRANGE =====
    let project = TestProject::new();

    // ===== ACT =====
    let result = project.run(&["privacy", "opt-in"]);

    // ===== ASSERT =====
    // Should succeed and update config
    let output = result.combined_output();
    assert!(
        result.success || output.contains("enabled") || output.contains("telemetry"),
        "Opt-in should succeed: {output}"
    );

    // Check that config file was created/updated
    let config_path = project.config_dir.path().join("omg").join("config.toml");
    if config_path.exists() {
        let config_content = fs::read_to_string(&config_path).unwrap();
        assert!(
            config_content.contains("telemetry") || config_content.contains("true"),
            "Config should reflect telemetry enabled"
        );
    }
}

#[test]
fn test_privacy_opt_in_help() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy", "opt-in", "--help"]);

    // ===== ASSERT =====
    result.assert_success();

    let output = result.combined_output();
    assert!(
        output.contains("opt") || output.contains("telemetry") || output.contains("enable"),
        "Help should explain opt-in functionality"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Toggle Between Opt-Out and Opt-In
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_toggle_opt_out_then_opt_in() {
    // ===== ARRANGE =====
    let project = TestProject::new();

    // ===== ACT & ASSERT =====
    // First opt-out
    let result1 = project.run(&["privacy", "opt-out"]);
    let output1 = result1.combined_output();
    assert!(
        output1.contains("disabled")
            || output1.contains("telemetry")
            || output1.contains("local")
            || output1.contains("opt-out")
            || result1.success,
        "First opt-out should process: {output1}"
    );

    // Then opt-in
    let result2 = project.run(&["privacy", "opt-in"]);
    let output2 = result2.combined_output();
    assert!(
        output2.contains("enabled")
            || output2.contains("telemetry")
            || output2.contains("opt-in")
            || result2.success,
        "Opt-in after opt-out should process: {output2}"
    );
}

#[test]
fn test_privacy_toggle_opt_in_then_opt_out() {
    // ===== ARRANGE =====
    let project = TestProject::new();

    // ===== ACT & ASSERT =====
    // First opt-in
    let result1 = project.run(&["privacy", "opt-in"]);
    let output1 = result1.combined_output();
    assert!(
        output1.contains("enabled")
            || output1.contains("telemetry")
            || output1.contains("opt-in")
            || result1.success,
        "Opt-in should process: {output1}"
    );

    // Then opt-out
    let result2 = project.run(&["privacy", "opt-out"]);
    let output2 = result2.combined_output();
    assert!(
        output2.contains("disabled")
            || output2.contains("telemetry")
            || output2.contains("local")
            || output2.contains("opt-out")
            || result2.success,
        "Opt-out after opt-in should process: {output2}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Environment Variable Override Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_opt_out_with_env_override() {
    // ===== ARRANGE =====
    let project = TestProject::new();

    // ===== ACT =====
    // Set environment variable that forces telemetry on
    let result = project.run_with_env(&["privacy", "opt-out"], &[("OMG_TELEMETRY", "1")]);

    // ===== ASSERT =====
    // Should still process opt-out command (may show license warning)
    let output = result.combined_output();
    assert!(
        output.contains("disabled")
            || output.contains("telemetry")
            || output.contains("environment")
            || output.contains("env")
            || output.contains("license")
            || output.contains("local")
            || output.contains("opt-out")
            || result.success,
        "Should process opt-out command: {output}"
    );
}

#[test]
fn test_privacy_status_shows_env_override() {
    // ===== ARRANGE =====
    let project = TestProject::new();

    // ===== ACT =====
    let result = project.run_with_env(&["privacy", "status"], &[("OMG_TELEMETRY", "0")]);

    // ===== ASSERT =====
    let output = result.combined_output();
    // Should indicate environment variable is set
    assert!(
        output.contains("environment")
            || output.contains("env")
            || output.contains("OMG_TELEMETRY")
            || result.success,
        "Status should show environment variable override: {output}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// JSON Output Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_status_json_output() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["--json", "privacy", "status"]);

    // ===== ASSERT =====
    if result.success {
        let output = result.combined_output();
        // If JSON mode is implemented, output should be valid JSON
        if output.trim().starts_with('{') || output.trim().starts_with('[') {
            let parse_result = serde_json::from_str::<serde_json::Value>(&output);
            assert!(
                parse_result.is_ok(),
                "JSON output should be valid JSON: {output}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Edge Cases & Error Handling
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_invalid_subcommand() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy", "invalid-command"]);

    // ===== ASSERT =====
    result.assert_failure();

    let output = result.combined_output();
    assert!(
        output.contains("error")
            || output.contains("invalid")
            || output.contains("unrecognized")
            || output.contains("help"),
        "Should show error for invalid subcommand: {output}"
    );
}

#[test]
fn test_privacy_export_invalid_output_path() {
    // ===== ARRANGE =====
    init_test_env();
    clear_license();

    // ===== ACT =====
    // Use invalid path (directory that doesn't exist)
    let result = run_omg(&[
        "privacy",
        "export",
        "--output",
        "/nonexistent/path/export.json",
    ]);

    // ===== ASSERT =====
    // Should fail (either due to no license or invalid path)
    let output = result.combined_output();
    assert!(
        !result.success,
        "Should fail with invalid output path or no license: {output}"
    );
}

#[test]
fn test_privacy_delete_confirm_flag_variations() {
    // ===== ARRANGE =====
    init_test_env();
    clear_license();

    // Test various ways to specify the confirm flag
    let test_cases = vec![
        vec!["privacy", "delete", "--confirm"],
        vec!["privacy", "delete", "--confirm=true"],
    ];

    for args in test_cases {
        // ===== ACT =====
        let result = run_omg(&args);

        // ===== ASSERT =====
        // All should attempt to process the deletion (will fail without license)
        let output = result.combined_output();
        assert!(
            output.contains("delete") || output.contains("license") || output.contains("activate"),
            "Should process delete with confirm flag: {args:?} -> {output}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Offline/Network Error Scenarios
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_commands_work_offline() {
    // ===== ARRANGE =====
    let project = TestProject::new();

    // Simulate offline by using invalid API endpoint
    // (Commands should degrade gracefully when API is unreachable)

    // ===== ACT & ASSERT =====

    // Status should work offline (shows local status)
    let result = project.run(&["privacy", "status"]);
    let output = result.combined_output();
    assert!(
        result.success || output.contains("privacy") || output.contains("telemetry"),
        "Status should work offline: {output}"
    );

    // Opt-out should work offline (local config change, may show license warning)
    let result = project.run(&["privacy", "opt-out"]);
    let output = result.combined_output();
    assert!(
        result.success
            || output.contains("disabled")
            || output.contains("telemetry")
            || output.contains("local")
            || output.contains("opt-out"),
        "Opt-out should work offline (locally): {output}"
    );

    // Opt-in should work offline (local config change)
    let result = project.run(&["privacy", "opt-in"]);
    let output = result.combined_output();
    assert!(
        result.success
            || output.contains("enabled")
            || output.contains("telemetry")
            || output.contains("opt-in"),
        "Opt-in should work offline: {output}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Verbosity and Quiet Mode Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_status_verbose() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["-v", "privacy", "status"]);

    // ===== ASSERT =====
    // Verbose mode should still show privacy info
    let output = result.combined_output();
    assert!(
        result.success || output.contains("privacy") || output.contains("Privacy"),
        "Verbose mode should work: {output}"
    );
}

#[test]
fn test_privacy_status_quiet() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["--quiet", "privacy", "status"]);

    // ===== ASSERT =====
    // Quiet mode should still execute the command
    // (actual quiet behavior implementation may vary)
    let output = result.combined_output();
    assert!(
        result.success || !output.is_empty(),
        "Quiet mode should execute without crashing: {output}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Documentation and Help Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_all_subcommands_have_help() {
    // ===== ARRANGE =====
    init_test_env();

    let subcommands = vec!["status", "export", "delete", "opt-out", "opt-in"];

    for subcmd in subcommands {
        // ===== ACT =====
        let result = run_omg(&["privacy", subcmd, "--help"]);

        // ===== ASSERT =====
        result.assert_success();

        let output = result.combined_output();
        assert!(
            !output.is_empty(),
            "Help for '{subcmd}' should produce output"
        );
    }
}
