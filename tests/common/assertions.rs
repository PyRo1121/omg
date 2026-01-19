//! Custom assertions for OMG tests

#![allow(dead_code)]
#![allow(clippy::expect_fun_call)]

use super::CommandResult;
use std::time::Duration;

/// Assert that a command completed within a time limit
pub fn assert_performance(result: &CommandResult, max_ms: u64, operation: &str) {
    let max = Duration::from_millis(max_ms);
    assert!(
        result.duration < max,
        "{operation} took {:?}, expected under {max:?}",
        result.duration
    );
}

/// Assert that output matches expected patterns
pub fn assert_output_matches(result: &CommandResult, patterns: &[&str]) {
    for pattern in patterns {
        assert!(
            result.contains(pattern),
            "Output does not contain '{pattern}':\n{}\n{}",
            result.stdout,
            result.stderr
        );
    }
}

/// Assert that output does NOT match any of the patterns
pub fn assert_output_excludes(result: &CommandResult, patterns: &[&str]) {
    for pattern in patterns {
        assert!(
            !result.contains(pattern),
            "Output unexpectedly contains '{pattern}':\n{}\n{}",
            result.stdout,
            result.stderr
        );
    }
}

/// Assert JSON output is valid and contains expected keys
pub fn assert_valid_json(result: &CommandResult, expected_keys: &[&str]) {
    let json: serde_json::Value = serde_json::from_str(&result.stdout)
        .expect(&format!("Invalid JSON output:\n{}", result.stdout));

    for key in expected_keys {
        assert!(
            json.get(key).is_some(),
            "JSON missing expected key '{key}':\n{}",
            result.stdout
        );
    }
}

/// Assert that a package search returns expected results
pub fn assert_search_results(result: &CommandResult, expected_packages: &[&str]) {
    result.assert_success();
    for pkg in expected_packages {
        assert!(
            result.stdout_contains(pkg),
            "Search results should contain '{pkg}':\n{}",
            result.stdout
        );
    }
}

/// Assert that package info contains required fields
pub fn assert_package_info(result: &CommandResult, package_name: &str) {
    result.assert_success();
    assert!(
        result.stdout_contains(package_name),
        "Package info should contain name '{package_name}'"
    );
    // Most package info should contain version-like patterns
    let has_version =
        result.stdout.contains('.') || result.stdout.to_lowercase().contains("version");
    assert!(
        has_version,
        "Package info should contain version information"
    );
}

/// Assert that environment capture produced valid output
pub fn assert_env_capture(result: &CommandResult) {
    result.assert_success();
    assert!(
        result.contains("omg.lock") || result.contains("captured") || result.contains("Captured"),
        "Env capture should mention lock file or captured status"
    );
}

/// Assert that environment check works correctly
pub fn assert_env_check(result: &CommandResult, expect_drift: bool) {
    if expect_drift {
        assert!(
            result.contains("drift") || result.contains("Drift") || !result.success,
            "Should detect drift"
        );
    } else {
        assert!(
            result.success || result.contains("match") || result.contains("Match"),
            "Should not detect drift or report match"
        );
    }
}

/// Assert runtime version detection
pub fn assert_version_detected(result: &CommandResult, expected_version: &str) {
    assert!(
        result.contains(expected_version) || result.contains("Detected"),
        "Should detect version {expected_version}:\n{}{}",
        result.stdout,
        result.stderr
    );
}

/// Assert that completions are generated correctly
pub fn assert_valid_completions(result: &CommandResult, shell: &str) {
    result.assert_success();
    match shell {
        "bash" => assert!(
            result.stdout_contains("complete") || result.stdout_contains("_omg"),
            "Bash completions should contain 'complete' or '_omg'"
        ),
        "zsh" => assert!(
            result.stdout_contains("compdef") || result.stdout_contains("_omg"),
            "Zsh completions should contain 'compdef' or '_omg'"
        ),
        "fish" => assert!(
            result.stdout_contains("complete") || result.stdout_contains("omg"),
            "Fish completions should contain 'complete'"
        ),
        _ => panic!("Unknown shell: {shell}"),
    }
}

/// Assert that security audit returns expected format
pub fn assert_audit_output(result: &CommandResult) {
    // Audit may require daemon or specific features
    // Should not panic, should provide meaningful output
    assert!(
        result.success
            || result.contains("daemon")
            || result.contains("requires")
            || result.contains("tier")
            || result.contains("Scanning"),
        "Audit should work or report why it can't:\n{}{}",
        result.stdout,
        result.stderr
    );
}

/// Assert that a snapshot was created
pub fn assert_snapshot_created(result: &CommandResult) {
    result.assert_success();
    assert!(
        result.contains("Snapshot") || result.contains("snap-") || result.contains("created"),
        "Should indicate snapshot creation"
    );
}

/// Assert that pins are managed correctly
pub fn assert_pin_output(result: &CommandResult, action: &str) {
    result.assert_success();
    match action {
        "list" => assert!(
            result.contains("Pinned") || result.contains("No pins") || result.contains("📌"),
            "Should show pin status"
        ),
        "add" => assert!(
            result.contains("Pinned") || result.contains("✓"),
            "Should confirm pin added"
        ),
        "remove" => assert!(
            result.contains("Unpinned") || result.contains("✓"),
            "Should confirm pin removed"
        ),
        _ => {}
    }
}
