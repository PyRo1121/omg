//! OMG Test Infrastructure - Fortune 500 Grade
//!
//! Comprehensive testing utilities, fixtures, mocks, and helpers
//! for enterprise-grade test coverage.

#![allow(dead_code)] // Test utilities may not all be used in every test file

pub mod assertions;
pub mod fixtures;
pub mod mocks;
pub mod runners;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Once;
use std::time::{Duration, Instant};

use tempfile::TempDir;

// Re-export serial_test for use in test files
#[allow(unused_imports)]
pub use serial_test::serial;

// ═══════════════════════════════════════════════════════════════════════════════
// TEST CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════════

static INIT: Once = Once::new();

/// Initialize test environment (called once per test run)
///
/// Note: Environment variables are set once at initialization. Tests that need
/// to modify environment variables should use the `#[serial]` attribute from
/// the `serial_test` crate to prevent data races.
pub fn init_test_env() {
    INIT.call_once(|| {
        // SAFETY: We are in a single-threaded context during Once::call_once initialization.
        // In Rust 2024, set_var is unsafe due to potential data races in multi-threaded programs.
        // Since this is called at the very beginning of the test suite, it is safe.
        unsafe {
            std::env::set_var("OMG_TEST_MODE", "1");
            std::env::set_var("OMG_DISABLE_TELEMETRY", "1");
            std::env::set_var("OMG_LOG_LEVEL", "warn");
        }
    });
}

/// Test configuration flags
#[derive(Debug, Clone)]
#[allow(dead_code)]
#[allow(clippy::struct_excessive_bools)]
pub struct TestConfig {
    pub run_system_tests: bool,
    pub run_network_tests: bool,
    pub run_destructive_tests: bool,
    pub run_perf_tests: bool,
    pub run_fuzz_tests: bool,
    pub run_stress_tests: bool,
    pub target_distro: Option<String>,
    pub timeout: Duration,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            run_system_tests: env::var("OMG_RUN_SYSTEM_TESTS")
                .map(|v| v == "1")
                .unwrap_or(false),
            run_network_tests: env::var("OMG_RUN_NETWORK_TESTS")
                .map(|v| v == "1")
                .unwrap_or(false),
            run_destructive_tests: env::var("OMG_RUN_DESTRUCTIVE_TESTS")
                .map(|v| v == "1")
                .unwrap_or(false),
            run_perf_tests: env::var("OMG_RUN_PERF_TESTS")
                .map(|v| v == "1")
                .unwrap_or(false),
            run_fuzz_tests: env::var("OMG_RUN_FUZZ_TESTS")
                .map(|v| v == "1")
                .unwrap_or(false),
            run_stress_tests: env::var("OMG_RUN_STRESS_TESTS")
                .map(|v| v == "1")
                .unwrap_or(false),
            target_distro: env::var("OMG_TEST_DISTRO").ok(),
            timeout: Duration::from_secs(30),
        }
    }
}

#[allow(dead_code)]
impl TestConfig {
    pub fn skip_if_no_system(&self, test_name: &str) -> bool {
        if self.run_system_tests {
            false
        } else {
            eprintln!("⏭️  Skipping {test_name} (set OMG_RUN_SYSTEM_TESTS=1)");
            true
        }
    }

    pub fn skip_if_no_network(&self, test_name: &str) -> bool {
        if self.run_network_tests {
            false
        } else {
            eprintln!("⏭️  Skipping {test_name} (set OMG_RUN_NETWORK_TESTS=1)");
            true
        }
    }

    pub fn skip_if_no_destructive(&self, test_name: &str) -> bool {
        if self.run_destructive_tests {
            false
        } else {
            eprintln!("⏭️  Skipping {test_name} (set OMG_RUN_DESTRUCTIVE_TESTS=1)");
            true
        }
    }

    pub fn is_arch(&self) -> bool {
        self.target_distro.as_deref() == Some("arch") || Path::new("/etc/arch-release").exists()
    }

    pub fn is_debian(&self) -> bool {
        self.target_distro.as_deref() == Some("debian")
            || (Path::new("/etc/debian_version").exists() && !self.is_ubuntu())
    }

