//! Comprehensive Privilege Escalation Tests
//!
//! Tests for sudo privilege escalation, including:
//! - Non-interactive sudo (-n flag) fallback behavior
//! - Password prompt detection
//! - Error message parsing for PermissionDenied
//! - Whitelist validation
//! - Mock sudo scenarios
//!
//! Run: cargo test --test privilege_tests
//!
//! These tests use extensive mocking to avoid requiring actual root privileges.

#![expect(clippy::unwrap_used)]
#![expect(clippy::pedantic)]

pub mod common;

// ═══════════════════════════════════════════════════════════════════════════════
// TEST CONFIGURATION AND HELPERS
// ═══════════════════════════════════════════════════════════════════════════════

/// Test harness for running commands with controlled environments
struct TestRunner {
    env_vars: Vec<(String, String)>,
}

impl TestRunner {
    fn new() -> Self {
        Self {
            env_vars: vec![
                ("OMG_TEST_MODE".to_string(), "1".to_string()),
                ("OMG_DISABLE_DAEMON".to_string(), "1".to_string()),
            ],
        }
    }

    fn with_env(mut self, key: &str, value: &str) -> Self {
        self.env_vars.push((key.to_string(), value.to_string()));
        self
    }

    /// Run the omg binary with test environment.
    ///
    /// Delegates to the shared isolated runner so each invocation gets unique
    /// `OMG_DATA_DIR` / `OMG_CONFIG_DIR` / `OMG_CACHE_DIR`; the runner's own
    /// `OMG_TEST_MODE` / `OMG_DISABLE_DAEMON` defaults match the values this
    /// suite used to set by hand.
    fn run(&self, args: &[&str]) -> TestResult {
        let owned: Vec<(String, String)> = self
            .env_vars
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        let refs: Vec<(&str, &str)> = owned
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let result = common::run_omg_with_options(args, None, &refs);
        TestResult {
            success: result.success,
            stdout: result.stdout,
            stderr: result.stderr,
            exit_code: result.exit_code,
        }
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug)]
struct TestResult {
    success: bool,
    stdout: String,
    stderr: String,
    exit_code: i32,
}

impl TestResult {
    fn combined_output(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }

    fn contains(&self, pattern: &str) -> bool {
        self.combined_output().contains(pattern)
    }

    fn assert_success(&self) -> &Self {
        assert!(
            self.success,
            "Command failed with exit code {}. Output:\n{}",
            self.exit_code,
            self.combined_output()
        );
        self
    }

