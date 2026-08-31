//! Contract tests for `src/cli/team.rs` — team init/join/status/push/pull CLI handlers.
//!
//! Every test pins an observable contract (exact output text, exact state change,
//! or an error naming cause + remedy) so that breaking the handler flips the test.
//! License-gated commands are exercised against the Free tier (no license in the
//! isolated data dir): paid tiers cannot verify offline, so the *gate* contract
//! ("command names the feature, tier, price, and upgrade URL, and performs no
//! workspace mutation") is what we falsify here.

pub mod common;

use common::serial;
use common::{CommandResult, TestProject};
use std::fs;
use std::path::Path;

use omg_lib::core::env::fingerprint::EnvironmentState;

const GATE_MSG: &str = "Feature 'team-sync' requires Team tier";
const PRICING_URL: &str = "https://pyro1121.com/pricing";

/// Craft a valid initialized team workspace (`.omg/team.toml` + status file)
/// without going through the license-gated `team init`.
fn craft_workspace(project: &TestProject) {
    let omg_dir = project.path().join(".omg");
    fs::create_dir_all(&omg_dir).expect("create .omg dir");
    // `remote_url` intentionally omitted: absent means "no remote", which makes
    // push/pull operate purely on local state without any network access.
    fs::write(
        omg_dir.join("team.toml"),
        r#"team_id = "acme/backend"
name = "Acme Backend"
member_id = "probe-user"
auto_sync = true
auto_push = false

[notifications]
on_lock_update = true
on_drift = true
on_member_join = false
"#,
    )
    .expect("write team.toml");
    fs::write(
        omg_dir.join("team-status.json"),
        r#"{
  "format_version": 1,
  "config": {
    "team_id": "acme/backend",
    "name": "Acme Backend",
    "member_id": "probe-user",
    "remote_url": null,
    "auto_sync": true,
    "auto_push": false,
    "notifications": {"on_lock_update": true, "on_drift": true, "on_member_join": false}
  },
  "lock_hash": "",
  "members": [],
  "updated_at": 1700000000
}
"#,
    )
    .expect("write team-status.json");
}

fn workspace_marker_exists(project: &TestProject) -> bool {
    Path::new(&project.path().join(".omg/team.toml")).exists()
}

