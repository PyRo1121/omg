//! Contract tests for `src/cli/security.rs` — SLSA identity policy surface.
//!
//! Target contracts:
//! 1. `check_slsa` is gated on the exact feature name "slsa" requiring the
//!    Enterprise tier, and the failure names both.
//! 2. The license gate fires BEFORE path validation / provenance verification,
//!    so an unlicensed invocation can never reach the filesystem probe or the
//!    Sigstore verifier.
//! 3. A self-asserted (unsigned/forged) enterprise license.json must degrade to
//!    Free and must NOT unlock SLSA verification — the JWT fail-closed policy.
//! 4. The `--certificate-identity` flag exists in the CLI contract (parsed by
//!    clap, forwarded past the gate), so trust-policy regressions that drop the
//!    flag break loudly.
//!
//! Everything here runs through the real binary via the shared harness; no
//! internal APIs are called directly.

pub mod common;

use common::{CommandResult, TestProject};

const GATE_MARKER: &str = "'slsa'";

fn assert_paid_tier_gate(result: &CommandResult) {
    result.assert_failure();
    let output = result.combined_output();
    assert!(
        output.contains("requires") && output.contains("tier"),
        "expected the paid-tier license gate, got:\n{output}"
    );
}

fn assert_slsa_gate(result: &CommandResult) {
    assert_paid_tier_gate(result);
    let output = result.combined_output();
    assert!(
        output.contains(GATE_MARKER),
        "the gate must name the exact 'slsa' feature, got:\n{output}"
    );
    assert!(
        output.contains("Enterprise"),
        "SLSA requires the Enterprise tier and the error must say so, got:\n{output}"
    );
}

/// Contract 1: the gate names the exact feature ("slsa") and the exact tier
/// (Enterprise). A typo'd feature registration, a tier demotion, or a generic
/// "permission denied" rewrite all break this.
#[test]
fn slsa_check_gate_names_feature_and_required_tier() {
    let project = TestProject::new();
    // Package argument deliberately points at a nonexistent file: if this ever
    // stops failing at the gate we want to know from THIS test too.
    let result = project.run(&["audit", "slsa", "artifact.bin"]);
    assert_slsa_gate(&result);
}

/// Contract 2: the license gate precedes path resolution AND provenance
/// verification. For an existing artifact the gate must still fire (never a
/// verifier attempt); for a missing one we must see the license error, not
/// "File not found".
#[test]
fn slsa_license_gate_precedes_path_resolution_and_verification() {
    let project = TestProject::new();
    project.create_file("artifact.bin", "not-a-real-attestation\n");

    let existing = project.run(&["audit", "slsa", "artifact.bin"]);
    assert_slsa_gate(&existing);
    let existing_out = existing.combined_output();
    assert!(
        !existing_out.contains("SLSA verification failed"),
        "unlicensed invocation must never reach the Sigstore verifier, got:\n{existing_out}"
    );

    let missing = project.run(&["audit", "slsa", "ghost.bin"]);
    assert_slsa_gate(&missing);
    let missing_out = missing.combined_output();
    assert!(
        !missing_out.contains("File not found"),
        "gate must fire before the existence check, got:\n{missing_out}"
    );
}

/// Contract 3: a forged, self-asserted license file (claims enterprise, carries
/// a garbage JWT) must fail closed to Free and keep SLSA locked. This pins the
/// JWT-verified-tier policy: `StoredLicense::tier_enum` derives the tier from
/// the *signed token*, never from the plaintext `tier` string on disk.
#[test]
fn forged_self_asserted_license_does_not_unlock_slsa() {
    let project = TestProject::new();
    project.create_file("artifact.bin", "payload\n");

    // Shape mirrors what `src/cli/ci.rs`'s generated CI workflow writes as a
    // "Mock Enterprise License": if that ever started working, this contract
    // catches the privilege hole immediately.
    std::fs::write(
        project.data_dir.path().join("license.json"),
        r#"{
            "key": "FORGED-CI-MOCK-KEY",
            "tier": "enterprise",
            "features": ["sbom", "audit", "secrets", "slsa", "policy"],
            "validated_at": 9999999999,
            "token": "eyJhbGciOiJFZERTQSIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJmb3JnZWQifQ.Zm9yZ2VkLXNpZw",
            "machine_id": null
        }"#,
    )
    .expect("write forged license fixture");

    let result = project.run(&["audit", "slsa", "artifact.bin"]);
    assert_slsa_gate(&result);
    let out = result.combined_output();
    assert!(
        !out.contains("Checking SLSA provenance"),
        "a forged license must never get far enough to start verification, got:\n{out}"
    );
    assert!(
        !out.contains("SLSA verification passed"),
        "a forged license must never produce a passing SLSA verdict, got:\n{out}"
    );
}

/// Contract 4: `--certificate-identity` is part of the CLI contract. An
/// unlicensed run must fail at the license gate (proving clap accepted the
/// flag and dispatched into check_slsa), not with an argument-parse error.
#[test]
fn certificate_identity_flag_reaches_the_gated_command() {
    let project = TestProject::new();
    let result = project.run(&[
        "audit",
        "slsa",
        "artifact.bin",
        "--certificate-identity",
        "release@example.com",
    ]);
    assert_slsa_gate(&result);
    let out = result.combined_output();
    assert!(
        !out.contains("unexpected argument") && !out.contains("Unrecognized command"),
        "--certificate-identity must be a real CLI flag, got clap rejection:\n{out}"
    );
}

/// Contract 5: `audit fix` (the auto-fix flow in security.rs) is gated on the
/// "audit" feature BEFORE any daemon connection attempt. Dropping the gate or
/// reordering it behind the daemon connect would leak paid functionality to
/// free users whenever a daemon happens to be running.
#[test]
fn audit_fix_is_license_gated_before_daemon_connect() {
    let project = TestProject::new();
    let result = project.run(&["audit", "fix"]);
    assert_paid_tier_gate(&result);
    let out = result.combined_output();
    assert!(
        out.contains("'audit'"),
        "the fix flow must name its exact gating feature 'audit', got:\n{out}"
    );
    assert!(
        !out.contains("Daemon not running"),
        "gate must fire before any daemon connection attempt, got:\n{out}"
    );
}
