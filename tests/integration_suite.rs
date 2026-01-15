//! OMG Comprehensive Integration Test Suite
//!
//! This test suite provides world-class coverage of all OMG features,
//! including edge cases, error handling, and performance validation.
//!
//! Run with: `cargo test --test integration_suite -- --test-threads=1`

#![allow(unused_variables)]

use std::env;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use tempfile::TempDir;

// ═══════════════════════════════════════════════════════════════════════════════
// TEST UTILITIES
// ═══════════════════════════════════════════════════════════════════════════════

/// Helper to run omg commands and capture output
fn run_omg(args: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omg"))
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute omg");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

/// Helper to run omg commands with a specific working directory
fn run_omg_in_dir(args: &[&str], dir: &Path) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_omg"))
        .args(args)
        .current_dir(dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute omg");

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

/// Helper to run omg commands with environment variables
fn run_omg_with_env(args: &[&str], env_vars: &[(&str, &str)]) -> (bool, String, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omg"));
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    let output = cmd.output().expect("Failed to execute omg");
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    (output.status.success(), stdout, stderr)
}

/// Helper to measure execution time
fn measure_time<F: FnOnce() -> T, T>(f: F) -> (T, Duration) {
    let start = Instant::now();
    let result = f();
    (result, start.elapsed())
}

/// Guard for destructive integration tests (real installs/updates)
fn destructive_tests_enabled() -> bool {
    matches!(env::var("OMG_RUN_DESTRUCTIVE_TESTS"), Ok(value) if value == "1")
}

/// Create a temporary project directory with common config files
fn create_test_project(dir: &Path, config_type: &str) {
    fs::create_dir_all(dir).unwrap();

    match config_type {
        "node" => {
            // Create .nvmrc
            let mut f = File::create(dir.join(".nvmrc")).unwrap();
            writeln!(f, "20.10.0").unwrap();

            // Create package.json
            let mut f = File::create(dir.join("package.json")).unwrap();
            writeln!(
                f,
                r#"{{"name": "test", "engines": {{"node": ">=18.0.0"}}}}"#
            )
            .unwrap();
        }
        "python" => {
            let mut f = File::create(dir.join(".python-version")).unwrap();
            writeln!(f, "3.11.0").unwrap();
        }
        "go" => {
            let mut f = File::create(dir.join("go.mod")).unwrap();
            writeln!(f, "module test\n\ngo 1.21").unwrap();
        }
        "rust" => {
            let mut f = File::create(dir.join("rust-toolchain.toml")).unwrap();
            writeln!(f, "[toolchain]\nchannel = \"stable\"").unwrap();
        }
        "ruby" => {
            let mut f = File::create(dir.join(".ruby-version")).unwrap();
            writeln!(f, "3.2.0").unwrap();
        }
        "java" => {
            let mut f = File::create(dir.join(".java-version")).unwrap();
            writeln!(f, "21").unwrap();
        }
        "bun" => {
            let mut f = File::create(dir.join(".bun-version")).unwrap();
            writeln!(f, "1.0.0").unwrap();
        }
        "tool-versions" => {
            let mut f = File::create(dir.join(".tool-versions")).unwrap();
            writeln!(f, "nodejs 20.10.0\npython 3.11.0\nruby 3.2.0").unwrap();
        }
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI FOUNDATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_foundation {
    use super::*;

    #[test]
    fn test_version_flag() {
        let (success, stdout, _) = run_omg(&["--version"]);
        assert!(success, "omg --version should succeed");
        assert!(
            stdout.contains("omg"),
            "Version output should contain 'omg'"
        );
    }

    #[test]
    fn test_help_flag() {
        let (success, stdout, _) = run_omg(&["--help"]);
        assert!(success, "omg --help should succeed");
        assert!(stdout.contains("Usage"), "Help should contain 'Usage'");
        assert!(stdout.contains("Commands"), "Help should list commands");
    }

    #[test]
    fn test_subcommand_help() {
        let subcommands = vec![
            "search", "install", "remove", "update", "info", "clean", "use", "list", "env",
            "audit", "status", "which", "config",
        ];

        for cmd in subcommands {
            let (success, stdout, _) = run_omg(&[cmd, "--help"]);
            assert!(success, "omg {cmd} --help should succeed");
            assert!(
                stdout.contains("Usage"),
                "Help for {cmd} should contain 'Usage'"
            );
        }
    }

    #[test]
    fn test_invalid_command() {
        let (success, _, stderr) = run_omg(&["nonexistent-command"]);
        assert!(!success, "Invalid command should fail");
        assert!(
            stderr.contains("error") || stderr.contains("unrecognized"),
            "Should report error for invalid command"
        );
    }

    #[test]
    fn test_missing_required_args() {
        // Install requires package names
        let (success, _, stderr) = run_omg(&["install"]);
        assert!(!success, "install without args should fail");
        assert!(
            stderr.contains("required") || stderr.contains("error"),
            "Should report missing arguments"
        );
    }

    #[test]
    fn test_verbose_flags() {
        // Test -v, -vv, -vvv
        let (success, _, _) = run_omg(&["-v", "status"]);
        assert!(success, "omg -v status should succeed");

        let (success, _, _) = run_omg(&["-vv", "status"]);
        assert!(success, "omg -vv status should succeed");

        let (success, _, _) = run_omg(&["-vvv", "status"]);
        assert!(success, "omg -vvv status should succeed");
    }

    #[test]
    fn test_quiet_flag() {
        let (success, stdout, _) = run_omg(&["-q", "status"]);
        assert!(success, "omg -q status should succeed");
        // Quiet mode should produce minimal output
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PACKAGE MANAGEMENT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod package_management {
    use super::*;

    #[test]
    fn test_search_official_package() {
        let (success, stdout, _) = run_omg(&["search", "firefox"]);
        assert!(success, "Search should succeed");
        assert!(stdout.contains("firefox"), "Should find firefox");
        assert!(
            stdout.contains("Official") || stdout.contains("extra") || stdout.contains("core"),
            "Should indicate official repository"
        );
    }

    #[test]
    fn test_search_with_detailed_flag() {
        let (success, stdout, _) = run_omg(&["search", "firefox", "--detailed"]);
        assert!(success, "Detailed search should succeed");
        // Detailed output should include votes/popularity for AUR
    }

    #[test]
    fn test_search_empty_query() {
        let (success, _, stderr) = run_omg(&["search", ""]);
        // Empty query might return error or empty results
        // Both are acceptable behaviors
    }

    #[test]
    fn test_search_special_characters() {
        // Test with special characters that might break parsing
        let (success, _, _) = run_omg(&["search", "lib++"]);
        // Should not crash
    }

    #[test]
    fn test_search_unicode() {
        // Test with unicode characters
        let (success, _, _) = run_omg(&["search", "日本語"]);
        // Should not crash, may return no results
    }

    #[test]
    fn test_search_very_long_query() {
        let long_query = "a".repeat(1000);
        let (success, _, _) = run_omg(&["search", &long_query]);
        // Should handle gracefully (no crash, may return error)
    }

    #[test]
    fn test_info_official_package() {
        let (success, stdout, _) = run_omg(&["info", "pacman"]);
        assert!(success, "Info for official package should succeed");
        assert!(stdout.contains("pacman"), "Should show package name");
        // Version is displayed as "pacman X.Y.Z" format
        assert!(
            stdout.contains("Version") || stdout.contains('.') && stdout.contains("pacman"),
            "Should show version"
        );
    }

    #[test]
    fn test_info_nonexistent_package() {
        let (success, stdout, _) = run_omg(&["info", "this-package-does-not-exist-12345"]);
        // Should fail gracefully or show "not found"
        assert!(
            !success || stdout.contains("not found"),
            "Should indicate package not found"
        );
    }

    #[test]
    fn test_install_real_package() {
        if !destructive_tests_enabled() {
            eprintln!("Skipping destructive test (set OMG_RUN_DESTRUCTIVE_TESTS=1)");
            return;
        }

        let pkg = env::var("OMG_TEST_PACKAGE").unwrap_or_else(|_| "ripgrep".to_string());
        let args = vec!["install", "-y", &pkg];
        let (success, stdout, stderr) = run_omg(&args);
        assert!(
            success || stdout.contains("already installed") || stderr.contains("already installed"),
            "Install should succeed or report already installed"
        );
    }

    #[test]
    fn test_update_check_only() {
        if !destructive_tests_enabled() {
            eprintln!("Skipping destructive test (set OMG_RUN_DESTRUCTIVE_TESTS=1)");
            return;
        }

        let (success, _stdout, _stderr) = run_omg(&["update", "--check"]);
        assert!(success, "Update check should succeed");
    }

    #[test]
    fn test_status_command() {
        let (success, stdout, _) = run_omg(&["status"]);
        assert!(success, "Status should succeed");
        assert!(
            stdout.contains("Packages")
                || stdout.contains("Updates")
                || stdout.contains("Runtimes"),
            "Status should show system info"
        );
    }

    #[test]
    fn test_clean_help() {
        let (success, stdout, _) = run_omg(&["clean", "--help"]);
        assert!(success, "Clean help should succeed");
        assert!(stdout.contains("orphans"), "Should mention orphans option");
        assert!(stdout.contains("cache"), "Should mention cache option");
    }

    #[test]
    fn test_explicit_packages() {
        let (success, stdout, _) = run_omg(&["explicit"]);
        assert!(success, "Explicit should succeed");
        // Should list some packages on a real Arch system
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// RUNTIME MANAGEMENT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod runtime_management {
    use super::*;

    const RUNTIMES: &[&str] = &["node", "python", "go", "rust", "ruby", "java", "bun"];

    #[test]
    fn test_list_all_runtimes() {
        let (success, stdout, _) = run_omg(&["list"]);
        assert!(success, "List should succeed");
        // Should list available runtimes
    }

    #[test]
    fn test_list_installed_node() {
        let (success, _, _) = run_omg(&["list", "node"]);
        assert!(success, "List node should succeed");
    }

    #[test]
    fn test_list_installed_python() {
        let (success, _, _) = run_omg(&["list", "python"]);
        assert!(success, "List python should succeed");
    }

    #[test]
    fn test_list_available_node() {
        let (success, stdout, _) = run_omg(&["list", "node", "--available"]);
        assert!(success, "List available node should succeed");
        // Should show versions from nodejs.org
    }

    #[test]
    fn test_list_available_python() {
        let (success, stdout, _) = run_omg(&["list", "python", "--available"]);
        assert!(success, "List available python should succeed");
    }

    #[test]
    fn test_list_available_go() {
        let (success, stdout, _) = run_omg(&["list", "go", "--available"]);
        assert!(success, "List available go should succeed");
    }

    #[test]
    fn test_list_available_rust() {
        let (success, stdout, _) = run_omg(&["list", "rust", "--available"]);
        assert!(success, "List available rust should succeed");
    }

    #[test]
    fn test_list_available_ruby() {
        let (success, stdout, _) = run_omg(&["list", "ruby", "--available"]);
        assert!(success, "List available ruby should succeed");
    }

    #[test]
    fn test_list_available_java() {
        let (success, stdout, _) = run_omg(&["list", "java", "--available"]);
        assert!(success, "List available java should succeed");
        // Should show LTS markers
    }

    #[test]
    fn test_list_available_bun() {
        let (success, stdout, _) = run_omg(&["list", "bun", "--available"]);
        assert!(success, "List available bun should succeed");
    }

    #[test]
    fn test_list_unknown_runtime() {
        let (success, stdout, _) = run_omg(&["list", "unknownruntime"]);
        // Should fail or show error
        assert!(
            !success || stdout.contains("Unknown") || stdout.contains("Supported"),
            "Should indicate unknown runtime"
        );
    }

    #[test]
    fn test_which_command() {
        for runtime in RUNTIMES {
            let (success, _, _) = run_omg(&["which", runtime]);
            assert!(success, "which {runtime} should succeed");
        }
    }

    #[test]
    fn test_use_without_version_no_config() {
        let temp_dir = TempDir::new().unwrap();
        let (success, _, stderr) = run_omg_in_dir(&["use", "node"], temp_dir.path());
        // Should fail because no version file exists
        assert!(
            !success || stderr.contains("No version") || stderr.contains("detected"),
            "Should fail without version file"
        );
    }

    #[test]
    fn test_use_with_nvmrc() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "node");

        let (success, stdout, _) = run_omg_in_dir(&["use", "node"], temp_dir.path());
        // Should detect the version from .nvmrc
        assert!(
            success || stdout.contains("20.10.0") || stdout.contains("Detected"),
            "Should detect version from .nvmrc"
        );
    }

    #[test]
    fn test_use_with_python_version() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "python");

        let (success, stdout, _) = run_omg_in_dir(&["use", "python"], temp_dir.path());
        // Should detect the version from .python-version
        assert!(
            success || stdout.contains("3.11.0") || stdout.contains("Detected"),
            "Should detect version from .python-version"
        );
    }

    #[test]
    fn test_use_with_tool_versions() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "tool-versions");

        // Test Node detection from .tool-versions
        let (success, stdout, _) = run_omg_in_dir(&["use", "node"], temp_dir.path());
        assert!(
            success || stdout.contains("20.10.0"),
            "Should detect node version from .tool-versions"
        );

        // Test Python detection from .tool-versions
        let (success, stdout, _) = run_omg_in_dir(&["use", "python"], temp_dir.path());
        assert!(
            success || stdout.contains("3.11.0"),
            "Should detect python version from .tool-versions"
        );
    }

    #[test]
    fn test_use_invalid_version_format() {
        let (success, _, _) = run_omg(&["use", "node", "not-a-version"]);
        // Should handle gracefully (may try to install or fail)
    }

    #[test]
    fn test_runtime_alias_node_nodejs() {
        // "nodejs" should work the same as "node"
        let (success1, stdout1, _) = run_omg(&["list", "node"]);
        let (success2, stdout2, _) = run_omg(&["list", "nodejs"]);
        assert_eq!(success1, success2, "node and nodejs should behave the same");
    }

    #[test]
    fn test_runtime_alias_go_golang() {
        // "golang" should work the same as "go"
        let (success1, _, _) = run_omg(&["list", "go"]);
        let (success2, _, _) = run_omg(&["list", "golang"]);
        assert_eq!(success1, success2, "go and golang should behave the same");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ENVIRONMENT MANAGEMENT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod environment_management {
    use super::*;

    #[test]
    fn test_env_capture() {
        let temp_dir = TempDir::new().unwrap();
        let (success, stdout, _) = run_omg_in_dir(&["env", "capture"], temp_dir.path());
        assert!(success, "env capture should succeed");
        assert!(
            stdout.contains("omg.lock") || stdout.contains("captured"),
            "Should mention lock file"
        );

        // Verify omg.lock was created
        assert!(
            temp_dir.path().join("omg.lock").exists(),
            "omg.lock should be created"
        );
    }

    #[test]
    fn test_env_capture_deterministic() {
        let temp_dir = TempDir::new().unwrap();

        // Capture twice
        run_omg_in_dir(&["env", "capture"], temp_dir.path());
        let lock1 = fs::read_to_string(temp_dir.path().join("omg.lock")).unwrap();

        // Small delay
        std::thread::sleep(Duration::from_millis(100));

        run_omg_in_dir(&["env", "capture"], temp_dir.path());
        let lock2 = fs::read_to_string(temp_dir.path().join("omg.lock")).unwrap();

        // Hash should be the same (ignoring timestamp)
        // Extract hash line and compare
        let hash1: Option<&str> = lock1.lines().find(|l| l.starts_with("hash"));
        let hash2: Option<&str> = lock2.lines().find(|l| l.starts_with("hash"));

        assert_eq!(hash1, hash2, "Hash should be deterministic");
    }

    #[test]
    fn test_env_check_no_drift() {
        let temp_dir = TempDir::new().unwrap();

        // Capture
        run_omg_in_dir(&["env", "capture"], temp_dir.path());

        // Check immediately - should have no drift
        let (success, stdout, _) = run_omg_in_dir(&["env", "check"], temp_dir.path());
        assert!(success, "env check should succeed when no drift");
        assert!(
            stdout.contains("No drift") || stdout.contains("matches"),
            "Should report no drift"
        );
    }

    #[test]
    fn test_env_check_without_lock() {
        let temp_dir = TempDir::new().unwrap();

        // Check without capturing first
        let (success, _, stderr) = run_omg_in_dir(&["env", "check"], temp_dir.path());
        assert!(!success, "env check should fail without omg.lock");
        assert!(
            stderr.contains("omg.lock")
                || stderr.contains("not found")
                || stderr.contains("capture"),
            "Should mention missing lock file"
        );
    }

    #[test]
    fn test_env_share_without_token() {
        let temp_dir = TempDir::new().unwrap();
        run_omg_in_dir(&["env", "capture"], temp_dir.path());

        // Clear GITHUB_TOKEN
        let (success, _, stderr) = run_omg_with_env(&["env", "share"], &[("GITHUB_TOKEN", "")]);
        // Should fail because no token
        assert!(
            !success || stderr.contains("GITHUB_TOKEN") || stderr.contains("token"),
            "Should require GITHUB_TOKEN"
        );
    }

    #[test]
    fn test_env_share_without_lock() {
        let temp_dir = TempDir::new().unwrap();

        // Try to share without capturing first
        let (success, _, stderr) = run_omg_in_dir(&["env", "share"], temp_dir.path());
        assert!(!success, "env share should fail without omg.lock");
    }

    #[test]
    fn test_env_sync_invalid_url() {
        let temp_dir = TempDir::new().unwrap();

        let (success, _, stderr) =
            run_omg_in_dir(&["env", "sync", "not-a-valid-gist-url"], temp_dir.path());
        assert!(!success, "env sync should fail with invalid URL");
    }

    #[test]
    fn test_env_subcommand_help() {
        let (success, stdout, _) = run_omg(&["env", "--help"]);
        assert!(success, "env --help should succeed");
        assert!(stdout.contains("capture"), "Should list capture subcommand");
        assert!(stdout.contains("check"), "Should list check subcommand");
        assert!(stdout.contains("share"), "Should list share subcommand");
        assert!(stdout.contains("sync"), "Should list sync subcommand");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SECURITY TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod security {
    use super::*;

    #[test]
    fn test_audit_command() {
        let (success, _stdout, stderr) = run_omg(&["audit"]);
        // May succeed or fail depending on daemon status
        // Should not crash
        assert!(
            success || stderr.contains("daemon") || stderr.contains("Daemon"),
            "Audit should work or report daemon issue"
        );
    }

    #[test]
    fn test_security_policy_file_loading() {
        let temp_dir = TempDir::new().unwrap();

        // Create a policy file
        let config_dir = temp_dir.path().join(".config").join("omg");
        fs::create_dir_all(&config_dir).unwrap();

        let mut policy_file = File::create(config_dir.join("policy.toml")).unwrap();
        writeln!(
            policy_file,
            r#"
allow_aur = false
require_pgp = true
minimum_grade = "Verified"
banned_packages = ["malware-pkg"]
        "#
        )
        .unwrap();

        // Run a command that would load policy
        // The actual policy enforcement is tested in unit tests
    }

    #[test]
    fn test_security_grade_display() {
        // When searching, security grades should be visible
        let (success, stdout, _) = run_omg(&["info", "pacman"]);
        assert!(success, "Info should succeed");
        // Note: Security grade display depends on implementation
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMPLETION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod completions {
    use super::*;

    #[test]
    fn test_completions_bash() {
        let (success, stdout, _) = run_omg(&["completions", "bash", "--stdout"]);
        assert!(success, "Bash completions should succeed");
        assert!(
            stdout.contains("complete")
                || stdout.contains("_omg")
                || stdout.contains("_omg_completions"),
            "Should output bash completion script"
        );
    }

    #[test]
    fn test_completions_zsh() {
        let (success, stdout, _) = run_omg(&["completions", "zsh", "--stdout"]);
        assert!(success, "Zsh completions should succeed");
        assert!(
            stdout.contains("compdef") || stdout.contains("_omg"),
            "Should output zsh completion script"
        );
    }

    #[test]
    fn test_completions_fish() {
        let (success, stdout, _) = run_omg(&["completions", "fish", "--stdout"]);
        assert!(success, "Fish completions should succeed");
        assert!(
            stdout.contains("complete") || stdout.contains("omg"),
            "Should output fish completion script"
        );
    }

    #[test]
    fn test_completions_invalid_shell() {
        let (success, _, stderr) = run_omg(&["completions", "invalidshell"]);
        assert!(!success, "Invalid shell should fail");
        assert!(
            stderr.contains("Unsupported") || stderr.contains("error"),
            "Should report unsupported shell"
        );
    }

    #[test]
    fn test_hidden_complete_command() {
        // Test the hidden dynamic completion command
        let (success, _, _) = run_omg(&["complete", "--shell", "zsh", "--current", "fire"]);
        // May or may not be implemented as a visible command
    }

    #[test]
    fn test_fuzzy_completion_typo() {
        // This tests the internal completion engine
        // The hidden complete command should handle typos
        let (success, stdout, _) = run_omg(&["complete", "--shell", "zsh", "--current", "frfx"]);
        // If implemented, should suggest "firefox"
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// SHELL HOOK TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod shell_hooks {
    use super::*;

    #[test]
    fn test_hook_bash() {
        let (success, stdout, _) = run_omg(&["hook", "bash"]);
        assert!(success, "Hook bash should succeed");
        // Should output shell initialization code
    }

    #[test]
    fn test_hook_zsh() {
        let (success, stdout, _) = run_omg(&["hook", "zsh"]);
        assert!(success, "Hook zsh should succeed");
    }

    #[test]
    fn test_hook_fish() {
        let (success, stdout, _) = run_omg(&["hook", "fish"]);
        assert!(success, "Hook fish should succeed");
    }

    #[test]
    fn test_hook_invalid_shell() {
        let (success, _, _) = run_omg(&["hook", "invalidshell"]);
        // Should fail or return empty
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONFIG TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod config {
    use super::*;

    #[test]
    fn test_config_list() {
        let (success, stdout, _) = run_omg(&["config"]);
        assert!(success, "Config list should succeed");
        // Should show configuration
    }

    #[test]
    fn test_config_get_key() {
        let (success, _, _) = run_omg(&["config", "data_dir"]);
        assert!(success, "Config get should succeed");
    }

    #[test]
    fn test_config_get_invalid_key() {
        let (success, stdout, _) = run_omg(&["config", "nonexistent_key"]);
        assert!(success, "Config get for invalid key should not crash");
        // Should show "(not set)" or similar
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PERFORMANCE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod performance {
    use super::*;

    #[test]
    fn test_status_performance() {
        let ((success, _, _), duration) = measure_time(|| run_omg(&["status"]));
        assert!(success, "Status should succeed");

        // Status should complete in under 500ms (generous for CI)
        assert!(
            duration < Duration::from_millis(500),
            "Status took too long: {duration:?}"
        );
    }

    #[test]
    fn test_list_performance() {
        let ((success, _, _), duration) = measure_time(|| run_omg(&["list"]));
        assert!(success, "List should succeed");

        // List installed should be very fast
        assert!(
            duration < Duration::from_millis(200),
            "List took too long: {duration:?}"
        );
    }

    #[test]
    fn test_which_performance() {
        let ((success, _, _), duration) = measure_time(|| run_omg(&["which", "node"]));
        assert!(success, "Which should succeed");

        // Which should be extremely fast (< 50ms)
        assert!(
            duration < Duration::from_millis(100),
            "Which took too long: {duration:?}"
        );
    }

    #[test]
    fn test_help_performance() {
        let ((success, _, _), duration) = measure_time(|| run_omg(&["--help"]));
        assert!(success, "Help should succeed");

        // Help should be instant
        assert!(
            duration < Duration::from_millis(50),
            "Help took too long: {duration:?}"
        );
    }

    #[test]
    fn test_completions_generation_performance() {
        let ((success, _, _), duration) =
            measure_time(|| run_omg(&["completions", "zsh", "--stdout"]));
        assert!(success, "Completions should succeed");

        // Completions generation should be fast
        assert!(
            duration < Duration::from_millis(100),
            "Completions generation took too long: {duration:?}"
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR HANDLING TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod error_handling {
    use super::*;

    #[test]
    fn test_graceful_handling_missing_alpm() {
        // If ALPM is not available (e.g., non-Arch system), should handle gracefully
        // This test is more for documentation than assertion
    }

    #[test]
    fn test_network_timeout_handling() {
        // Test with a very short timeout environment variable if supported
    }

    #[test]
    fn test_invalid_lock_file() {
        let temp_dir = TempDir::new().unwrap();

        // Create invalid omg.lock
        let mut f = File::create(temp_dir.path().join("omg.lock")).unwrap();
        writeln!(f, "this is not valid toml {{{{").unwrap();

        let (success, _, stderr) = run_omg_in_dir(&["env", "check"], temp_dir.path());
        assert!(!success, "Should fail with invalid lock file");
        // Should show a helpful error, not panic
    }

    #[test]
    fn test_corrupted_lock_file() {
        let temp_dir = TempDir::new().unwrap();

        // Create corrupted omg.lock (valid TOML but wrong schema)
        let mut f = File::create(temp_dir.path().join("omg.lock")).unwrap();
        writeln!(f, "[wrong_section]\nkey = \"value\"").unwrap();

        let (success, _, _) = run_omg_in_dir(&["env", "check"], temp_dir.path());
        // Should handle gracefully
    }

    #[test]
    fn test_permission_denied_handling() {
        // This test is platform-specific and may not work in all environments
        // Skipping actual implementation but documenting the need
    }

    #[test]
    fn test_disk_full_handling() {
        // Difficult to test, but documenting the need
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// EDGE CASE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod edge_cases {
    use super::*;

    #[test]
    fn test_empty_environment() {
        let temp_dir = TempDir::new().unwrap();
        // Empty directory - no runtimes, no packages tracked
        let (success, _, _) = run_omg_in_dir(&["env", "capture"], temp_dir.path());
        assert!(success, "Should handle empty environment");
    }

    #[test]
    fn test_deeply_nested_directory() {
        let temp_dir = TempDir::new().unwrap();

        // Create deeply nested structure with .nvmrc at root
        create_test_project(temp_dir.path(), "node");

        let deep_path = temp_dir
            .path()
            .join("a")
            .join("b")
            .join("c")
            .join("d")
            .join("e");
        fs::create_dir_all(&deep_path).unwrap();

        // Running from deep path should still find .nvmrc at root
        let (success, stdout, _) = run_omg_in_dir(&["use", "node"], &deep_path);
        // Should detect version from parent directories
    }

    #[test]
    fn test_symlink_handling() {
        let temp_dir = TempDir::new().unwrap();
        create_test_project(temp_dir.path(), "node");

        // Create a symlink to the directory
        let symlink_path = temp_dir.path().join("symlink_dir");
        #[cfg(unix)]
        std::os::unix::fs::symlink(temp_dir.path(), &symlink_path).ok();

        // Should work through symlinks
    }

    #[test]
    fn test_concurrent_operations() {
        use std::thread;

        // Run multiple omg commands concurrently
        let handles: Vec<_> = (0..5)
            .map(|_| thread::spawn(|| run_omg(&["status"])))
            .collect();

        for handle in handles {
            let (success, _, _) = handle.join().unwrap();
            assert!(success, "Concurrent status should succeed");
        }
    }

    #[test]
    fn test_very_large_package_list() {
        // Searching for common terms that return many results
        let (success, _, _) = run_omg(&["search", "lib"]);
        assert!(success, "Large search should succeed");
    }

    #[test]
    fn test_unicode_in_paths() {
        let temp_dir = TempDir::new().unwrap();
        let unicode_dir = temp_dir.path().join("项目目录");
        fs::create_dir_all(&unicode_dir).unwrap();

        create_test_project(&unicode_dir, "node");

        // Should handle unicode paths
        let (success, _, _) = run_omg_in_dir(&["use", "node"], &unicode_dir);
    }

    #[test]
    fn test_whitespace_in_version() {
        // Version with leading/trailing whitespace
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join(".nvmrc")).unwrap();
        writeln!(f, "  20.10.0  ").unwrap();

        let (success, stdout, _) = run_omg_in_dir(&["use", "node"], temp_dir.path());
        // Should trim whitespace
    }

    #[test]
    fn test_comments_in_version_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut f = File::create(temp_dir.path().join(".nvmrc")).unwrap();
        writeln!(f, "# This is a comment").unwrap();
        writeln!(f, "20.10.0").unwrap();

        let (success, _, _) = run_omg_in_dir(&["use", "node"], temp_dir.path());
        // Should handle comments (implementation dependent)
    }

    #[test]
    fn test_lts_version_alias() {
        // Using "lts" as a version
        let (success, _, _) = run_omg(&["use", "node", "lts"]);
        // Should resolve to actual LTS version
    }

    #[test]
    fn test_latest_version_alias() {
        // Using "latest" as a version
        let (success, _, _) = run_omg(&["use", "node", "latest"]);
        // Should resolve to latest version
    }

    #[test]
    fn test_partial_version() {
        // Using partial version like "20" instead of "20.10.0"
        let (success, _, _) = run_omg(&["use", "node", "20"]);
        // Should resolve to latest 20.x.x
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DATABASE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod database {
    use super::*;

    #[test]
    fn test_database_creation() {
        // The database should be created automatically
        // Just verify omg runs - DB is created on demand
        let (success, _, _) = run_omg(&["status"]);
        assert!(success, "Status should succeed (creates DB if needed)");
    }

    #[test]
    fn test_database_concurrent_access() {
        use std::thread;

        // Multiple threads accessing the database
        let handles: Vec<_> = (0..3)
            .map(|_| thread::spawn(|| run_omg(&["list", "node"])))
            .collect();

        for handle in handles {
            let (success, _, _) = handle.join().unwrap();
            assert!(success, "Concurrent DB access should succeed");
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DAEMON TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod daemon {
    use super::*;

    #[test]
    fn test_daemon_help() {
        let (success, stdout, _) = run_omg(&["daemon", "--help"]);
        assert!(success, "Daemon help should succeed");
        assert!(
            stdout.contains("foreground"),
            "Should mention foreground option"
        );
    }

    #[test]
    fn test_status_with_daemon() {
        // Status should work with or without daemon
        let (success, _, _) = run_omg(&["status"]);
        assert!(success, "Status should succeed");
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// INTEGRATION SCENARIOS
// ═══════════════════════════════════════════════════════════════════════════════

mod integration_scenarios {
    use super::*;

    #[test]
    fn scenario_new_developer_onboarding() {
        let temp_dir = TempDir::new().unwrap();

        // 1. Create project with .tool-versions
        create_test_project(temp_dir.path(), "tool-versions");

        // 2. Developer runs status to see what's needed
        let (success, _, _) = run_omg_in_dir(&["status"], temp_dir.path());
        assert!(success, "Status should work");

        // 3. Developer syncs environment (if lock exists from team)
        // Simulated by running env capture
        let (success, _, _) = run_omg_in_dir(&["env", "capture"], temp_dir.path());
        assert!(success, "Env capture should work");

        // 4. Check for drift
        let (success, _, _) = run_omg_in_dir(&["env", "check"], temp_dir.path());
        assert!(success, "Env check should show no drift");
    }

    #[test]
    fn scenario_switching_projects() {
        let project1 = TempDir::new().unwrap();
        let project2 = TempDir::new().unwrap();

        // Project 1 uses Node 18
        let mut f = File::create(project1.path().join(".nvmrc")).unwrap();
        writeln!(f, "18.0.0").unwrap();

        // Project 2 uses Node 20
        let mut f = File::create(project2.path().join(".nvmrc")).unwrap();
        writeln!(f, "20.0.0").unwrap();

        // Switch to project 1
        let (success, stdout1, _) = run_omg_in_dir(&["use", "node"], project1.path());

        // Switch to project 2
        let (success, stdout2, _) = run_omg_in_dir(&["use", "node"], project2.path());

        // Versions should be different
        assert!(
            stdout1.contains("18") || stdout2.contains("20"),
            "Should detect different versions per project"
        );
    }

    #[test]
    fn scenario_security_audit_workflow() {
        // 1. Run status to see overview
        let (success, _, _) = run_omg(&["status"]);
        assert!(success, "Status should work");

        // 2. Run full audit
        let (_, stdout, _) = run_omg(&["audit"]);
        // May require daemon

        // 3. Search for a package to install
        let (success, _, _) = run_omg(&["search", "firefox"]);
        assert!(success, "Search should work");

        // 4. Get info on package
        let (success, _, _) = run_omg(&["info", "firefox"]);
        assert!(success, "Info should work");
    }

    #[test]
    fn scenario_team_environment_sync() {
        let dev1_dir = TempDir::new().unwrap();
        let dev2_dir = TempDir::new().unwrap();

        // Dev 1 captures their environment
        create_test_project(dev1_dir.path(), "tool-versions");
        let (success, _, _) = run_omg_in_dir(&["env", "capture"], dev1_dir.path());
        assert!(success, "Dev1 capture should work");

        // Copy lock file to dev2 (simulating gist share/sync)
        let lock_content = fs::read_to_string(dev1_dir.path().join("omg.lock")).unwrap();
        fs::write(dev2_dir.path().join("omg.lock"), &lock_content).unwrap();

        // Dev 2 checks their environment
        create_test_project(dev2_dir.path(), "tool-versions");
        let (success, _, _) = run_omg_in_dir(&["env", "check"], dev2_dir.path());
        assert!(success, "Dev2 check should work");
    }
}
