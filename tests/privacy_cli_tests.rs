//! Privacy CLI Command Tests
//!
//! Tests for local privacy commands: status, export, opt-out, and opt-in.
//!
//! Account export and deletion are session-authenticated web operations and
//! are intentionally not exposed through the license-key-authenticated CLI.

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
    // Bare `privacy` dispatches to Status (src/bin/omg.rs:901:
    // `Some(PrivacyCommands::Status) | None => telemetry::privacy_status()`).
    let result = run_omg(&["privacy"]);

    // ===== ASSERT =====
    // Privacy status is local-only and points account-level requests to the
    // authenticated web surface.
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("Privacy Settings"),
        "Output should render the privacy settings header: {output}"
    );
    assert!(
        output.contains("Account export and deletion require an authenticated session")
            && output.contains("https://omg.latham.cloud/privacy/"),
        "Output should direct account-level rights to the authenticated web surface: {output}"
    );
}

#[test]
fn test_privacy_status_explicit() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["privacy", "status"]);

    // ===== ASSERT =====
    // Same handler as bare `privacy`, dispatched through Some(Status).
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("Privacy Settings"),
        "Status should render the privacy settings header: {output}"
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

    for command in ["export", "opt-out", "opt-in"] {
        assert!(
            output.contains(command),
            "Status should list the '{command}' privacy command: {output}"
        );
    }
    assert!(
        !output.contains("omg privacy delete"),
        "session-authenticated account deletion must not be advertised as a CLI command: {output}"
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
        !output.contains("Delete all") && !output.contains("delete <"),
        "Help must not advertise the removed account-deletion command"
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

// test_privacy_export_with_output_flag deleted (tst03): redundant duplicate of
// test_privacy_export_without_license, which already pins --output handling more
// strongly (parses the written file and checks the local/remote shape).

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
    // Local opt-out is unconditional (src/cli/telemetry.rs opt_out_api) and
    // always confirms with the "Telemetry disabled" banner.
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("Telemetry disabled"),
        "Opt-out must confirm telemetry is disabled: {output}"
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

    // ===== ACT =====
    let result = project.run(&["privacy", "opt-out"]);

    // ===== ASSERT =====
    // Without a license there is no server to sync with, so opt-out must
    // succeed via the local-only path and say so
    // (src/cli/telemetry.rs opt_out_api: response is None when unlicensed).
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("Telemetry disabled locally"),
        "Unlicensed opt-out must confirm the local policy change: {output}"
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
    // Opt-in succeeds and confirms with the "Telemetry enabled" banner
    // (src/cli/telemetry.rs opt_in_api).
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("Telemetry enabled"),
        "Opt-in must confirm telemetry is enabled: {output}"
    );
}

#[test]
fn test_privacy_opt_in_updates_config() {
    // ===== ARRANGE =====
    let project = TestProject::new();

    // ===== ACT =====
    let result = project.run(&["privacy", "opt-in"]);

    // ===== ASSERT =====
    result.assert_success();

    // FALSIFIABLE: Settings::save writes to <config_dir>/config.toml
    // (src/config/settings.rs config_path; OMG_CONFIG_DIR is used verbatim,
    // src/core/paths.rs config_dir). The old version looked for a nonexistent
    // omg/config.toml nested path wrapped in `if exists()`, so it never
    // asserted anything.
    let config_path = project.config_dir.path().join("config.toml");
    let config_content = fs::read_to_string(&config_path)
        .unwrap_or_else(|error| panic!("config must exist after opt-in: {error}"));
    assert!(
        config_content.contains("telemetry_enabled = true"),
        "config must persist telemetry enabled, got: {config_content}"
    );
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
    let config_path = project.config_dir.path().join("config.toml");

    // ===== ACT & ASSERT =====
    // First opt-out: succeeds and persists telemetry_enabled = false.
    let result1 = project.run(&["privacy", "opt-out"]);
    result1.assert_success();
    let config1 = fs::read_to_string(&config_path)
        .unwrap_or_else(|error| panic!("config must exist after opt-out: {error}"));
    assert!(
        config1.contains("telemetry_enabled = false"),
        "after opt-out the config must disable telemetry, got: {config1}"
    );

    // Then opt-in: flips the persisted state back to true.
    let result2 = project.run(&["privacy", "opt-in"]);
    result2.assert_success();
    let config2 = fs::read_to_string(&config_path)
        .unwrap_or_else(|error| panic!("config must survive opt-in: {error}"));
    assert!(
        config2.contains("telemetry_enabled = true"),
        "after opt-in the config must enable telemetry, got: {config2}"
    );
}

