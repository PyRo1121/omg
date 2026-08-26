//! cov-3: contract tests for src/cli/commands.rs rollback partitioning and
//! env sync exit codes.
//!
//! Pinned contracts (each falsifiable — see report cov-3.md for the mutation
//! verification log):
//!
//! - `rollback_action` Sync arm: a successful Sync transaction rolls back to
//!   "Nothing to roll back" with exit code 0.
//! - `rollback_action` guard: failed transactions are refused with an explicit
//!   reason, never silently replayed.
//! - `rollback_action` Restore partition: an Update change without a recorded
//!   old version aborts naming that exact package; official-source changes go
//!   down the pacman-cache path, which fails closed naming package+version+dirs
//!   when the cache cannot satisfy it.
//! - `find_cached_arch_package_in` security identity check via the real CLI:
//!   an archive whose embedded .PKGINFO disagrees with the requested name/version
//!   is refused ("refusing to install"), as is an archive without readable
//!   .PKGINFO.
//! - ID handling: unknown-but-valid IDs error with "Transaction ID not found";
//!   malformed IDs are rejected before any lookup.
//! - Destructive-operation gate: rollback of an existing transaction without
//!   --yes in non-interactive mode must fail and name the remedy command.
//! - Install rollback end-to-end (mock backend): prints removal work, exits 0,
//!   records a Remove transaction sourced "rollback" into history.json.
//! - `env sync` input validation exit codes: >255-char or control-character
//!   arguments fail with exit != 0 and the exact message
//!   "Invalid Gist URL or ID", before any network activity.
//!
//! Run:
//!   cargo test --features arch --test coverage_3

#![expect(clippy::unwrap_used, clippy::expect_used)]

pub mod common;

use common::*;

const SYNC_TXN_ID: &str = "11111111aaaa2222bbbb3333cccc4444";
const FAILED_UPDATE_ID: &str = "aaaaaaaa111122223333444455556666";
const NO_OLD_VERSION_ID: &str = "bbbbbbbb111122223333444455556666";
const INSTALL_TXN_ID: &str = "cccccccc111122223333444455556666";
const OFFICIAL_UPDATE_ID: &str = "dddddddd111122223333444455556666";

/// Serialize one transaction matching `crate::core::history::Transaction`'s
/// persisted shape (bare JSON array, jiff RFC3339 timestamps).
fn txn_json(id: &str, kind: &str, success: bool, changes: &str) -> String {
    format!(
        r#"{{
  "id": "{id}",
  "timestamp": "2026-01-15T10:30:00Z",
  "transaction_type": "{kind}",
  "changes": [{changes}],
  "success": {success}
}}"#
    )
}

fn change_json(name: &str, old_version: Option<&str>, source: &str) -> String {
    let old = match old_version {
        Some(v) => format!(r#""{v}""#),
        None => "null".to_string(),
    };
    format!(
        r#"{{
      "name": "{name}",
      "old_version": {old},
      "new_version": "9.9.9",
      "source": "{source}"
    }}"#
    )
}

/// Write a history log containing exactly `entries` into the project's data
/// dir (where HistoryManager::new() resolves via OMG_DATA_DIR).
fn seed_history(project: &TestProject, entries: &[String]) {
    std::fs::write(
        project.data_dir.path().join("history.json"),
        format!("[{}]\n", entries.join(",\n")),
    )
    .unwrap();
}

// ═══════════════════════════════════════════════════════════════════════════
// rollback_action partition logic, exercised through the real binary
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn rollback_sync_transaction_reports_nothing_to_do() {
    let project = TestProject::for_distro("arch");
    seed_history(&project, &[txn_json(SYNC_TXN_ID, "Sync", true, "")]);

    let result = project.run(&["rollback", SYNC_TXN_ID, "--yes"]);

    result.assert_success();
    result.assert_stdout_contains("Nothing to roll back");
    // A database-sync transaction must not spawn any package operation:
    // no removal line may appear.
    assert!(
        !result.stdout.contains("Removing"),
        "sync rollback must not attempt package operations:\n{}",
        result.stdout
    );
}

#[test]
fn rollback_rejects_failed_transaction_naming_reason() {
    let project = TestProject::for_distro("arch");
    seed_history(
        &project,
        &[txn_json(
            FAILED_UPDATE_ID,
            "Update",
            false,
            &change_json("example", Some("1.0-1"), "core"),
        )],
    );

    let result = project.run(&["rollback", FAILED_UPDATE_ID, "--yes"]);

    result.assert_failure();
    result.assert_stderr_contains(
        "Cannot automatically roll back a failed or partially applied transaction",
    );
}

#[test]
fn rollback_update_without_old_version_names_the_package() {
    let project = TestProject::for_distro("arch");
    seed_history(
        &project,
        &[txn_json(
            NO_OLD_VERSION_ID,
            "Update",
            true,
            &change_json("bash", None, "core"),
        )],
    );

    let result = project.run(&["rollback", NO_OLD_VERSION_ID, "--yes"]);

    result.assert_failure();
    result.assert_stderr_contains("does not record the old version of 'bash'");
}