    pub fn is_ubuntu(&self) -> bool {
        self.target_distro.as_deref() == Some("ubuntu")
            || fs::read_to_string("/etc/os-release")
                .map(|s| s.contains("Ubuntu"))
                .unwrap_or(false)
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// COMMAND EXECUTION
// ═══════════════════════════════════════════════════════════════════════════════

/// Result from running an OMG command
#[derive(Debug, Clone)]
pub struct CommandResult {
    pub success: bool,
    #[allow(dead_code)]
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}

#[allow(dead_code)]
impl CommandResult {
    pub fn combined_output(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }

    pub fn contains(&self, needle: &str) -> bool {
        self.stdout.contains(needle) || self.stderr.contains(needle)
    }

    pub fn stdout_contains(&self, needle: &str) -> bool {
        self.stdout.contains(needle)
    }

    pub fn stderr_contains(&self, needle: &str) -> bool {
        self.stderr.contains(needle)
    }

    pub fn assert_success(&self) {
        assert!(
            self.success,
            "Command failed with exit code {}:\nstdout: {}\nstderr: {}",
            self.exit_code, self.stdout, self.stderr
        );
    }

    pub fn assert_failure(&self) {
        assert!(
            !self.success,
            "Command unexpectedly succeeded:\nstdout: {}\nstderr: {}",
            self.stdout, self.stderr
        );
    }

    pub fn assert_stdout_contains(&self, needle: &str) {
        assert!(
            self.stdout.contains(needle),
            "stdout does not contain '{}'\nstdout: {}",
            needle,
            self.stdout
        );
    }

    pub fn assert_stderr_contains(&self, needle: &str) {
        assert!(
            self.stderr.contains(needle),
            "stderr does not contain '{}'\nstderr: {}",
            needle,
            self.stderr
        );
    }

    pub fn assert_duration_under(&self, max: Duration) {
        assert!(
            self.duration < max,
            "Command took {:?}, expected under {:?}",
            self.duration,
            max
        );
    }
}

/// Run an OMG command
pub fn run_omg(args: &[&str]) -> CommandResult {
    run_omg_with_options(args, None, &[])
}

/// Run an OMG command in a specific directory
pub fn run_omg_in_dir(args: &[&str], dir: &Path) -> CommandResult {
    run_omg_with_options(args, Some(dir), &[])
}

/// Run an OMG command with environment variables
pub fn run_omg_with_env(args: &[&str], env_vars: &[(&str, &str)]) -> CommandResult {
    run_omg_with_options(args, None, env_vars)
}

/// Run an OMG command with full options
pub fn run_omg_with_options(
    args: &[&str],
    dir: Option<&Path>,
    env_vars: &[(&str, &str)],
) -> CommandResult {
    let start = Instant::now();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omg"));
    cmd.args(args)
        .env("OMG_TEST_MODE", "1")
        .env("OMG_DISABLE_DAEMON", "1")
        .env("OMG_DISABLE_TELEMETRY", "1")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Isolate tests by using unique data/config dirs if not provided
    let temp_data = TempDir::new().unwrap();
    let temp_config = TempDir::new().unwrap();
    
    let has_data_dir = env_vars.iter().any(|(k, _)| *k == "OMG_DATA_DIR");
    let has_config_dir = env_vars.iter().any(|(k, _)| *k == "OMG_CONFIG_DIR");

    if !has_data_dir {
        cmd.env("OMG_DATA_DIR", temp_data.path());
    }
    if !has_config_dir {
        cmd.env("OMG_CONFIG_DIR", temp_config.path());
    }

    if let Some(d) = dir {
        cmd.current_dir(d);
    }

    for (key, value) in env_vars {
        cmd.env(key, value);
    }

    let output = cmd.output().expect("Failed to execute omg");
    let duration = start.elapsed();

    CommandResult {
        success: output.status.success(),
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration,
    }
}

/// Run a raw shell command
#[allow(dead_code)]
pub fn run_shell(cmd: &str) -> CommandResult {
    let start = Instant::now();

    let output = Command::new("sh")
        .args(["-c", cmd])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute shell command");

    let duration = start.elapsed();

    CommandResult {
        success: output.status.success(),
        exit_code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEST PROJECT HELPERS
// ═══════════════════════════════════════════════════════════════════════════════

/// A test project with managed temp directory
#[allow(dead_code)]
pub struct TestProject {
    pub dir: TempDir,
    pub config: TestConfig,
}

#[allow(dead_code)]
impl TestProject {
    pub fn new() -> Self {
        init_test_env();
        Self {
            dir: TempDir::new().expect("Failed to create temp dir"),
            config: TestConfig::default(),
        }
    }

    pub fn with_config(config: TestConfig) -> Self {
        init_test_env();
        Self {
            dir: TempDir::new().expect("Failed to create temp dir"),
            config,
        }
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    pub fn run(&self, args: &[&str]) -> CommandResult {
        run_omg_in_dir(args, self.path())
    }

    pub fn run_with_env(&self, args: &[&str], env_vars: &[(&str, &str)]) -> CommandResult {
        run_omg_with_options(args, Some(self.path()), env_vars)
    }

    /// Create a file in the project
    pub fn create_file(&self, name: &str, content: &str) -> PathBuf {
        let path = self.path().join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, content).unwrap();
        path
    }

    /// Create a directory in the project
    pub fn create_dir(&self, name: &str) -> PathBuf {
        let path = self.path().join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// Read a file from the project
    pub fn read_file(&self, name: &str) -> Option<String> {
        fs::read_to_string(self.path().join(name)).ok()
    }

    /// Check if a file exists
    pub fn file_exists(&self, name: &str) -> bool {
        self.path().join(name).exists()
    }

    // Project templates

    pub fn with_node_project(&self) -> &Self {
        self.create_file(".nvmrc", "20.10.0");
        self.create_file(
            "package.json",
            r#"{"name": "test", "engines": {"node": ">=18.0.0"}}"#,
        );
        self
    }

    pub fn with_python_project(&self) -> &Self {
        self.create_file(".python-version", "3.11.0");
        self.create_file("requirements.txt", "requests==2.31.0\npytest==7.4.0");
        self
    }

    pub fn with_rust_project(&self) -> &Self {
        self.create_file("rust-toolchain.toml", "[toolchain]\nchannel = \"stable\"");
        self.create_file(
            "Cargo.toml",
            "[package]\nname = \"test\"\nversion = \"0.1.0\"",
        );
        self
    }

    pub fn with_go_project(&self) -> &Self {
        self.create_file("go.mod", "module test\n\ngo 1.21");
        self
    }

    pub fn with_tool_versions(&self, versions: &[(&str, &str)]) -> &Self {
        let content: String = versions
            .iter()
            .map(|(k, v)| format!("{k} {v}"))
            .collect::<Vec<_>>()
            .join("\n");
        self.create_file(".tool-versions", &content);
        self
    }

    pub fn with_mise_config(&self, tools: &[(&str, &str)]) -> &Self {
        let tools_str: String = tools
            .iter()
            .map(|(k, v)| format!("{k} = \"{v}\""))
            .collect::<Vec<_>>()
            .join("\n");
        self.create_file(".mise.toml", &format!("[tools]\n{tools_str}"));
        self
    }

    pub fn with_omg_lock(&self, content: &str) -> &Self {
        self.create_file("omg.lock", content);
        self
    }

    pub fn with_security_policy(&self, policy: &str) -> &Self {
        self.create_dir(".config/omg");
        self.create_file(".config/omg/policy.toml", policy);
        self
    }

    pub fn with_team_config(&self, team_id: &str) -> &Self {
        self.create_dir(".omg");
        self.create_file(
            ".omg/team.toml",
            &format!("[team]\nid = \"{team_id}\"\nname = \"Test Team\""),
        );
        self
    }
}

impl Default for TestProject {
    fn default() -> Self {
        Self::new()
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// PACKAGE MANAGER DETECTION
// ═══════════════════════════════════════════════════════════════════════════════

/// Detect the current distro's package manager
pub fn detect_package_manager() -> Option<&'static str> {
    if Path::new("/usr/bin/pacman").exists() {
        Some("pacman")
    } else if Path::new("/usr/bin/apt").exists() {
        Some("apt")
    } else if Path::new("/usr/bin/dnf").exists() {
        Some("dnf")
    } else {
        None
    }
}

/// Check if a package is installed (distro-agnostic)
pub fn is_package_installed(name: &str) -> bool {
    match detect_package_manager() {
        Some("pacman") => run_shell(&format!("pacman -Q {name} 2>/dev/null")).success,
        Some("apt") => run_shell(&format!("dpkg -l {name} 2>/dev/null | grep -q '^ii'")).success,
        _ => false,
    }
}

/// Get installed package version (distro-agnostic)
pub fn get_package_version(name: &str) -> Option<String> {
    match detect_package_manager() {
        Some("pacman") => {
            let result = run_shell(&format!("pacman -Q {name} 2>/dev/null | cut -d' ' -f2"));
            if result.success {
                Some(result.stdout.trim().to_string())
            } else {
                None
            }
        }
        Some("apt") => {
            let result = run_shell(&format!(
                "dpkg-query -W -f='${{Version}}' {name} 2>/dev/null"
            ));
            if result.success {
                Some(result.stdout.trim().to_string())
            } else {
                None
            }
        }
        _ => None,
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TEST MACROS
// ═══════════════════════════════════════════════════════════════════════════════

/// Skip test if condition is met
#[macro_export]
macro_rules! skip_if {
    ($cond:expr, $reason:expr) => {
        if $cond {
            eprintln!("⏭️  Skipping test: {}", $reason);
            return;
        }
    };
}

/// Require system tests to be enabled
#[macro_export]
macro_rules! require_system_tests {
    () => {
        let config = $crate::common::TestConfig::default();
        if config.skip_if_no_system(module_path!()) {
            return;
        }
    };
}

/// Require network tests to be enabled
#[macro_export]
macro_rules! require_network_tests {
    () => {
        let config = $crate::common::TestConfig::default();
        if config.skip_if_no_network(module_path!()) {
            return;
        }
    };
}

/// Require destructive tests to be enabled
#[macro_export]
macro_rules! require_destructive_tests {
    () => {
        let config = $crate::common::TestConfig::default();
        if config.skip_if_no_destructive(module_path!()) {
            return;
        }
    };
}

/// Require Arch Linux
#[macro_export]
macro_rules! require_arch {
    () => {
        let config = $crate::common::TestConfig::default();
        if !config.is_arch() {
            eprintln!("⏭️  Skipping test: requires Arch Linux");
            return;
        }
    };
}

/// Require Debian
#[macro_export]
macro_rules! require_debian {
    () => {
        let config = $crate::common::TestConfig::default();
        if !config.is_debian() {
            eprintln!("⏭️  Skipping test: requires Debian");
            return;
        }
    };
}

/// Require Ubuntu
#[macro_export]
macro_rules! require_ubuntu {
    () => {
        let config = $crate::common::TestConfig::default();
        if !config.is_ubuntu() {
            eprintln!("⏭️  Skipping test: requires Ubuntu");
            return;
        }
    };
}