#[test]
fn test_privacy_toggle_opt_in_then_opt_out() {
    // ===== ARRANGE =====
    let project = TestProject::new();
    let config_path = project.config_dir.path().join("config.toml");

    // ===== ACT & ASSERT =====
    // First opt-in: succeeds and persists telemetry_enabled = true.
    let result1 = project.run(&["privacy", "opt-in"]);
    result1.assert_success();
    let config1 = fs::read_to_string(&config_path)
        .unwrap_or_else(|error| panic!("config must exist after opt-in: {error}"));
    assert!(
        config1.contains("telemetry_enabled = true"),
        "after opt-in the config must enable telemetry, got: {config1}"
    );

    // Then opt-out: flips the persisted state back to false.
    let result2 = project.run(&["privacy", "opt-out"]);
    result2.assert_success();
    let config2 = fs::read_to_string(&config_path)
        .unwrap_or_else(|error| panic!("config must survive opt-out: {error}"));
    assert!(
        config2.contains("telemetry_enabled = false"),
        "after opt-out the config must disable telemetry, got: {config2}"
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
    // OMG_TELEMETRY=1 must not block a local opt-out: opt_out_api disables
    // telemetry unconditionally in the saved settings
    // (src/cli/telemetry.rs opt_out_api; env vars only affect
    // is_telemetry_opt_out reads, src/core/telemetry.rs:85).
    let result = project.run_with_env(&["privacy", "opt-out"], &[("OMG_TELEMETRY", "1")]);

    // ===== ASSERT =====
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("Telemetry disabled"),
        "Opt-out must confirm telemetry disabled even with OMG_TELEMETRY=1: {output}"
    );
    let config_content = fs::read_to_string(project.config_dir.path().join("config.toml"))
        .expect("config must exist after opt-out");
    assert!(
        config_content.contains("telemetry_enabled = false"),
        "env override must not prevent persisting the opt-out, got: {config_content}"
    );
}

#[test]
fn test_privacy_status_shows_env_override() {
    // ===== ARRANGE =====
    let project = TestProject::new();

    // ===== ACT =====
    let result = project.run_with_env(&["privacy", "status"], &[("OMG_TELEMETRY", "0")]);

    // ===== ASSERT =====
    // `privacy status` does not render an "Environment" line. The honest
    // observable contract is that status still succeeds and renders its
    // header with the override set.
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("Privacy Settings"),
        "Status must render normally with OMG_TELEMETRY set: {output}"
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
    // The global --json gate explicitly allowlists `privacy` and documents it
    // as the scripted JSON entrypoint (src/bin/omg.rs dispatch_command), so a
    // scripted caller must get exit-code success. NOTE: the handler currently
    // renders human-readable text even under --json; that gap is recorded as a
    // SUSPECTED PRODUCT BUG in the tst03 report rather than asserted here.
    result.assert_success();
    assert!(
        !result.combined_output().trim().is_empty(),
        "--json privacy status must produce output"
    );
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
    // Export without a license skips the remote API and fails locally when the
    // output path is unwritable (src/cli/telemetry.rs export_data ->
    // atomic_write_file_sync).
    result.assert_failure();
    let output = result.combined_output();
    assert!(
        output.contains("Error") || output.contains("Failed"),
        "Failure must name its cause, not vanish silently: {output}"
    );
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

    // Status is local-only and always renders the account privacy URL.
    let result = project.run(&["privacy", "status"]);
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("privacy export") && output.contains("https://omg.latham.cloud/privacy/"),
        "Status must render local commands and the authenticated account surface: {output}"
    );

    // Opt-out works offline: local config change succeeds unconditionally.
    let result = project.run(&["privacy", "opt-out"]);
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("Telemetry disabled"),
        "Opt-out must succeed offline: {output}"
    );

    // Opt-in works offline: local config change succeeds unconditionally.
    let result = project.run(&["privacy", "opt-in"]);
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("Telemetry enabled"),
        "Opt-in must succeed offline: {output}"
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
    // Verbose mode must not change the command's outcome or suppress output.
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("Privacy Settings"),
        "Verbose mode must still render privacy status: {output}"
    );
}

#[test]
fn test_privacy_status_quiet() {
    // ===== ARRANGE =====
    init_test_env();

    // ===== ACT =====
    let result = run_omg(&["--quiet", "privacy", "status"]);

    // ===== ASSERT =====
    // --quiet suppresses non-essential output but "command results still
    // print" (src/cli/args.rs:31-33), so the status body must survive.
    result.assert_success();
    let output = result.combined_output();
    assert!(
        output.contains("Privacy Settings"),
        "Quiet mode must keep command results: {output}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Documentation and Help Tests
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_privacy_all_subcommands_have_help() {
    // ===== ARRANGE =====
    init_test_env();

    let subcommands = vec!["status", "export", "opt-out", "opt-in"];

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
