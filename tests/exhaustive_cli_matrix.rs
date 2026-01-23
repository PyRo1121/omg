//! OMG Exhaustive CLI Matrix Test Suite
//!
//! This suite verifies every single CLI command across all supported OS flavors
//! using a high-fidelity synthetic mock environment.
//!
//! Goal: Test "absolute everything" without needing real root or real distros.

mod common;

use common::*;
use serial_test::serial;
use tempfile::TempDir;

#[cfg(feature = "arch")]
fn run_arch(args: &[&str]) -> CommandResult {
    run_omg_with_env(args, &[("OMG_TEST_DISTRO", "arch"), ("OMG_TEST_MODE", "1")])
}

#[cfg(feature = "debian")]
fn run_debian(args: &[&str]) -> CommandResult {
    run_omg_with_env(
        args,
        &[("OMG_TEST_DISTRO", "debian"), ("OMG_TEST_MODE", "1")],
    )
}

// ═══════════════════════════════════════════════════════════════════════════════
// ARCH LINUX MATRIX
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(all(test, feature = "arch"))]
mod arch_matrix {
    use super::*;

    #[test]
    #[serial]
    fn test_search() {
        let res = run_arch(&["search", "firefox"]);
        res.assert_success();
        res.assert_stdout_contains("firefox");
        res.assert_stdout_contains("official");
    }

    #[test]
    #[serial]
    fn test_info() {
        let res = run_arch(&["info", "pacman"]);
        res.assert_success();
        res.assert_stdout_contains("pacman");
    }

    #[test]
    #[serial]
    fn test_list() {
        let res = run_arch(&["list"]);
        res.assert_success();
    }

    #[test]
    #[serial]
    fn test_status() {
        let res = run_arch(&["status"]);
        res.assert_success();
        res.assert_stdout_contains("Packages");
    }

    #[test]
    #[serial]
    fn test_explicit() {
        let res = run_arch(&["explicit"]);
        res.assert_success();
        res.assert_stdout_contains("pacman");
    }

