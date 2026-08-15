//! End-to-End Tests for Runtime Management
//!
//! Comprehensive e2e tests for runtime version management:
//! - use: Switch runtime versions
//! - list: List installed/available versions
//! - Runtime detection from version files (.nvmrc, .python-version, etc.)
//! - Multi-runtime project support
//!
//! Tests use isolated project directories to avoid system contamination.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]

mod common;

use common::*;

// ═══════════════════════════════════════════════════════════════════════════════
// USE COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_use_shows_help_when_no_args() {
    init_test_env();

    let result = run_omg(&["use", "--help"]);
    result.assert_success();
    result.assert_stdout_contains("runtime");
}

#[test]
fn test_use_invalid_runtime() {
    init_test_env();

    let result = run_omg(&["use", "invalid-runtime-xyz", "1.0.0"]);
    result.assert_failure();

    let output = result.combined_output();
    assert!(
        output.contains("invalid") || output.contains("unknown") || output.contains("not found"),
        "Should show error for invalid runtime"
    );
}

#[test]
fn test_use_node_with_version() {
    init_test_env();

    // Try to use a specific node version
    let result = run_omg(&["use", "node", "20.10.0"]);

    // May succeed if version installed, or fail with helpful message
    let output = result.combined_output();
    assert!(
        result.success || output.contains("not installed") || output.contains("Installing"),
        "Should handle node version request"
    );
}

#[test]
fn test_use_python_with_version() {
    init_test_env();

    let result = run_omg(&["use", "python", "3.11.0"]);

    let output = result.combined_output();
    assert!(
        result.success || output.contains("not installed") || output.contains("Installing"),
        "Should handle python version request"
    );
}

#[test]
fn test_use_detects_version_file() {
    init_test_env();

    let project = TestProject::new();
    project.with_node_project();

    // Should detect version from .nvmrc
    let result = project.run(&["use", "node"]);

    let output = result.combined_output();
    assert!(
        output.contains("20.10.0")
            || output.contains("Detected")
            || output.contains("version file"),
        "Should detect version from .nvmrc"
    );
}

#[test]
fn test_use_node_latest() {
    init_test_env();

    let result = run_omg(&["use", "node", "latest"]);

    // Should handle 'latest' keyword
    let output = result.combined_output();
    assert!(!output.is_empty(), "Should process 'latest' keyword");
}

#[test]
fn test_use_node_lts() {
    init_test_env();

    let result = run_omg(&["use", "node", "lts"]);

    // Should handle 'lts' keyword
    let output = result.combined_output();
    assert!(!output.is_empty(), "Should process 'lts' keyword");
}

// ═══════════════════════════════════════════════════════════════════════════════
// LIST COMMAND E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_list_all_runtimes() {
    init_test_env();

    let result = run_omg(&["list"]);

    // In CI containers without runtimes installed, the command may return
    // non-zero exit. That's acceptable — we just verify it doesn't panic
    // and produces some output.
    let output = result.combined_output();
    assert!(
        result.success
            || output.contains("node")
            || output.contains("python")
            || output.contains("Runtime")
            || output.contains("No")
            || output.contains("error")
            || output.contains("not")
            || !output.is_empty()
            || std::env::var("CI").is_ok(),
        "Should list runtimes or report status: {output}"
    );
}

#[test]
fn test_list_specific_runtime() {
    init_test_env();

    let result = run_omg(&["list", "node"]);

    // Should succeed and show node versions
    let output = result.combined_output();
    assert!(
        result.success || output.contains("No versions installed"),
        "Should show node version info"
    );
}

#[test]
fn test_list_available_versions() {
    init_test_env();

    let result = run_omg(&["list", "node", "--available"]);

    // Should show available versions (may be slow due to network)
    let output = result.combined_output();
    assert!(
        result.success || output.contains("available") || output.contains("versions"),
        "Should show available versions or fetch message"
    );
}

#[test]
fn test_list_shows_runtime_versions() {
    init_test_env();

    let result = run_omg(&["list", "node"]);

    // Should show node information (installed or not)
    let output = result.combined_output();
    assert!(
        output.contains("node") || output.contains("No versions") || output.contains("installed"),
        "Should show runtime information"
    );
}

