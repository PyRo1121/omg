#![cfg(feature = "arch")]

use std::path::Path;

use anyhow::{Context, Result};
use tempfile::TempDir;

pub mod common;

use common::CommandResult;

/// Run the CLI through the shared isolated runner, pinned to the Arch mock
/// backend and the given working directory. Replaces the former hand-rolled
/// `Command` copy that duplicated the runner's environment contract.
fn run(root: &Path, args: &[&str]) -> CommandResult {
    common::run_omg_with_options(
        args,
        Some(root),
        &[
            ("OMG_TEST_DISTRO", "arch"),
            ("NO_COLOR", "1"),
            ("OMG_DISABLE_TELEMETRY", "1"),
        ],
    )
}

fn output_text(output: &CommandResult) -> String {
    output.combined_output()
}

#[test]
fn explicit_absolute_paths_work_for_portable_files() -> Result<()> {
    let root = TempDir::new()?;
    let manifest = root.path().join("manifest.json");
    let lock = root.path().join("omg.lock");

    let export = run(
        root.path(),
        &["migrate", "export", "--output", &manifest.to_string_lossy()],
    );
    assert!(export.success, "{}", output_text(&export));

    let import = run(
        root.path(),
        &[
            "migrate",
            "import",
            &manifest.to_string_lossy(),
            "--dry-run",
        ],
    );
    assert!(import.success, "{}", output_text(&import));

    let capture = run(root.path(), &["env", "capture"]);
    assert!(capture.success, "{}", output_text(&capture));
    assert!(lock.is_file());

    let diff = run(
        root.path(),
        &[
            "diff",
            "--from",
            &lock.to_string_lossy(),
            &lock.to_string_lossy(),
        ],
    );
    assert!(diff.success, "{}", output_text(&diff));
    Ok(())
}

#[test]
fn unknown_config_keys_fail() -> Result<()> {
    let root = TempDir::new()?;
    let output = run(root.path(), &["config", "get", "not.a.real.key"]);
    assert!(!output.success, "{}", output_text(&output));
    assert!(output_text(&output).contains("Unknown config key"));
    Ok(())
}

#[test]
fn privacy_export_works_without_a_license() -> Result<()> {
    let root = TempDir::new()?;
    let export_path = root.path().join("privacy.json");
    let output = run(
        root.path(),
        &[
            "privacy",
            "export",
            "--output",
            &export_path.to_string_lossy(),
        ],
    );
    assert!(output.success, "{}", output_text(&output));

    let export: serde_json::Value = serde_json::from_slice(&std::fs::read(export_path)?)?;
    assert!(export.get("local").is_some());
    assert!(export["remote"].is_null());
    Ok(())
}

#[test]
fn advertised_json_outputs_are_valid_json() -> Result<()> {
    let root = TempDir::new()?;
    for command in ["history", "stats", "outdated"] {
        let output = run(root.path(), &["--json", command]);
        assert!(output.success, "{}", output_text(&output));
        serde_json::from_str::<serde_json::Value>(&output.stdout)
            .with_context(|| format!("{command} emitted invalid JSON"))?;
        assert!(
            output.stderr.is_empty(),
            "{command} emitted non-error diagnostics in JSON mode: {}",
            output.stderr
        );
    }
    Ok(())
}

#[test]
fn successful_requests_do_not_invent_transaction_summaries() -> Result<()> {
    let root = TempDir::new()?;
    for (args, unsupported_claim) in [
        (["install", "--yes", "firefox"], "Installed 1 package"),
        (
            ["remove", "--yes", "firefox"],
            "Packages removed successfully",
        ),
    ] {
        let output = run(root.path(), &args);
        assert!(output.success, "{}", output_text(&output));
        assert!(
            !output.stdout.contains(unsupported_claim),
            "{}",
            output_text(&output)
        );
    }
    Ok(())
}

#[test]
fn clean_dry_run_never_attempts_privilege_escalation() -> Result<()> {
    let root = TempDir::new()?;
    let output = run(root.path(), &["clean", "--all", "--dry-run"]);
    assert!(output.success, "{}", output_text(&output));
    assert!(!output_text(&output).contains("sudo:"));
    Ok(())
}
