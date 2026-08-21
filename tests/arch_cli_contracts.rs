#![cfg(feature = "arch")]

use std::path::Path;
use std::process::{Command, Output};

use anyhow::{Context, Result};
use tempfile::TempDir;

fn run(root: &Path, args: &[&str]) -> Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_omg"))
        .args(args)
        .current_dir(root)
        .env("OMG_TEST_MODE", "1")
        .env("OMG_TEST_DISTRO", "arch")
        .env("OMG_DISABLE_TELEMETRY", "1")
        .env("OMG_DATA_DIR", root.join("data"))
        .env("OMG_CONFIG_DIR", root.join("config"))
        .env("NO_COLOR", "1")
        .output()
        .context("omg command should start")
}

fn output_text(output: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn explicit_absolute_paths_work_for_portable_files() -> Result<()> {
    let root = TempDir::new()?;
    let manifest = root.path().join("manifest.json");
    let lock = root.path().join("omg.lock");

    let export = run(
        root.path(),
        &["migrate", "export", "--output", &manifest.to_string_lossy()],
    )?;
    assert!(export.status.success(), "{}", output_text(&export));

    let import = run(
        root.path(),
        &[
            "migrate",
            "import",
            &manifest.to_string_lossy(),
            "--dry-run",
        ],
    )?;
    assert!(import.status.success(), "{}", output_text(&import));

    let capture = run(root.path(), &["env", "capture"])?;
    assert!(capture.status.success(), "{}", output_text(&capture));
    assert!(lock.is_file());

    let diff = run(
        root.path(),
        &[
            "diff",
            "--from",
            &lock.to_string_lossy(),
            &lock.to_string_lossy(),
        ],
    )?;
    assert!(diff.status.success(), "{}", output_text(&diff));
    Ok(())
}

#[test]
fn unknown_config_keys_fail() -> Result<()> {
    let root = TempDir::new()?;
    let output = run(root.path(), &["config", "get", "not.a.real.key"])?;
    assert!(!output.status.success(), "{}", output_text(&output));
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
    )?;
    assert!(output.status.success(), "{}", output_text(&output));

    let export: serde_json::Value = serde_json::from_slice(&std::fs::read(export_path)?)?;
    assert!(export.get("local").is_some());
    assert!(export["remote"].is_null());
    Ok(())
}

#[test]
fn advertised_json_outputs_are_valid_json() -> Result<()> {
    let root = TempDir::new()?;
    for command in ["history", "stats"] {
        let output = run(root.path(), &["--json", command])?;
        assert!(output.status.success(), "{}", output_text(&output));
        serde_json::from_slice::<serde_json::Value>(&output.stdout)
            .with_context(|| format!("{command} emitted invalid JSON"))?;
    }
    Ok(())
}

#[test]
fn clean_dry_run_never_attempts_privilege_escalation() -> Result<()> {
    let root = TempDir::new()?;
    let output = run(root.path(), &["clean", "--all", "--dry-run"])?;
    assert!(output.status.success(), "{}", output_text(&output));
    assert!(!output_text(&output).contains("sudo:"));
    Ok(())
}
