#![cfg(feature = "arch")]
#![expect(clippy::unwrap_used, clippy::pedantic)]
//! Install-command integration tests (mock backend).
//!
//! Wave-2 honesty fix: these tests previously claimed to verify *real*
//! pacman/AUR installs while actually running under `OMG_TEST_MODE=1`, which
//! routes every operation to `MockPackageManager` — so their names lied and
//! their `sudo pacman -Rdd` cleanup was dead code. They are now named for
//! what they pin: the install command against the seeded mock backend, with
//! fully isolated per-test data directories (`TestProject`).
//!
//! Real-system install coverage remains gated behind
//! `OMG_RUN_DESTRUCTIVE_TESTS=1` in `install_update_comprehensive.rs`.

pub mod common;

use common::TestProject;

#[test]
fn test_mocked_install_seeded_package_dry_run_succeeds() {
    let project = TestProject::new();
    project.mock_available("firefox", "122.0").unwrap();

    let result = project.run(&["install", "--dry-run", "firefox"]);
    result.assert_success();
    result.assert_stdout_contains("firefox");
}

#[test]
fn test_mocked_install_dry_run_handles_multiple_seeded_packages() {
    let project = TestProject::new();
    project.mock_available("firefox", "122.0").unwrap();
    project.mock_available("git", "2.43.0").unwrap();

    let result = project.run(&["install", "--dry-run", "firefox", "git"]);
    result.assert_success();
    result.assert_stdout_contains("firefox");
    result.assert_stdout_contains("git");
}

#[test]
fn test_mocked_install_dry_run_does_not_change_state() {
    let project = TestProject::new();
    project.mock_available("firefox", "122.0").unwrap();

    let state_path = project.data_dir.path().join("mock_state_pacman.json");
    let before = std::fs::read(&state_path).unwrap();
    let result = project.run(&["install", "--dry-run", "firefox"]);
    result.assert_success();
    assert_eq!(
        std::fs::read(state_path).unwrap(),
        before,
        "dry run must not write mock package state"
    );
}

#[test]
fn test_mocked_install_seeded_installed_package_succeeds() {
    let project = TestProject::new();
    project.mock_install("firefox", "122.0").unwrap();

    let result = project.run(&["install", "--yes", "firefox"]);
    result.assert_success();
}

#[test]
fn test_mocked_install_honors_banned_package_policy() {
    let project = TestProject::new();
    project.mock_available("firefox", "122.0").unwrap();
    std::fs::write(
        project.config_dir.path().join("policy.toml"),
        "banned_packages = [\"firefox\"]\n",
    )
    .unwrap();

    let result = project.run(&["install", "--yes", "firefox"]);
    result.assert_failure();
    assert!(
        result.combined_output().contains("banned"),
        "policy failure must name the ban: {}",
        result.combined_output()
    );
    let state: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(project.data_dir.path().join("mock_state_pacman.json")).unwrap(),
    )
    .unwrap();
    assert!(
        state["installed"].get("firefox").is_none(),
        "banned package must not install"
    );
}

#[test]
fn test_mocked_install_missing_package_fails_explicitly() {
    let project = TestProject::new();
    let fake_pkg = "this-package-does-not-exist-12345";

    let result = project.run(&["install", fake_pkg]);
    result.assert_failure();

    let output = result.combined_output();
    assert!(
        output.contains("not found"),
        "expected 'not found' in output, got: {output}"
    );
}
