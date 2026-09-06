//! Custom assertions for OMG tests.
//!
//! Keep these helpers strict: every assertion must be able to fail on a real
//! regression. Do not add `||` fallbacks that accept unrelated output as a
//! pass; inline the exact expectation at the call site instead.

use super::CommandResult;

/// Assert normal CLI completion, without requiring command success.
///
/// These input-robustness tests allow success/help (0), an application error
/// from `src/bin/omg.rs::finish` (1), or a clap argument error (2). They do not
/// run arbitrary child commands whose exit statuses would need another policy.
/// A signal (-1 in `CommandResult`), Rust panic (101 or panic output), or the
/// harness timeout marker must fail even if another field looks successful.
pub fn assert_process_completed(result: &CommandResult) {
    assert!(
        !result.stderr.contains("[test harness timeout]"),
        "Command timed out: {result:?}"
    );
    assert!(
        matches!(result.exit_code, 0..=2),
        "Command did not complete with an ordinary CLI exit code (0, 1, or 2): {result:?}"
    );
    assert!(
        !result.contains("panicked at"),
        "Command emitted a Rust panic diagnostic: {result:?}"
    );
}

/// Assert that a package search succeeds and lists every expected package.
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

/// Assert that package info succeeded and shows the package together with a
/// version-like token (`major.minor[...]`).
///
/// The old heuristic (`stdout.contains('.')`) matched almost any prose; this
/// requires an actual dotted numeric token such as `6.0.2` or `2.6.1`.
pub fn assert_package_info(result: &CommandResult, package_name: &str) {
    result.assert_success();
    assert!(
        result.stdout_contains(package_name),
        "Package info should contain name '{package_name}'"
    );
    let has_version_token = result.stdout.split_whitespace().any(|token| {
        let digits = token.chars().filter(char::is_ascii_digit).count();
        digits >= 2 && token.contains('.')
    });
    assert!(
        has_version_token,
        "Package info should contain a version-like token (e.g. '6.0.2'), got:\n{}",
        result.stdout
    );
}

/// Assert that an `audit scan` completed successfully and reported its
/// findings (non-empty stdout).
///
/// Failure output must surface in the panic message instead of being
/// laundered through keyword alternatives.
pub fn assert_audit_scan_completed(result: &CommandResult) {
    result.assert_success();
    assert!(
        !result.stdout.trim().is_empty(),
        "audit scan must produce report output:\n{}",
        result.stderr
    );
}
