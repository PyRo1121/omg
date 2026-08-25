//! Debian-backend search tests.
//!
//! Only compiled when a debian backend feature is enabled. Under
//! `OMG_TEST_MODE` the package manager routes to the mock backend for
//! `OMG_TEST_DISTRO=debian`, whose index is seeded with apt, firefox-esr and
//! git (src/package_managers/mock.rs `MockPackageDb::debian_defaults`). That
//! makes the search contract hermetic: no live apt database or omgd daemon
//! required.

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use assert_cmd::Command;

struct DebianEnv {
    _data_dir: tempfile::TempDir,
    _config_dir: tempfile::TempDir,
}

fn debian_search_command(query: &str) -> (Command, DebianEnv) {
    let data_dir = tempfile::TempDir::new().expect("temp data dir");
    let config_dir = tempfile::TempDir::new().expect("temp config dir");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_omg"));
    cmd.env("OMG_TEST_MODE", "1")
        .env("OMG_TEST_DISTRO", "debian")
        .env("OMG_DISABLE_DAEMON", "1")
        .env("OMG_DISABLE_TELEMETRY", "1")
        .env("OMG_DATA_DIR", data_dir.path())
        .env("OMG_CONFIG_DIR", config_dir.path())
        .arg("search")
        .arg(query);
    (
        cmd,
        DebianEnv {
            _data_dir: data_dir,
            _config_dir: config_dir,
        },
    )
}

#[test]
fn test_omg_search_debian_mock_backend_returns_seeded_results() {
    // firefox-esr is a seeded mock default; the search must surface it with
    // its official source tag instead of an empty or errored result.
    let (mut cmd, _env) = debian_search_command("firefox-esr");
    let output = cmd.output().expect("spawn omg search");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "search must succeed, exit {:?}\nstdout: {stdout}\nstderr: {stderr}",
        output.status.code()
    );
    assert!(
        stdout.contains("firefox-esr"),
        "search must return the seeded package, got stdout: {stdout}"
    );
    assert!(
        stdout.contains("Official"),
        "seeded packages are official repo entries, got stdout: {stdout}"
    );
}

#[test]
fn test_omg_search_debian_mock_backend_reports_misses_gracefully() {
    // A name absent from the seeded index must produce an explicit,
    // successful "no results" report — never a crash or daemon error.
    let (mut cmd, _env) = debian_search_command("definitely-not-a-seeded-package-xyz");
    let output = cmd.output().expect("spawn omg search");
    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(
        output.status.success(),
        "a miss must not be an error, exit {:?}\nstdout: {stdout}",
        output.status.code()
    );
    assert!(
        stdout.contains("No results found"),
        "a miss must print an explicit no-results summary, got: {stdout}"
    );
}