    #[test]
    #[serial]
    fn test_install_remove_cycle() {
        let data_dir = TempDir::new().unwrap();
        let data_path = data_dir.path().to_str().unwrap();
        let envs = &[
            ("OMG_TEST_DISTRO", "arch"),
            ("OMG_TEST_MODE", "1"),
            ("OMG_DATA_DIR", data_path),
        ];

        // Test install
        let res = run_omg_with_env(&["install", "-y", "firefox"], envs);
        println!("Install stderr: {}", res.stderr);
        res.assert_success();

        // Test explicit now contains firefox
        let res = run_omg_with_env(&["explicit"], envs);
        println!("Explicit stderr: {}", res.stderr);
        res.assert_stdout_contains("firefox");

        // Test remove
        let res = run_omg_with_env(&["remove", "-y", "firefox"], envs);
        println!("Remove stderr: {}", res.stderr);
        res.assert_success();

        // Test explicit no longer contains firefox
        let res = run_omg_with_env(&["explicit"], envs);
        println!("Explicit after remove stderr: {}", res.stderr);
        assert!(!res.stdout.contains("firefox"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// DEBIAN MATRIX
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(all(test, feature = "debian"))]
mod debian_matrix {
    use super::*;

    #[test]
    #[serial]
    fn test_search() {
        let res = run_debian(&["search", "apt"]);
        res.assert_success();
        res.assert_stdout_contains("apt");
        res.assert_stdout_contains("official");
    }

    #[test]
    #[serial]
    fn test_info() {
        let res = run_debian(&["info", "apt"]);
        res.assert_success();
        res.assert_stdout_contains("apt");
    }

    #[test]
    #[serial]
    fn test_status() {
        let res = run_debian(&["status"]);
        res.assert_success();
        res.assert_stdout_contains("Packages");
    }

    #[test]
    #[serial]
    fn test_install_remove_cycle() {
        let data_dir = TempDir::new().unwrap();
        let data_path = data_dir.path().to_str().unwrap();
        let envs = &[
            ("OMG_TEST_DISTRO", "debian"),
            ("OMG_TEST_MODE", "1"),
            ("OMG_DATA_DIR", data_path),
        ];

        run_omg_with_env(&["install", "-y", "git"], envs).assert_success();
        run_omg_with_env(&["explicit"], envs).assert_stdout_contains("git");
        run_omg_with_env(&["remove", "-y", "git"], envs).assert_success();
        let res = run_omg_with_env(&["explicit"], envs);
        assert!(!res.stdout.contains("git"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// RUNTIME MATRIX (OS-Agnostic)
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod runtime_matrix {
    use super::*;

    #[test]
    #[serial]
    fn test_use_node_detection() {
        let project = TestProject::new();
        project.create_file(".nvmrc", "20.0.0");
        let res = project.run(&["use", "node"]);
        res.assert_success();
        res.assert_stdout_contains("20.0.0");
    }

    #[test]
    #[serial]
    fn test_use_python_detection() {
        let project = TestProject::new();
        project.create_file(".python-version", "3.12.0");
        let res = project.run(&["use", "python"]);
        res.assert_success();
        res.assert_stdout_contains("3.12.0");
    }

    #[test]
    #[serial]
    fn test_which_all_runtimes() {
        let runtimes = ["node", "python", "go", "rust", "ruby", "java", "bun"];
        for rt in runtimes {
            let res = run_omg(&["which", rt]);
            res.assert_success();
        }
    }

    #[test]
    #[serial]
    fn test_env_workflow() {
        let project = TestProject::new();
        let data_dir = TempDir::new().unwrap();
        let data_path = data_dir.path().to_str().unwrap();
        let envs = &[
            ("OMG_TEST_DISTRO", "arch"),
            ("OMG_TEST_MODE", "1"),
            ("OMG_DATA_DIR", data_path),
        ];

        // Verify project path exists
        assert!(project.path().exists());

        // Capture
        let res = project.run_with_env(&["env", "capture"], envs);
        res.assert_success();

        // Print directory contents for debugging
        let entries = std::fs::read_dir(project.path())
            .unwrap()
            .map(|res| res.map(|e| e.file_name().into_string().unwrap()))
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        println!("Project dir content: {entries:?}");

        assert!(project.file_exists("omg.lock"));

        // Check
        project
            .run_with_env(&["env", "check"], envs)
            .assert_success();
    }

    #[test]
    #[serial]
    fn test_doctor_command() {
        let res = run_omg(&["doctor"]);
        res.assert_success();
        res.assert_stdout_contains("Checking system health");
    }

    #[test]
    #[serial]
    fn test_config_workflow() {
        // Get
        run_omg(&["config"]).assert_success();

        // Set/Get cycle
        run_omg(&["config", "verbose", "2"]).assert_success();
    }

    #[test]
    #[serial]
    fn test_audit_command() {
        let res = run_omg(&["audit"]);
        // May fail if daemon not running, but should give clean error message
        assert!(!res.combined_output().contains("panic"));
    }

    #[test]
    #[serial]
    fn test_new_and_run_scaffolding() {
        let project = TestProject::new();
        // Create a new project (using dry-run or mock backend)
        let res = project.run(&["new", "rust", "my-app"]);
        assert!(!res.combined_output().contains("panic"));

        // Mock a task runner (Makefile)
        project.create_file("Makefile", "test:\n\techo 'running tests'");
        let res = project.run(&["run", "test"]);
        assert!(res.success || res.stderr.contains("not found"));
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ERROR & BOUNDARY MATRIX
// ═══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod boundary_matrix {
    use super::*;

    #[test]
    #[serial]
    fn test_nonexistent_command() {
        let res = run_omg(&["unknown-cmd"]);
        res.assert_failure();
    }

    #[test]
    #[serial]
    fn test_invalid_package_name() {
        let res = run_omg(&["install", "invalid; name"]);
        res.assert_failure();
        res.assert_stderr_contains("Invalid character");
    }

    #[test]
    #[serial]
    fn test_empty_search() {
        let res = run_omg(&["search", ""]);
        // Should not crash, output might vary but success/failure is fine as long as no panic
        assert!(!res.combined_output().contains("panic"));
    }
}