#[test]
fn rollback_unknown_valid_id_fails_with_not_found() {
    let project = TestProject::for_distro("arch");
    seed_history(&project, &[txn_json(SYNC_TXN_ID, "Sync", true, "")]);

    // Well-formed hex prefix that matches nothing in the seeded history.
    let result = project.run(&["rollback", "99999999", "--yes"]);

    result.assert_failure();
    result.assert_stderr_contains("Transaction ID not found");
}

#[test]
fn rollback_malformed_id_is_rejected_before_lookup() {
    let project = TestProject::for_distro("arch");
    seed_history(&project, &[txn_json(SYNC_TXN_ID, "Sync", true, "")]);

    for bad_id in ["zzz-not-hex", "../../history"] {
        let result = project.run(&["rollback", bad_id, "--yes"]);
        result.assert_failure();
        result.assert_stderr_contains("Invalid transaction ID format");
        // The rejection happens at normalization time, not after a lookup
        // against the (existing) seeded entry.
        assert!(
            !result.stderr.contains("Transaction ID not found"),
            "id '{bad_id}' must be rejected by validation, not lookup:\n{}",
            result.stderr
        );
    }
}

#[test]
fn rollback_existing_transaction_without_yes_requires_yes_flag() {
    let project = TestProject::for_distro("arch");
    seed_history(&project, &[txn_json(SYNC_TXN_ID, "Sync", true, "")]);

    // Harness runs detached from a TTY -> non-interactive mode.
    let result = project.run(&["rollback", SYNC_TXN_ID]);

    result.assert_failure();
    result.assert_stderr_contains("--yes flag");
    // The remedy must be copy-pasteable automation advice, not just a complaint.
    result.assert_stderr_contains("omg rollback");
    result.assert_stderr_contains("--yes");
}