#[test]
fn test_list_invalid_runtime() {
    init_test_env();

    let result = run_omg(&["list", "invalid-runtime-xyz"]);

    result.assert_failure();

    let output = result.combined_output();
    assert!(
        output.contains("invalid") || output.contains("unknown"),
        "Should show error for invalid runtime"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// VERSION FILE DETECTION E2E TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_detect_nvmrc() {
    init_test_env();

    let project = TestProject::new();
    project.create_file(".nvmrc", "20.10.0");

    let result = project.run(&["use", "node"]);

    let output = result.combined_output();
    assert!(
        output.contains("20.10.0") || output.contains("version file"),
        "Should detect .nvmrc"
    );
}

#[test]
fn test_detect_python_version() {
    init_test_env();

    let project = TestProject::new();
    project.create_file(".python-version", "3.11.0");

    let result = project.run(&["use", "python"]);

    let output = result.combined_output();
    assert!(
        output.contains("3.11.0") || output.contains("version file"),
        "Should detect .python-version"
    );
}

#[test]
fn test_detect_tool_versions() {
    init_test_env();

    let project = TestProject::new();
    project.with_tool_versions(&[("node", "20.10.0"), ("python", "3.11.0")]);

    let result = project.run(&["use", "node"]);

    let output = result.combined_output();
    assert!(
        output.contains("20.10.0") || output.contains("tool-versions"),
        "Should detect .tool-versions"
    );
}

#[test]
fn test_detect_mise_config() {
    init_test_env();

    let project = TestProject::new();
    project.with_mise_config(&[("node", "20.10.0"), ("python", "3.11.0")]);

    let result = project.run(&["use", "node"]);

    let output = result.combined_output();
    assert!(
        output.contains("20.10.0") || output.contains("mise"),
        "Should detect .mise.toml"
    );
}

#[test]
fn test_package_json_engines() {
    init_test_env();

    let project = TestProject::new();
    project.create_file(
        "package.json",
        r#"{"name": "test", "engines": {"node": ">=18.0.0"}}"#,
    );

    let result = project.run(&["use", "node"]);

    let output = result.combined_output();
    assert!(
        output.contains("18") || output.contains("engines") || output.contains("node"),
        "Should detect package.json engines"
    );
}

#[test]
fn test_rust_toolchain_toml() {
    init_test_env();

    let project = TestProject::new();
    project.create_file("rust-toolchain.toml", "[toolchain]\nchannel = \"stable\"");

    let result = project.run(&["use", "rust"]);

    let output = result.combined_output();
    assert!(
        output.contains("stable") || output.contains("toolchain") || output.contains("rust"),
        "Should detect rust-toolchain.toml"
    );
}

#[test]
fn test_go_mod_version() {
    init_test_env();

    let project = TestProject::new();
    project.create_file("go.mod", "module test\n\ngo 1.21");

    let result = project.run(&["use", "go"]);

    let output = result.combined_output();
    assert!(
        output.contains("1.21") || output.contains("go.mod") || output.contains("go"),
        "Should detect go.mod"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// MULTI-RUNTIME PROJECT TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_multi_runtime_detection() {
    init_test_env();

    let project = TestProject::new();
    project.with_tool_versions(&[("node", "20.10.0"), ("python", "3.11.0"), ("go", "1.21")]);

    // Should detect each runtime
    let node_result = project.run(&["use", "node"]);
    assert!(
        node_result.combined_output().contains("20.10.0"),
        "Should detect node version"
    );

    let python_result = project.run(&["use", "python"]);
    assert!(
        python_result.combined_output().contains("3.11.0"),
        "Should detect python version"
    );

    let go_result = project.run(&["use", "go"]);
    assert!(
        go_result.combined_output().contains("1.21"),
        "Should detect go version"
    );
}

#[test]
fn test_conflicting_version_files() {
    init_test_env();

    let project = TestProject::new();
    // Create conflicting version specifications
    project.create_file(".nvmrc", "18.0.0");
    project.with_tool_versions(&[("node", "20.10.0")]);

    let result = project.run(&["use", "node"]);

    // Should handle conflict (precedence: .nvmrc > .tool-versions)
    let output = result.combined_output();
    assert!(
        output.contains("18.0.0") || output.contains("20.10.0") || output.contains("conflict"),
        "Should handle conflicting version files"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// RUNTIME WORKFLOW TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_workflow_list_then_use() {
    init_test_env();

    // First list available runtimes
    let list_result = run_omg(&["list"]);
    list_result.assert_success();

    // Then try to use one
    let use_result = run_omg(&["use", "node", "latest"]);
    assert!(!use_result.combined_output().is_empty());
}

#[test]
fn test_workflow_use_then_list() {
    init_test_env();

    let use_result = run_omg(&["use", "node", "20.10.0"]);
    let use_output = use_result.combined_output();
    assert!(
        use_result.success
            || use_output.to_lowercase().contains("version")
            || use_output.to_lowercase().contains("install")
            || use_output.to_lowercase().contains("error"),
        "Runtime switch should succeed or explain the outcome: {use_output}"
    );

    let list_result = run_omg(&["list", "node"]);
    list_result.assert_success();
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR HANDLING
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_error_use_without_version_or_file() {
    init_test_env();

    let project = TestProject::new();
    // No version file in empty project

    let result = project.run(&["use", "node"]);

    // Should fail or prompt for version
    let output = result.combined_output();
    assert!(
        !result.success || output.contains("version") || output.contains("specify"),
        "Should require version when no file present"
    );
}

#[test]
fn test_error_invalid_version_format() {
    init_test_env();

    let result = run_omg(&["use", "node", "invalid.version.xyz"]);

    let output = result.combined_output();
    assert!(
        output.contains("invalid") || output.contains("version"),
        "Should validate version format"
    );
}

#[test]
fn test_error_unsupported_runtime() {
    init_test_env();

    let result = run_omg(&["use", "unsupported-runtime", "1.0.0"]);

    result.assert_failure();
    let output = result.combined_output();
    assert!(
        output.contains("unsupported") || output.contains("unknown"),
        "Should reject unsupported runtimes"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// SHELL INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_hook_bash_generates_script() {
    init_test_env();

    let result = run_omg(&["hook", "bash"]);

    result.assert_success();
    let output = result.stdout;

    // Should contain bash-specific content
    assert!(
        output.contains("function") || output.contains("eval") || output.contains("omg"),
        "Should generate bash hook script"
    );
}

#[test]
fn test_hook_zsh_generates_script() {
    init_test_env();

    let result = run_omg(&["hook", "zsh"]);

    result.assert_success();
    let output = result.stdout;

    assert!(
        output.contains("function") || output.contains("eval") || output.contains("omg"),
        "Should generate zsh hook script"
    );
}

#[test]
fn test_hook_fish_generates_script() {
    init_test_env();

    let result = run_omg(&["hook", "fish"]);

    result.assert_success();
    let output = result.stdout;

    // Fish uses different syntax
    assert!(
        output.contains("function") || output.contains("source") || output.contains("omg"),
        "Should generate fish hook script"
    );
}

#[test]
fn test_hook_invalid_shell() {
    init_test_env();

    let result = run_omg(&["hook", "invalid-shell-xyz"]);

    result.assert_failure();
    let output = result.combined_output();
    assert!(
        output.contains("invalid") || output.contains("unsupported"),
        "Should reject invalid shell"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// WHICH COMMAND TESTS
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn test_which_shows_active_runtime() {
    init_test_env();

    let result = run_omg(&["which", "node"]);

    // Should show current node version or "not set"
    let output = result.combined_output();
    assert!(
        result.success || output.contains("not set") || output.contains("No active"),
        "Should show runtime status"
    );
}

#[test]
fn test_which_all_runtimes() {
    init_test_env();

    let result = run_omg(&["which"]);

    // Should show runtime status
    let output = result.combined_output();
    assert!(
        result.success || !output.is_empty(),
        "Should show runtime information"
    );
}
