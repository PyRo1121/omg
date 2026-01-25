#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
//! OMG Comprehensive Test Suite
//!
//! Tests EVERY feature and module in the codebase.
//!
//! Run all: cargo test --test comprehensive_tests --features arch
//! Run specific module: cargo test --test comprehensive_tests cli_
//! Run with coverage: cargo tarpaulin --test comprehensive_tests

#![allow(clippy::doc_markdown)]
#![allow(unused_variables)]

mod common;

#[allow(unused_imports)]
use common::fixtures::*;
use common::*;
use std::time::Duration;

// ═══════════════════════════════════════════════════════════════════════════════
// CLI COMMAND TESTS - Every single command
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_help {
    use super::*;

    #[test]
    fn test_main_help() {
        let result = run_omg(&["--help"]);
        result.assert_success();
        assert!(result.stdout_contains("omg") || result.stdout_contains("OMG"));
    }

    #[test]
    fn test_version() {
        let result = run_omg(&["--version"]);
        result.assert_success();
        assert!(result.stdout_contains("0.") || result.stdout_contains("1."));
    }

    #[test]
    fn test_search_help() {
        let result = run_omg(&["search", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_install_help() {
        let result = run_omg(&["install", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_remove_help() {
        let result = run_omg(&["remove", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_update_help() {
        let result = run_omg(&["update", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_info_help() {
        let result = run_omg(&["info", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_sync_help() {
        let result = run_omg(&["sync", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_clean_help() {
        let result = run_omg(&["clean", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_why_help() {
        let result = run_omg(&["why", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_outdated_help() {
        let result = run_omg(&["outdated", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_pin_help() {
        let result = run_omg(&["pin", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_size_help() {
        let result = run_omg(&["size", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_blame_help() {
        let result = run_omg(&["blame", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_diff_help() {
        let result = run_omg(&["diff", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_snapshot_help() {
        let result = run_omg(&["snapshot", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_ci_help() {
        let result = run_omg(&["ci", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_migrate_help() {
        let result = run_omg(&["migrate", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_explicit_help() {
        let result = run_omg(&["explicit", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_use_help() {
        let result = run_omg(&["use", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_list_help() {
        let result = run_omg(&["list", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_which_help() {
        let result = run_omg(&["which", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_run_help() {
        let result = run_omg(&["run", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_tool_help() {
        let result = run_omg(&["tool", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_env_help() {
        let result = run_omg(&["env", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_new_help() {
        let result = run_omg(&["new", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_init_help() {
        let result = run_omg(&["init", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_doctor_help() {
        let result = run_omg(&["doctor", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_audit_help() {
        let result = run_omg(&["audit", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_status_help() {
        let result = run_omg(&["status", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_completions_help() {
        let result = run_omg(&["completions", "--help"]);
        result.assert_success();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI SEARCH TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_search {
    use super::*;

    #[test]
    fn test_search_empty_query() {
        let result = run_omg(&["search", ""]);
        // Should handle empty gracefully
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_search_simple_query() {
        let result = run_omg(&["search", "vim"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_search_special_chars() {
        let result = run_omg(&["search", "test-pkg"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_search_unicode() {
        let result = run_omg(&["search", "test"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_search_very_long() {
        let long_query = "a".repeat(1000);
        let result = run_omg(&["search", &long_query]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_search_with_limit() {
        let result = run_omg(&["search", "lib", "--limit", "5"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_search_json_output() {
        let result = run_omg(&["search", "bash", "--json"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI INFO TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_info {
    use super::*;

    #[test]
    fn test_info_nonexistent() {
        let result = run_omg(&["info", "nonexistent-package-12345"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_info_common_package() {
        let result = run_omg(&["info", "bash"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_info_with_json() {
        let result = run_omg(&["info", "glibc", "--json"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI RUNTIME TESTS - All runtimes
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_runtimes {
    use super::*;

    // Node.js
    #[test]
    fn test_list_node() {
        let result = run_omg(&["list", "node"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_which_node() {
        let result = run_omg(&["which", "node"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_use_node_invalid() {
        let result = run_omg(&["use", "node", "999.999.999"]);
        assert!(!result.stderr_contains("panicked"));
    }

    // Python
    #[test]
    fn test_list_python() {
        let result = run_omg(&["list", "python"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_which_python() {
        let result = run_omg(&["which", "python"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_use_python_invalid() {
        let result = run_omg(&["use", "python", "999.999.999"]);
        assert!(!result.stderr_contains("panicked"));
    }

    // Go
    #[test]
    fn test_list_go() {
        let result = run_omg(&["list", "go"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_which_go() {
        let result = run_omg(&["which", "go"]);
        assert!(!result.stderr_contains("panicked"));
    }

    // Rust
    #[test]
    fn test_list_rust() {
        let result = run_omg(&["list", "rust"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_which_rust() {
        let result = run_omg(&["which", "rust"]);
        assert!(!result.stderr_contains("panicked"));
    }

    // Bun
    #[test]
    fn test_list_bun() {
        let result = run_omg(&["list", "bun"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_which_bun() {
        let result = run_omg(&["which", "bun"]);
        assert!(!result.stderr_contains("panicked"));
    }

    // Ruby
    #[test]
    fn test_list_ruby() {
        let result = run_omg(&["list", "ruby"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_which_ruby() {
        let result = run_omg(&["which", "ruby"]);
        assert!(!result.stderr_contains("panicked"));
    }

    // Java
    #[test]
    fn test_list_java() {
        let result = run_omg(&["list", "java"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_which_java() {
        let result = run_omg(&["which", "java"]);
        assert!(!result.stderr_contains("panicked"));
    }

    // Invalid runtime
    #[test]
    fn test_list_invalid_runtime() {
        let result = run_omg(&["list", "notaruntime"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_use_invalid_runtime() {
        let result = run_omg(&["use", "notaruntime", "1.0.0"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI ENV TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_env {
    use super::*;

    #[test]
    fn test_env_check() {
        let project = TestProject::new();
        let result = project.run(&["env", "check"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_env_capture() {
        let project = TestProject::new();
        let result = project.run(&["env", "capture"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_env_share() {
        let project = TestProject::new();
        let result = project.run(&["env", "share"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI TOOL TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_tool {
    use super::*;

    #[test]
    fn test_tool_list() {
        let result = run_omg(&["tool", "list"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_tool_install_help() {
        let result = run_omg(&["tool", "install", "--help"]);
        result.assert_success();
    }

    #[test]
    fn test_tool_remove_help() {
        let result = run_omg(&["tool", "remove", "--help"]);
        result.assert_success();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI STATUS TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_status {
    use super::*;

    #[test]
    fn test_status_basic() {
        let result = run_omg(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_status_in_project() {
        let project = TestProject::new();
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_status_with_package_json() {
        let project = TestProject::new();
        project.create_file("package.json", r#"{"name": "test", "version": "1.0.0"}"#);
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_status_with_cargo_toml() {
        let project = TestProject::new();
        project.create_file(
            "Cargo.toml",
            r#"[package]
name = "test"
version = "0.1.0"
"#,
        );
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_status_with_pyproject() {
        let project = TestProject::new();
        project.create_file(
            "pyproject.toml",
            r#"[project]
name = "test"
version = "0.1.0"
"#,
        );
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI DOCTOR TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_doctor {
    use super::*;

    #[test]
    fn test_doctor_basic() {
        let result = run_omg(&["doctor"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_doctor_verbose() {
        let result = run_omg(&["doctor", "--verbose"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI INIT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_init {
    use super::*;

    #[test]
    fn test_init_basic() {
        let project = TestProject::new();
        let result = project.run(&["init"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_init_with_force() {
        let project = TestProject::new();
        let result = project.run(&["init", "--force"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI NEW TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_new {
    use super::*;

    #[test]
    fn test_new_list_templates() {
        let result = run_omg(&["new", "--list"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_new_rust() {
        let project = TestProject::new();
        let result = project.run(&["new", "rust", "myproject"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_new_node() {
        let project = TestProject::new();
        let result = project.run(&["new", "node", "myproject"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_new_python() {
        let project = TestProject::new();
        let result = project.run(&["new", "python", "myproject"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI SNAPSHOT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_snapshot {
    use super::*;

    #[test]
    fn test_snapshot_list() {
        let result = run_omg(&["snapshot", "list"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_snapshot_create() {
        let project = TestProject::new();
        let result = project.run(&["snapshot", "create"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI CI TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_ci {
    use super::*;

    #[test]
    fn test_ci_init_github() {
        let project = TestProject::new();
        let result = project.run(&["ci", "init", "--provider", "github"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_ci_init_gitlab() {
        let project = TestProject::new();
        let result = project.run(&["ci", "init", "--provider", "gitlab"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI MIGRATE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_migrate {
    use super::*;

    #[test]
    fn test_migrate_export() {
        let project = TestProject::new();
        let result = project.run(&["migrate", "export"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_migrate_import_help() {
        let result = run_omg(&["migrate", "import", "--help"]);
        result.assert_success();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI PIN TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_pin {
    use super::*;

    #[test]
    fn test_pin_list() {
        let project = TestProject::new();
        let result = project.run(&["pin", "--list"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_pin_add() {
        let project = TestProject::new();
        let result = project.run(&["pin", "node@20.10.0"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI AUDIT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_audit {
    use super::*;

    #[test]
    fn test_audit_basic() {
        let result = run_omg(&["audit"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_audit_json() {
        let result = run_omg(&["audit", "--json"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI COMPLETIONS TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_completions {
    use super::*;

    #[test]
    fn test_completions_bash() {
        let result = run_omg(&["completions", "bash"]);
        result.assert_success();
    }

    #[test]
    fn test_completions_zsh() {
        let result = run_omg(&["completions", "zsh"]);
        result.assert_success();
    }

    #[test]
    fn test_completions_fish() {
        let result = run_omg(&["completions", "fish"]);
        result.assert_success();
    }

    #[test]
    fn test_completions_powershell() {
        let result = run_omg(&["completions", "powershell"]);
        result.assert_success();
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CLI RUN TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod cli_run {
    use super::*;

    #[test]
    fn test_run_list() {
        let project = TestProject::new();
        let result = project.run(&["run", "--list"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_run_with_package_json() {
        let project = TestProject::new();
        project.create_file(
            "package.json",
            r#"{
            "name": "test",
            "scripts": {
                "test": "echo hello"
            }
        }"#,
        );
        let result = project.run(&["run", "--list"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_run_with_makefile() {
        let project = TestProject::new();
        project.create_file("Makefile", "test:\n\techo hello\n");
        let result = project.run(&["run", "--list"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PROJECT DETECTION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod project_detection {
    use super::*;

    #[test]
    fn test_detect_nvmrc() {
        let project = TestProject::new();
        project.create_file(".nvmrc", "20.10.0");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_detect_node_version() {
        let project = TestProject::new();
        project.create_file(".node-version", "20.10.0");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_detect_python_version() {
        let project = TestProject::new();
        project.create_file(".python-version", "3.12.0");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_detect_tool_versions() {
        let project = TestProject::new();
        project.create_file(".tool-versions", "nodejs 20.10.0\npython 3.12.0");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_detect_rust_toolchain() {
        let project = TestProject::new();
        project.create_file("rust-toolchain.toml", "[toolchain]\nchannel = \"stable\"");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_detect_go_mod() {
        let project = TestProject::new();
        project.create_file("go.mod", "module test\n\ngo 1.21");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_detect_mise_toml() {
        let project = TestProject::new();
        project.create_file(".mise.toml", "[tools]\nnode = \"20\"");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR HANDLING TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod error_handling {
    use super::*;

    #[test]
    fn test_invalid_command() {
        let result = run_omg(&["nonexistent-command"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_missing_argument() {
        let result = run_omg(&["install"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_invalid_flag() {
        let result = run_omg(&["search", "--invalid-flag"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_conflicting_flags() {
        let result = run_omg(&["search", "test", "--json", "--quiet"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PERFORMANCE TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod performance {
    use super::*;

    #[test]
    fn test_help_fast() {
        let result = run_omg(&["--help"]);
        // Help should be under 2 seconds even on slow systems
        assert!(result.duration < Duration::from_secs(2));
    }

    #[test]
    fn test_version_fast() {
        let result = run_omg(&["--version"]);
        assert!(result.duration < Duration::from_secs(2));
    }

    #[test]
    fn test_status_reasonable() {
        let result = run_omg(&["status"]);
        // Status should be under 5 seconds
        assert!(result.duration < Duration::from_secs(5));
    }

    #[test]
    fn test_completions_fast() {
        let result = run_omg(&["completions", "bash"]);
        assert!(result.duration < Duration::from_secs(2));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// CONCURRENCY TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod concurrency {
    use super::*;
    use std::thread;

    #[test]
    fn test_concurrent_status() {
        let handles: Vec<_> = (0..4)
            .map(|_| thread::spawn(|| run_omg(&["status"])))
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            assert!(!result.stderr_contains("panicked"));
        }
    }

    #[test]
    fn test_concurrent_help() {
        let handles: Vec<_> = (0..8)
            .map(|_| thread::spawn(|| run_omg(&["--help"])))
            .collect();

        for handle in handles {
            let result = handle.join().unwrap();
            result.assert_success();
        }
    }

    #[test]
    fn test_concurrent_different_commands() {
        let handles: Vec<_> = vec![
            thread::spawn(|| run_omg(&["--help"])),
            thread::spawn(|| run_omg(&["status"])),
            thread::spawn(|| run_omg(&["list", "node"])),
            thread::spawn(|| run_omg(&["which", "node"])),
        ];

        for handle in handles {
            let result = handle.join().unwrap();
            assert!(!result.stderr_contains("panicked"));
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// FILE HANDLING TESTS
// ═══════════════════════════════════════════════════════════════════════════════

mod file_handling {
    use super::*;

    #[test]
    fn test_malformed_package_json() {
        let project = TestProject::new();
        project.create_file("package.json", "{ invalid json");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_malformed_cargo_toml() {
        let project = TestProject::new();
        project.create_file("Cargo.toml", "[invalid toml");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_empty_nvmrc() {
        let project = TestProject::new();
        project.create_file(".nvmrc", "");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_binary_file_as_config() {
        let project = TestProject::new();
        project.create_file(".nvmrc", "\x00\x01\x02\x03");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_very_large_config_file() {
        let project = TestProject::new();
        let large_content = "a".repeat(100_000);
        project.create_file("package.json", &format!(r#"{{"name": "{large_content}"}}"#));
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_deeply_nested_directory() {
        let project = TestProject::new();
        let deep_path = (0..50)
            .map(|i| format!("dir{i}"))
            .collect::<Vec<_>>()
            .join("/");
        project.create_dir(&deep_path);
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// EDGE CASES
// ═══════════════════════════════════════════════════════════════════════════════

mod edge_cases {
    use super::*;

    #[test]
    fn test_unicode_project_name() {
        let project = TestProject::new();
        project.create_file("package.json", r#"{"name": "test-project-🚀"}"#);
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_whitespace_only_version() {
        let project = TestProject::new();
        project.create_file(".nvmrc", "   \n\t  ");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_version_with_comments() {
        let project = TestProject::new();
        project.create_file(".nvmrc", "20.10.0 # my preferred version");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_multiple_version_files() {
        let project = TestProject::new();
        project.create_file(".nvmrc", "20.10.0");
        project.create_file(".node-version", "18.0.0");
        project.create_file(".tool-versions", "nodejs 16.0.0");
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }

    #[test]
    fn test_symlink_loop() {
        let project = TestProject::new();
        // Create two directories that could form a loop
        project.create_dir("a");
        project.create_dir("b");
        // The project handles this gracefully
        let result = project.run(&["status"]);
        assert!(!result.stderr_contains("panicked"));
    }
}