#[test]
fn rollback_install_removes_packages_and_records_rollback_history() {
    let project = TestProject::for_distro("arch");
    // Seed mock backend state so the Remove path has something recorded.
    project.mock_install("example", "1.0-1").unwrap();
    seed_history(
        &project,
        &[txn_json(
            INSTALL_TXN_ID,
            "Install",
            true,
            &change_json("example", None, "aur"),
        )],
    );

    let result = project.run(&["rollback", INSTALL_TXN_ID, "--yes"]);

    result.assert_success();
    result.assert_stdout_contains("Removing 1 package(s)");
    result.assert_stdout_contains("Rollback completed successfully");

    // Exact state change: the rollback itself is recorded as a successful
    // Remove transaction whose single change names the package and is
    // sourced "rollback" (not misattributed to a user remove).
    let history = std::fs::read_to_string(project.data_dir.path().join("history.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&history).unwrap();
    let entries = parsed.as_array().expect("history.json must stay an array");
    assert_eq!(
        entries.len(),
        2,
        "rollback must append exactly one entry: {history}"
    );
    let entry = &entries[1];
    assert_eq!(entry["transaction_type"], "Remove");
    assert_eq!(entry["success"], true);
    assert_eq!(entry["changes"][0]["name"], "example");
    assert_eq!(entry["changes"][0]["source"], "rollback");
    assert_eq!(entry["changes"][0]["new_version"], serde_json::Value::Null);

    // The mock backend state must reflect the removal too.
    let mock_state: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(project.data_dir.path().join("mock_state_pacman.json")).unwrap(),
    )
    .unwrap();
    let installed = mock_state["installed"]
        .as_object()
        .expect("mock state must keep its installed map");
    assert!(
        !installed.contains_key("example"),
        "rollback must remove 'example' from installed mock state: {installed:?}"
    );
}

// ═════════════════════════════════════════════════════════════════════
// find_cached_arch_package / _in miss + fail-closed paths through the CLI.
//
// OMG_PACMAN_ROOT points at an empty temp root, so every path here ends at a
// refusal BEFORE ArchPackageManager::install would ever run — no privileged
// operation is reachable from these tests.
//
// NOTE (product bug cov3-bug-1, see report): find_cached_arch_package
// discards inner errors (`if let Ok(path)`), so the specific "refusing to
// install" diagnostics from find_cached_arch_package_in are unreachable via
// the CLI — every cache failure surfaces as the generic "not available in
// configured pacman caches" error. These tests pin that observable behavior:
// an unsatisfiable/mismatched cache must still refuse BEFORE any install is
// attempted, naming package, version and searched dirs.
// ═════════════════════════════════════════════════════════════════════

#[cfg(feature = "arch")]
mod arch_cache_contracts {
    use super::*;
    use flate2::write::GzEncoder;

    fn cache_dir(project: &TestProject) -> std::path::PathBuf {
        let dir = project.pacman_root.path().join("var/cache/pacman/pkg");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Build a real .pkg.tar.gz archive with the given .PKGINFO payload
    /// (or none at all when `pkginfo` is None).
    fn write_archive(path: &std::path::Path, pkginfo: Option<&str>) {
        let enc = GzEncoder::new(
            std::fs::File::create(path).unwrap(),
            flate2::Compression::fast(),
        );
        let mut tar = tar::Builder::new(enc);
        if let Some(pkginfo) = pkginfo {
            let mut header = tar::Header::new_gnu();
            header.set_size(pkginfo.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, ".PKGINFO", pkginfo.as_bytes())
                .unwrap();
        } else {
            let dummy: &[u8] = b"\n";
            let mut header = tar::Header::new_gnu();
            header.set_size(1);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, ".MTREE", dummy).unwrap();
        }
        tar.into_inner().unwrap().finish().unwrap();
    }

    fn seed_official_update_txn(project: &TestProject, id: &str) {
        seed_history(
            project,
            &[txn_json(
                id,
                "Update",
                true,
                &change_json("example", Some("1.0-1"), "core"),
            )],
        );
    }

    #[test]
    fn rollback_official_update_missing_from_cache_names_pkg_version_and_dirs() {
        require_arch!();
        let project = TestProject::for_distro("arch");
        let _empty = cache_dir(&project); // empty cache: read_dir succeeds, zero matches
        seed_official_update_txn(&project, OFFICIAL_UPDATE_ID);

        let result = project.run(&["rollback", OFFICIAL_UPDATE_ID, "--yes"]);

        assert_unsatisfiable_cache_refusal(&result);
    }

    /// Shared contract: when the pacman cache cannot satisfy a restore,
    /// rollback must exit non-zero with the explicit cache-miss error naming
    /// the requested package, its version and the searched directories — and
    /// must NEVER print completion output. Applies equally to an empty cache
    /// and to archives present but rejected by the .PKGINFO identity check.
    fn assert_unsatisfiable_cache_refusal(result: &CommandResult) {
        result.assert_failure();
        let stderr = &result.stderr;
        assert!(
            stderr.contains("is not available in configured pacman caches"),
            "unsatisfiable cache must say so explicitly:\n{stderr}"
        );
        assert!(
            stderr.contains("example") && stderr.contains("1.0-1"),
            "error must name the requested package AND version:\n{stderr}"
        );
        // The redirected cache root must be listed so an operator can see where omg looked.
        assert!(
            stderr.contains("var/cache/pacman/pkg"),
            "cache-miss error must list searched cache dirs:\n{stderr}"
        );
        // And it must stop there — never claim completion.
        assert!(!result.stdout.contains("Rollback completed successfully"));
    }

    #[test]
    fn rollback_refuses_cached_archive_whose_pkginfo_disagrees() {
        require_arch!();
        let project = TestProject::for_distro("arch");
        let dir = cache_dir(&project);
        // Filename promises example 1.0-1; embedded .PKGINFO claims 9.9.
        // The identity check must reject this archive: no install may even be
        // attempted, regardless of how the refusal is worded.
        write_archive(
            &dir.join("example-1.0-1-x86_64.pkg.tar.gz"),
            Some("pkgname = example\npkgver = 9.9\npkgrel = 1\n"),
        );
        seed_official_update_txn(&project, OFFICIAL_UPDATE_ID);

        let result = project.run(&["rollback", OFFICIAL_UPDATE_ID, "--yes"]);

        assert_unsatisfiable_cache_refusal(&result);
    }

    #[test]
    fn rollback_refuses_cached_archive_without_readable_pkginfo() {
        require_arch!();
        let project = TestProject::for_distro("arch");
        let dir = cache_dir(&project);
        // Correct filename, but no .PKGINFO inside: must fail closed.
        write_archive(&dir.join("example-1.0-1-x86_64.pkg.tar.gz"), None);
        seed_official_update_txn(&project, OFFICIAL_UPDATE_ID);

        let result = project.run(&["rollback", OFFICIAL_UPDATE_ID, "--yes"]);

        assert_unsatisfiable_cache_refusal(&result);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// env sync input-validation exit codes (validation fires before networking)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn env_sync_rejects_overlong_gist_argument_with_exact_error() {
    let project = TestProject::for_distro("arch");
    // 256 chars: one past the accepted maximum.
    let too_long: String = "a".repeat(256);
    assert_eq!(too_long.len(), 256);

    let result = project.run(&["env", "sync", &too_long]);

    result.assert_failure();
    assert_ne!(result.exit_code, 101, "must be a clean error, not a panic");
    result.assert_stderr_contains("Invalid Gist URL or ID");
}

#[test]
fn env_sync_rejects_control_characters_with_exact_error() {
    let project = TestProject::for_distro("arch");

    let result = project.run(&["env", "sync", "abc\u{7}def"]);

    result.assert_failure();
    assert_ne!(result.exit_code, 101, "must be a clean error, not a panic");
    result.assert_stderr_contains("Invalid Gist URL or ID");
}