fn assert_gate_failure(res: &CommandResult, context: &str) {
    res.assert_failure();
    res.assert_stderr_contains(GATE_MSG);
    res.assert_stderr_contains(PRICING_URL);
    // The gate must fire before ANY output beyond the error itself.
    assert!(
        !res.stdout_contains("Team workspace initialized"),
        "{context}: gate bypassed, saw success output:\n{}",
        res.stdout
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// team init
// ═══════════════════════════════════════════════════════════════════════════

/// Contract: `team init` rejects a team ID containing characters outside
/// [A-Za-z0-9/_-], names the offending ID verbatim in the final error, offers
/// the alphanumeric suggestion, and leaves NO workspace marker behind.
#[test]
#[serial]
fn init_rejects_invalid_team_id_and_writes_nothing() {
    let project = TestProject::new();
    let res = project.run(&["team", "init", "bad team!"]);
    res.assert_failure();
    res.assert_stderr_contains("Invalid team ID");
    res.assert_stderr_contains("Error: Invalid team ID: bad team!");
    res.assert_stdout_contains("Team IDs must be alphanumeric with /, -, or _ allowed");
    assert!(
        !workspace_marker_exists(&project),
        "a rejected init must not create .omg/team.toml"
    );
}

/// Contract: a team display name containing a control character is rejected
/// with the exact validation message before any license or filesystem work.
#[test]
#[serial]
fn init_rejects_control_char_team_name() {
    let project = TestProject::new();
    let name_with_newline = "backend\nrm";
    let res = project.run(&["team", "init", "acme/backend", "--name", name_with_newline]);
    res.assert_failure();
    res.assert_stderr_contains("Invalid team name (too long or contains control characters)");
    res.assert_stderr_contains("Error: Invalid team name");
    assert!(
        !workspace_marker_exists(&project),
        "a rejected init must not create .omg/team.toml"
    );
}

/// Contract: a team name longer than 128 bytes is rejected with the same
/// exact validation message.
#[test]
#[serial]
fn init_rejects_overlong_team_name() {
    let project = TestProject::new();
    let long_name = "x".repeat(129);
    let res = project.run(&["team", "init", "acme/backend", "--name", &long_name]);
    res.assert_failure();
    res.assert_stderr_contains("Invalid team name (too long or contains control characters)");
    assert!(
        !workspace_marker_exists(&project),
        "a rejected init must not create .omg/team.toml"
    );
}

/// Contract: without a paid license, `init` fails AFTER input validation and
/// BEFORE creating any workspace state, with an error naming the feature, the
/// required tier, the price, and the upgrade URL.
#[test]
#[serial]
fn init_without_license_names_feature_tier_price_and_upgrade_url() {
    let project = TestProject::new();
    let res = project.run(&["team", "init", "acme/backend", "--name", "Backend"]);
    res.assert_failure();
    res.assert_stderr_contains(GATE_MSG);
    res.assert_stderr_contains("($200/mo)");
    res.assert_stderr_contains(PRICING_URL);
    assert!(
        !workspace_marker_exists(&project),
        "the license gate must precede workspace creation"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// team join
// ═══════════════════════════════════════════════════════════════════════════

/// Contract: `join` refuses plaintext HTTP URLs, tells the user exactly what
/// to do instead, and writes nothing to disk.
#[test]
#[serial]
fn join_rejects_http_url_with_https_remedy() {
    let project = TestProject::new();
    let res = project.run(&["team", "join", "http://example.com/acme/backend"]);
    res.assert_failure();
    res.assert_stderr_contains("Only HTTPS URLs allowed for security");
    res.assert_stderr_contains("Error: Only HTTPS URLs allowed for security");
    res.assert_stdout_contains("Use https:// instead of http://");
    assert!(
        !workspace_marker_exists(&project),
        "a rejected join must not create .omg/team.toml"
    );
}

/// Contract: remote URLs containing control characters fail validation even
/// when they start with https:// .
#[test]
#[serial]
fn join_rejects_control_char_url() {
    let project = TestProject::new();
    let url_with_newline = format!("https://example.com/{}\n/evil", "acme");
    let res = project.run(&["team", "join", &url_with_newline]);
    res.assert_failure();
    res.assert_stderr_contains("Invalid remote URL");
    assert!(
        !workspace_marker_exists(&project),
        "a rejected join must not create .omg/team.toml"
    );
}

/// Contract: remote URLs longer than 1024 bytes fail validation.
#[test]
#[serial]
fn join_rejects_overlong_url() {
    let project = TestProject::new();
    let url = format!("https://example.com/{}", "a".repeat(1100));
    let res = project.run(&["team", "join", &url]);
    res.assert_failure();
    res.assert_stderr_contains("Invalid remote URL");
    assert!(
        !workspace_marker_exists(&project),
        "a rejected join must not create .omg/team.toml"
    );
}

/// Contract: unsupported HTTPS remotes are rejected before the team workspace
/// is initialized, so the following pull cannot be guaranteed to work.
#[test]
#[serial]
fn join_rejects_unsupported_https_remote_before_mutation() {
    let project = TestProject::new();
    let res = project.run(&["team", "join", "https://github.com/acme/backend"]);
    res.assert_failure();
    res.assert_stderr_contains("gist.github.com");
    assert!(
        !workspace_marker_exists(&project),
        "unsupported remotes must not initialize a workspace"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// team status
// ═══════════════════════════════════════════════════════════════════════════

/// Contract: outside a workspace, `status` fails and points at the exact
/// remedy command.
#[test]
#[serial]
fn status_outside_workspace_fails_naming_init_remedy() {
    let project = TestProject::new();
    let res = project.run(&["team", "status"]);
    res.assert_failure();
    res.assert_stderr_contains("Not a team workspace");
    res.assert_stdout_contains("Run 'omg team init <team-id>' first");
}

/// Contract: in a crafted initialized workspace, `status` succeeds and reports
/// the team identity, the empty-lock sentinel "none", and the 1/1 sync ratio
/// produced by registering the configured local member.
#[test]
#[serial]
fn status_in_workspace_reports_identity_empty_lock_and_member_count() {
    let project = TestProject::new();
    craft_workspace(&project);
    let res = project.run(&["team", "status"]);
    res.assert_success();
    res.assert_stdout_contains("[Team Status] 1/1 members in sync");
    res.assert_stdout_contains("Team: Acme Backend (acme/backend)");
    res.assert_stdout_contains("Lock hash: none");
    res.assert_stdout_contains("in sync");
}

// ═══════════════════════════════════════════════════════════════════════════
// team push / pull
// ═══════════════════════════════════════════════════════════════════════════

/// Contract: `push` writes an integrity-valid `omg.lock`, records its hash as
/// the team lock hash in the durable status file, and registers the configured
/// member as in-sync.
#[test]
#[serial]
fn push_writes_valid_lockfile_and_records_lock_hash_in_status() {
    let project = TestProject::new();
    craft_workspace(&project);

    let res = project.run(&["team", "push"]);
    res.assert_success();
    res.assert_stdout_contains("Team lock updated!");

    let lock_path = project.path().join("omg.lock");
    let state =
        EnvironmentState::load(&lock_path).expect("push must leave an integrity-valid omg.lock");
    assert_eq!(
        state.hash.len(),
        64,
        "lockfile hash must be a full SHA256 hex digest"
    );
    assert!(
        state.hash.chars().all(|c| c.is_ascii_hexdigit()),
        "lockfile hash must be hex, got {}",
        state.hash
    );

    let status_raw =
        fs::read_to_string(project.path().join(".omg/team-status.json")).expect("read status");
    let status: serde_json::Value =
        serde_json::from_str(&status_raw).expect("parse team-status.json");
    assert_eq!(
        status["lock_hash"].as_str(),
        Some(state.hash.as_str()),
        "status lock_hash must equal the pushed lockfile hash"
    );
    let members = status["members"].as_array().expect("members array");
    assert!(
        members
            .iter()
            .any(|m| m["id"] == "probe-user" && m["in_sync"] == true),
        "configured member must be recorded as in-sync, got: {members:?}"
    );
}

/// Contract: after a successful push, `pull` (no remote configured) compares
/// purely local state and reports in-sync with a zero exit code.
#[test]
#[serial]
fn pull_after_push_reports_local_sync_success() {
    let project = TestProject::new();
    craft_workspace(&project);

    let push_res = project.run(&["team", "push"]);
    push_res.assert_success();

    let res = project.run(&["team", "pull"]);
    res.assert_success();
    res.assert_stdout_contains("Environment is in sync with team!");
}

/// Contract: when the committed lock differs from the live environment (valid
/// integrity, different content), `pull` exits non-zero, warns about drift on
/// stdout, and names the `omg env check` diagnostic command.
#[test]
#[serial]
fn pull_detects_drift_when_lock_differs_from_environment() {
    let project = TestProject::new();
    craft_workspace(&project);

    let push_res = project.run(&["team", "push"]);
    push_res.assert_success();

    // Rewrite the lock with a different-but-integrity-valid state.
    let lock_path = project.path().join("omg.lock");
    let mut drifted = EnvironmentState::load(&lock_path).expect("load pushed lock");
    drifted
        .runtimes
        .insert("node".to_string(), "99.0.0-drifted".to_string());
    drifted.save(&lock_path).expect("rewrite drifted lock");
    // Sanity: save() must keep the file integrity-valid, otherwise this test
    // would exercise the corruption path instead of the drift path.
    EnvironmentState::load(&lock_path).expect("drifted lock must stay integrity-valid");

    let res = project.run(&["team", "pull"]);
    res.assert_failure();
    res.assert_stdout_contains("Environment drift detected!");
    res.assert_stdout_contains("Run 'omg env check' to see differences");
    res.assert_stderr_contains("Error: Environment drift detected");
}

/// Contract: a lockfile whose stored hash does not match its contents is
/// rejected loudly (integrity error naming the mismatch) rather than being
/// silently treated as in-sync or as drift.
#[test]
#[serial]
fn corrupted_lockfile_fails_pull_loudly_instead_of_reporting_state() {
    let project = TestProject::new();
    craft_workspace(&project);

    let push_res = project.run(&["team", "push"]);
    push_res.assert_success();

    let lock_path = project.path().join("omg.lock");
    let mut tampered = EnvironmentState::load(&lock_path).expect("load pushed lock");
    tampered.hash = "f".repeat(64);
    let tampered_toml = toml::to_string_pretty(&tampered).expect("serialize tampered lock");
    fs::write(&lock_path, tampered_toml).expect("write tampered lock");

    let res = project.run(&["team", "pull"]);
    res.assert_failure();
    res.assert_stderr_contains("Lockfile integrity check failed");
    res.assert_stderr_contains("stored hash does not match contents");
}

/// Contract: pulling with a remote that is not a gist.github.com URL fails
/// with an error naming the unsupported URL and the supported scheme, instead
/// of reporting local-only state as a successful team sync.
#[test]
#[serial]
fn pull_rejects_non_gist_remote_url_instead_of_reporting_fake_sync() {
    let project = TestProject::new();
    craft_workspace(&project);
    let cfg = project.path().join(".omg/team.toml");
    let mut content = fs::read_to_string(&cfg).expect("read team.toml");
    content.insert_str(0, "remote_url = \"https://github.com/acme/backend\"\n");
    fs::write(&cfg, content).expect("add github remote");

    let res = project.run(&["team", "pull"]);
    res.assert_failure();
    res.assert_stderr_contains("Unsupported team remote URL 'https://github.com/acme/backend'");
    res.assert_stderr_contains("pull currently supports only gist.github.com remotes");
}

// ═══════════════════════════════════════════════════════════════════════════
// golden-path templates
// ═══════════════════════════════════════════════════════════════════════════

/// Contract: template names allow only alphanumerics and hyphens; rejection
/// happens BEFORE the license gate and before any config file is written.
#[test]
#[serial]
fn golden_path_create_rejects_invalid_template_name_before_license_gate() {
    let project = TestProject::new();
    let res = project.run(&["team", "golden-path", "create", "bad name!"]);
    res.assert_failure();
    res.assert_stderr_contains("Invalid template name");
    res.assert_stdout_contains("Template names must be alphanumeric with hyphens only");
    res.assert_stderr_contains("Error: Invalid template name (alphanumeric and hyphens only)");
    assert!(
        !project.config_dir.path().join("golden-paths.toml").exists(),
        "rejected create must not write golden-paths.toml"
    );
    // Validation precedes the license gate: the gate message must NOT appear.
    assert!(
        !res.stderr_contains(GATE_MSG),
        "input validation must fire before the license gate"
    );
}

/// Contract: unsafe Node version strings (path-hostile characters) are
/// rejected before the license gate and before any config write.
#[test]
#[serial]
fn golden_path_create_rejects_unsafe_node_version_before_license_gate() {
    let project = TestProject::new();
    let res = project.run(&[
        "team",
        "golden-path",
        "create",
        "my-template",
        "--node",
        "20:~evil",
    ]);
    res.assert_failure();
    res.assert_stderr_contains("Invalid Node version:");
    assert!(
        !project.config_dir.path().join("golden-paths.toml").exists(),
        "rejected create must not write golden-paths.toml"
    );
    assert!(
        !res.stderr_contains(GATE_MSG),
        "input validation must fire before the license gate"
    );
}

/// Contract: package lists containing unsafe package names are rejected
/// before the license gate.
#[test]
#[serial]
fn golden_path_create_rejects_unsafe_package_name_before_license_gate() {
    let project = TestProject::new();
    let res = project.run(&[
        "team",
        "golden-path",
        "create",
        "my-template",
        "--packages",
        "good-pkg,bad;pkg",
    ]);
    res.assert_failure();
    res.assert_stderr_contains("Invalid package name:");
    assert!(
        !project.config_dir.path().join("golden-paths.toml").exists(),
        "rejected create must not write golden-paths.toml"
    );
}

/// Contract: a syntactically valid `golden-path create` still cannot persist
/// anything without the team license — the gate fires and no config exists.
#[test]
#[serial]
fn golden_path_create_valid_input_still_gated_by_license() {
    let project = TestProject::new();
    let res = project.run(&[
        "team",
        "golden-path",
        "create",
        "my-template",
        "--node",
        "20",
    ]);
    assert_gate_failure(&res, "golden-path create");
    assert!(
        !project.config_dir.path().join("golden-paths.toml").exists(),
        "gate must prevent config persistence"
    );
}

/// Contract: `golden-path list` requires the team license.
#[test]
#[serial]
fn golden_path_list_requires_license_gate() {
    let project = TestProject::new();
    let res = project.run(&["team", "golden-path", "list"]);
    assert_gate_failure(&res, "golden-path list");
}

/// Contract: `golden-path delete` requires the team license.
#[test]
#[serial]
fn golden_path_delete_requires_license_gate() {
    let project = TestProject::new();
    let res = project.run(&["team", "golden-path", "delete", "whatever"]);
    assert_gate_failure(&res, "golden-path delete");
}

// ═══════════════════════════════════════════════════════════════════════════
// license gates on the remaining team surface
// ═══════════════════════════════════════════════════════════════════════════

/// Contract: every remaining team subcommand denies Free-tier users with an
/// error naming the `team-sync` feature and the pricing URL.
#[test]
#[serial]
fn team_roles_list_requires_license_gate() {
    let project = TestProject::new();
    let res = project.run(&["team", "roles", "list"]);
    assert_gate_failure(&res, "roles list");
}

#[test]
#[serial]
fn team_members_requires_license_gate() {
    let project = TestProject::new();
    let res = project.run(&["team", "members"]);
    assert_gate_failure(&res, "members");
}

#[test]
#[serial]
fn team_activity_requires_license_gate() {
    let project = TestProject::new();
    let res = project.run(&["team", "activity", "--days", "7"]);
    assert_gate_failure(&res, "activity");
}

#[test]
#[serial]
fn team_dashboard_requires_license_gate() {
    let project = TestProject::new();
    let res = project.run(&["team", "dashboard"]);
    assert_gate_failure(&res, "dashboard");
}

/// Contract: `compliance` is honest about having no local evaluation engine —
/// but only after passing the license gate; Free tier gets the gate error.
#[test]
#[serial]
fn team_compliance_requires_license_gate() {
    let project = TestProject::new();
    let res = project.run(&["team", "compliance"]);
    assert_gate_failure(&res, "compliance");
}

#[test]
#[serial]
fn team_compliance_export_requires_license_gate() {
    let project = TestProject::new();
    let res = project.run(&["team", "compliance", "--export", "/tmp/cov5-export.md"]);
    assert_gate_failure(&res, "compliance export");
}