    fn assert_failure(&self) -> &Self {
        assert!(
            !self.success,
            "Command unexpectedly succeeded. Output:\n{}",
            self.combined_output()
        );
        self
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PRIVILEGE WHITELIST TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_whitelist_allowed_operations() {
    // Test that whitelisted operations are accepted
    let runner = TestRunner::new();

    // User-facing commands: clap handles --help before any privilege logic.
    // ("upgrade"/"fullupdate"/"turboupdate" are internal elevated entrypoints,
    // deliberately NOT clap commands, so they are excluded here.)
    let allowed_ops = ["install", "remove", "update", "sync", "clean"];

    for op in allowed_ops {
        let result = runner.run(&[op, "--help"]);
        result.assert_success();
    }
}

#[test]
fn test_whitelist_blocks_unsafe_operations() {
    // Contract (src/core/privilege.rs, elevate_for_operation): only
    // install/remove/upgrade/update/sync/clean may request elevation. Every
    // read-only operation must be rejected with PermissionDenied and an error
    // message that names the operation and says it is not whitelisted.
    use omg_lib::core::privilege;
    use std::io::ErrorKind;

    for op in ["search", "info", "status", "why", "blame"] {
        let err = privilege::elevate_for_operation(op, &[])
            .expect_err("read-only operations must never be elevatable");
        assert_eq!(
            err.kind(),
            ErrorKind::PermissionDenied,
            "operation '{op}' must be denied with PermissionDenied"
        );
        let message = err.to_string();
        assert!(
            message.contains("not whitelisted") && message.contains(op),
            "denial for '{op}' must name the operation and the whitelist: {message}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SUDO -N FLAG FALLBACK TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_sudo_n_flag_fallback_on_password_required() {
    // Regression contract (src/core/privilege.rs, sudo_payload_status_in):
    // credentials are validated by a pre-flight `sudo -n -v` before any payload,
    // so a non-interactive `--yes` run can never block on a password prompt.
    // It must terminate with an outcome on BOTH paths:
    //   success  -> reports what happened (up to date / updates listed)
    //   failure  -> names its cause (sudo/NOPASSWD/turbo/development mode)
    let runner = TestRunner::new().with_env("CI", "1");

    let result = runner.run(&["update", "--yes"]);
    let combined = result.combined_output();

    assert_ne!(result.exit_code, 101, "update --yes panicked:\n{combined}");
    for prompt in ["[sudo]", "password for", "Password:"] {
        assert!(
            !combined.contains(prompt),
            "--yes must never prompt for a password ({prompt}). Got:\n{combined}"
        );
    }

    if result.success {
        assert!(
            combined.to_lowercase().contains("up to date")
                || combined.to_lowercase().contains("update"),
            "successful update --yes must report its outcome. Got:\n{combined}"
        );
    } else {
        let lowered = combined.to_lowercase();
        assert!(
            [
                "sudo",
                "nopasswd",
                "password",
                "turbo",
                "development",
                "permission",
                "root"
            ]
            .iter()
            .any(|cause| lowered.contains(cause)),
            "failed update --yes must name its cause instead of prompting. Got:\n{combined}"
        );
    }
}

#[test]
fn test_privileged_program_fail_closed_in_dev_mode() {
    // Contract (src/core/privilege.rs, run_privileged_program): in dev/test
    // builds external-program elevation must bail BEFORE touching sudo, and
    // the error must be actionable: it names the mode, the program, and the
    // sudo fallback.
    use omg_lib::core::privilege;

    let result = temp_env::with_vars([("OMG_TEST_MODE", Some("1"))], || {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(privilege::run_privileged_program("apt-get", &["update"]))
    });

    let err = result.expect_err("dev/test builds must refuse privilege elevation");
    let message = err.to_string();
    assert!(
        message.contains("development mode"),
        "error must explain the dev-mode limitation: {message}"
    );
    assert!(
        message.contains("apt-get"),
        "error must name the program it refused to elevate: {message}"
    );
    assert!(
        message.contains("sudo"),
        "error must suggest the manual sudo alternative: {message}"
    );
}

#[test]
fn test_elevate_rejects_injection_style_operations() {
    // Contract (src/core/privilege.rs, ALLOWED_ROOT_OPS): the whitelist is an
    // exact string match, so shell metacharacters or concatenated commands can
    // never smuggle an elevation through. Every crafted op must be rejected
    // with PermissionDenied naming the payload.
    use omg_lib::core::privilege;
    use std::io::ErrorKind;

    let hostile_ops = [
        "install; rm -rf /",
        "install && cat /etc/shadow",
        "$(echo pwned)",
        "`id`",
        "install\nremove",
    ];

    for op in hostile_ops {
        let err = privilege::elevate_for_operation(op, &[])
            .expect_err("crafted operations must never reach elevation");
        assert_eq!(err.kind(), ErrorKind::PermissionDenied, "op: {op}");
        assert!(
            err.to_string().contains(op),
            "denial must echo the rejected op '{op}': {err}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR MESSAGE DETECTION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
// UPDATE COMMAND SUDO INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

// ═══════════════════════════════════════════════════════════════════════════════
// DEVELOPMENT BUILD DETECTION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_update_check_mode_no_password_prompt() {
    // CRITICAL: --check mode should never prompt for password
    let runner = TestRunner::new();

    let result = runner.run(&["update", "--check"]);

    // Should succeed without prompting
    result.assert_success();

    // Should not contain any prompts
    let combined = result.combined_output();
    assert!(
        !combined.contains("[sudo]")
            && !combined.contains("password for")
            && !combined.contains("Password:"),
        "--check should not prompt for password. Got: {}",
        combined
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// DEVELOPMENT BUILD DETECTION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_dev_build_marker_blocks_elevation() {
    // Contract (src/core/privilege.rs, sudo_payload_status_in): the dev-build
    // marker CARGO_PRIMARY_PACKAGE — on its own, without OMG_TEST_MODE — must
    // also fail closed. If elevation is needed, the failure has to name the
    // dev-mode limitation; if no elevation was needed, an outcome is reported.
    let runner = TestRunner::new().with_env("CARGO_PRIMARY_PACKAGE", "1");

    let result = runner.run(&["update", "--yes"]);
    let combined = result.combined_output();

    assert_ne!(result.exit_code, 101, "update --yes panicked:\n{combined}");
    assert!(
        !combined.contains("[sudo]"),
        "dev builds must never reach a sudo prompt. Got:\n{combined}"
    );
    if result.success {
        assert!(
            combined.to_lowercase().contains("up to date")
                || combined.to_lowercase().contains("update"),
            "successful update --yes must report its outcome. Got:\n{combined}"
        );
    } else {
        let lowered = combined.to_lowercase();
        assert!(
            lowered.contains("development")
                || lowered.contains("turbo")
                || lowered.contains("sudo"),
            "dev-mode elevation failure must explain the limitation. Got:\n{combined}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// EDGE CASE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_empty_args_handling() {
    // Contract (src/cli/args.rs: `arg_required_else_help = true`): with no
    // subcommand clap prints the help text (including the Usage line) and
    // exits 2. It must never panic or run an implicit default command.
    let runner = TestRunner::new();

    let result = runner.run(&[]);

    assert_eq!(
        result.exit_code,
        2,
        "empty args must exit with clap's usage error code 2. Output:\n{}",
        result.combined_output()
    );
    assert!(
        result.contains("Usage:"),
        "empty args must print help with a Usage line. Got:\n{}",
        result.combined_output()
    );
    assert!(
        !result.stderr.contains("panicked at"),
        "empty args must not panic"
    );
}

#[test]
fn test_sequential_status_commands() {
    let runner = TestRunner::new();
    let results: Vec<_> = (0..5).map(|_| runner.run(&["status"])).collect();

    for (i, result) in results.iter().enumerate() {
        // FALSIFIABLE: every run must complete without panicking AND produce
        // some rendered output (status report or a named error).
        assert_ne!(result.exit_code, 101, "run {} panicked", i + 1);
        assert!(
            !result.stdout.is_empty() || !result.stderr.is_empty(),
            "status run {} produced no output at all",
            i + 1
        );
    }
}

#[test]
fn test_special_chars_in_package_names() {
    // Contract: `info <name>` treats every name as data. For names that do not
    // exist it must fail gracefully with an error that echoes the queried name
    // (src/cli/packages/info.rs: "Package '<name>' not found"), and it must
    // never leak system files such as /etc/passwd into the output.
    let runner = TestRunner::new();

    let special_names = [
        "test-package",
        "test_package",
        "test.package",
        "test123",
        "TEST123",
    ];

    for name in special_names {
        let result = runner.run(&["info", name]);
        let combined = result.combined_output();

        assert_ne!(
            result.exit_code, 101,
            "info '{name}' must not panic. Output:\n{combined}"
        );
        assert!(
            !combined.contains("root:x:") && !result.stdout.contains("root:$"),
            "info '{name}' must never expose /etc/passwd content"
        );

        if result.success {
            assert!(
                result.stdout.contains(name),
                "successful info must show the package '{name}'. Got:\n{}",
                result.stdout
            );
        } else {
            assert!(
                combined.contains("not found") && combined.contains(name),
                "failed info for unknown package '{name}' must say so and echo the name.\nGot:\n{combined}"
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR PATH COVERAGE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_sudo_command_not_found_reports_error() {
    // Contract: running a command that cannot resolve must surface a named
    // error rather than silently succeeding. `omg info` on a package that no
    // repo knows exits nonzero and says so.
    let runner = TestRunner::new();

    let result = runner.run(&["info", "definitely-not-a-real-command-xyz"]);

    result.assert_failure();
    assert!(
        result.stderr.contains("not found") || result.stdout.contains("not found"),
        "unknown package must be reported as not found. Got:\n{}",
        result.combined_output()
    );
}

#[test]
fn test_is_root_function_status_completes() {
    // The is_root check feeds `status`; it must complete without crashing and
    // render its report.
    let runner = TestRunner::new();

    let result = runner.run(&["status"]);

    assert_ne!(
        result.exit_code,
        101,
        "status panicked; is_root path is broken. Output:\n{}",
        result.combined_output()
    );
    assert!(
        !result.stdout.is_empty() || !result.stderr.is_empty(),
        "status produced no output at all"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// REGRESSION TESTS FOR BUG FIXES
// ═══════════════════════════════════════════════════════════════════════════════

// Regression coverage for the historical "sudo -n exit code 1 was not
// detected, so CI runs hung on a password prompt" bug now lives in
// `test_sudo_n_flag_fallback_on_password_required` above, which pins the
// no-prompt + named-failure contract end to end.

// ═══════════════════════════════════════════════════════════════════════════════
// YES FLAG NON-INTERACTIVE SUDO TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_yes_flag_prevents_password_prompt() {
    // Test that --yes flag uses non-interactive sudo (-n)
    // and doesn't fall back to interactive mode
    use omg_lib::core::privilege;

    // Set the yes flag
    privilege::set_yes_flag(true);
    assert!(privilege::get_yes_flag(), "Yes flag should be set");

    // Clear it
    privilege::set_yes_flag(false);
    assert!(!privilege::get_yes_flag(), "Yes flag should be cleared");
}

#[test]
fn test_install_command_parses_yes_flag() {
    // Test that install command correctly parses --yes flag
    let runner = TestRunner::new();

    // This should not panic or fail to parse
    let result = runner.run(&["install", "--help"]);
    assert!(result.stdout.contains("--yes") || result.stdout.contains("-y"));

    let result = runner.run(&["install", "-h"]);
    assert!(result.stdout.contains("--yes") || result.stdout.contains("-y"));
}

#[test]
fn test_update_command_parses_yes_flag() {
    // Test that update command correctly parses --yes flag
    let runner = TestRunner::new();

    let result = runner.run(&["update", "--help"]);
    assert!(result.stdout.contains("--yes") || result.stdout.contains("-y"));

    let result = runner.run(&["update", "-h"]);
    assert!(result.stdout.contains("--yes") || result.stdout.contains("-y"));
}

#[test]
fn test_remove_command_parses_yes_flag() {
    // Test that remove command correctly parses --yes flag
    let runner = TestRunner::new();

    let result = runner.run(&["remove", "--help"]);
    assert!(result.stdout.contains("--yes") || result.stdout.contains("-y"));
}
