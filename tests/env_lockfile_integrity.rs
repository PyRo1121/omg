//! Lockfile integrity pins (`omg.lock`).
//!
//! Wave-2 addition: the hardened integrity check in
//! `omg_lib::core::env::fingerprint::EnvironmentState::load` (stored-hash vs
//! recomputed-content comparison) previously had no `tests/**` coverage — the
//! only related CLI test accepted any failure text, so a silent regression to
//! unparsed-trust semantics would have gone unnoticed.
//!
//! These tests pin, at both the library seam and through the real binary,
//! that a well-formed but tampered lockfile fails *its integrity check*, not
//! merely "some error".

#![expect(clippy::unwrap_used)]

pub mod common;

use common::TestProject;
use omg_lib::core::env::fingerprint::EnvironmentState;
use std::collections::HashMap;

fn sample_state() -> EnvironmentState {
    let mut state = EnvironmentState {
        schema_version: omg_lib::core::env::fingerprint::EnvironmentState::SCHEMA_VERSION,
        runtimes: HashMap::new(),
        packages: vec!["curl".to_string(), "git".to_string()],
        timestamp: 1_700_000_000,
        hash: String::new(),
    };
    state.hash = state.calculate_hash();
    state
}

#[test]
fn save_then_load_round_trips_and_recomputes_the_hash() -> anyhow::Result<()> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("omg.lock");

    let state = sample_state();
    // `save` normalizes and re-computes the stored hash from contents.
    state.save(&path)?;

    let loaded = EnvironmentState::load(&path)?;
    assert_eq!(loaded, state, "round trip must preserve the captured state");
    assert_eq!(loaded.hash, loaded.calculate_hash());
    Ok(())
}

#[test]
fn load_rejects_a_tampered_lockfile_with_an_integrity_error() -> anyhow::Result<()> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("omg.lock");

    // Persist a valid, self-consistent lockfile first.
    let contents = {
        let state = sample_state();
        toml::to_string_pretty(&state)?
    };
    std::fs::write(&path, &contents)?;

    // Inject an extra package through the TOML value model so the on-disk
    // file is valid TOML with a valid schema but contents that contradict
    // the stored hash — exactly what an attacker editing the lockfile by
    // hand produces.
    let mut value: toml::Value = toml::from_str(&contents)?;
    value["packages"]
        .as_array_mut()
        .expect("packages is an array")
        .push(toml::Value::String("injected-pkg".to_string()));
    std::fs::write(&path, toml::to_string_pretty(&value)?)?;

    let error =
        EnvironmentState::load(&path).expect_err("tampered lockfile must fail its integrity check");
    assert!(
        error.to_string().contains("integrity check failed"),
        "expected integrity failure, got: {error}"
    );
    Ok(())
}

#[test]
fn load_rejects_malformed_toml_instead_of_panicking() -> anyhow::Result<()> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("omg.lock");
    std::fs::write(&path, "this is not valid toml {{{{")?;

    let result = EnvironmentState::load(&path);
    assert!(result.is_err(), "malformed TOML must be rejected");
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Failed to parse lockfile"),
        "parse failures must carry context"
    );
    Ok(())
}

#[test]
fn load_reports_a_missing_lockfile_as_a_read_failure() -> anyhow::Result<()> {
    let dir = tempfile::TempDir::new()?;
    let path = dir.path().join("does-not-exist.lock");

    let error =
        EnvironmentState::load(&path).expect_err("missing lockfile must be an explicit error");
    assert!(error.to_string().contains("Failed to read lockfile"));
    Ok(())
}

/// Integration pin through the real binary: `omg env check` must reject a
/// tampered lockfile via the integrity path, not silently treat attacker
/// edits as drift to report.
#[test]
fn env_check_fails_on_tampered_lockfile_integrity() {
    let project = TestProject::new();

    // Write a valid-schema lockfile whose contents contradict its stored hash.
    let mut state = EnvironmentState {
        schema_version: omg_lib::core::env::fingerprint::EnvironmentState::SCHEMA_VERSION,
        runtimes: HashMap::new(),
        packages: vec![],
        timestamp: 0,
        hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
    };
    state.packages.push("tampered-pkg".to_string());
    let contents = toml::to_string_pretty(&state).expect("tampered fixture must serialize");
    project.create_file("omg.lock", &contents);

    let result = project.run(&["env", "check"]);
    let combined = result.combined_output();
    assert!(
        !result.success,
        "`env check` must fail on a tampered lockfile, got:\n{combined}"
    );
    assert!(
        combined.contains("integrity check failed"),
        "`env check` must surface the integrity failure specifically, got:\n{combined}"
    );
}
