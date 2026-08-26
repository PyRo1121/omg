//! Coverage tests for `src/package_managers/alpm_ops.rs`:
//! `open_default_alpm`, `get_update_list` (non-test-mode ALPM path), and
//! `execute_transaction` (fresh-handle path).
//!
//! Every test drives the real library code against an isolated fake pacman
//! root (`AlpmHarness`) selected through the `OMG_PACMAN_ROOT` /
//! `OMG_PACMAN_DB_DIR` / `OMG_PACMAN_CONF` environment overrides. Deliberately
//! NOT setting `OMG_TEST_MODE` forces the production ALPM code paths instead
//! of the pure-Rust `pacman_db` mock parsing.

#![cfg(feature = "arch")]
#![expect(clippy::unwrap_used, clippy::expect_used, clippy::pedantic)]

pub mod common;

#[path = "alpm_harness.rs"]
mod alpm_harness;

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use alpm_harness::{AlpmHarness, HarnessPkg};
use common::serial;
use omg_lib::package_managers::alpm_direct::clear_alpm_cache;
use omg_lib::package_managers::alpm_ops::{
    execute_transaction, get_update_list, open_default_alpm,
};
use tempfile::{TempDir, tempdir};

// ===========================================================================
// Fake pacman environment
// ===========================================================================

/// Add a local-db entry (`{db}/local/{name}-{version}/desc`) so libalpm sees
/// the package as installed inside the harness root.
fn add_local_pkg(root: &Path, name: &str, version: &str) -> std::io::Result<()> {
    let local_dir = root.join("var/lib/pacman/local");
    // libalpm 16 validates this marker eagerly when the local DB is opened;
    // a fresh root without it fails with "database is incorrect version".
    fs::write(local_dir.join("ALPM_DB_VERSION"), "9")?;
    let entry = local_dir.join(format!("{name}-{version}"));
    fs::create_dir_all(&entry)?;
    fs::write(
        entry.join("desc"),
        format!("%NAME%\n{name}\n\n%VERSION%\n{version}\n\n"),
    )
}

/// Build a minimal pacman.conf understood by `PacmanConfig::parse_str`.
fn pacman_conf(options: &[(&str, &str)], repos: &[&str]) -> String {
    let mut content = String::from("[options]\n");
    for (key, value) in options {
        content.push_str(&format!("{key} = {value}\n"));
    }
    // Unreachable server: transactions may plan but must never succeed at
    // commit time, keeping every test offline and deterministic.
    for repo in repos {
        content.push_str(&format!(
            "\n[{repo}]\nServer = file:///omg-cov6-unreachable/$repo/os/$arch\n"
        ));
    }
    content
}

struct FakePacman {
    harness: AlpmHarness,
    _data_dir: TempDir,
    _config_dir: TempDir,
    conf_file: PathBuf,
}

impl FakePacman {
    fn new(conf_content: &str) -> anyhow::Result<Self> {
        let harness = AlpmHarness::new()?;
        let data_dir = tempdir()?;
        let config_dir = tempdir()?;
        let conf_file = config_dir.path().join("pacman.conf");
        fs::write(&conf_file, conf_content)?;
        Ok(Self {
            harness,
            _data_dir: data_dir,
            _config_dir: config_dir,
            conf_file,
        })
    }

    /// Run `f` with the production environment redirected at the fake root.
    ///
    /// `clear_alpm_cache()` before and after prevents stale thread-local ALPM
    /// handles from leaking between tests that point at different roots.
    fn run<R>(&self, f: impl FnOnce() -> R) -> R {
        clear_alpm_cache();
        let vars: Vec<(String, OsString)> = vec![
            ("OMG_TEST_MODE".into(), OsString::new()), // unset: force ALPM paths
            ("OMG_DISABLE_TELEMETRY".into(), "1".into()),
            ("OMG_DISABLE_DAEMON".into(), "1".into()),
            (
                "OMG_DATA_DIR".into(),
                self._data_dir.path().as_os_str().into(),
            ),
            (
                "OMG_CONFIG_DIR".into(),
                self._config_dir.path().as_os_str().into(),
            ),
            (
                "OMG_CACHE_DIR".into(),
                self._config_dir.path().join("cache").as_os_str().into(),
            ),
            (
                "OMG_PACMAN_ROOT".into(),
                self.harness.root().as_os_str().into(),
            ),
            (
                "OMG_PACMAN_DB_DIR".into(),
                self.harness.db_path().as_os_str().into(),
            ),
            ("OMG_PACMAN_CONF".into(), self.conf_file.as_os_str().into()),
        ];
        let vars: Vec<(&str, Option<&OsString>)> =
            vars.iter().map(|(k, v)| (k.as_str(), Some(v))).collect();
        let result = temp_env::with_vars(vars, f);
        clear_alpm_cache();
        result
    }

    fn root(&self) -> &Path {
        self.harness.root()
    }
}

// ===========================================================================
// open_default_alpm
// ===========================================================================

#[test]
#[serial]
fn open_default_alpm_opens_harness_root_and_reads_its_local_db() -> anyhow::Result<()> {
    if !common::TestConfig::default().is_arch() {
        common::report_skip("requires Arch Linux");
        return Ok(());
    }

    let fake = FakePacman::new(&pacman_conf(&[], &["core"]))?;
    add_local_pkg(fake.root(), "cov6-local-alpha", "1.0-1")?;
    fake.harness
        .add_sync_pkg("core", &HarnessPkg::new("cov6-sync-only", "5.5-1"))?;

    fake.run(|| {
        let alpm = open_default_alpm()
            .expect("open_default_alpm must succeed against the harness pacman root");

        // The returned handle must be bound to the overridden root's local DB,
        // proving both root and db-dir come from the resolved paths.
        let pkg = alpm
            .localdb()
            .pkg("cov6-local-alpha")
            .expect("handle must expose the fake root's local database");
        assert_eq!(pkg.version().as_str(), "1.0-1");

        // And the sync DB registered through the same handle sees the harness
        // sync database built by the harness helper.
        alpm.register_syncdb("core", alpm::SigLevel::USE_DEFAULT)
            .expect("registering the harness core repo must work");
        let sync_dbs: Vec<_> = alpm.syncdbs().into_iter().collect();
        let sync_pkg = sync_dbs[0]
            .pkg("cov6-sync-only")
            .expect("sync db content must be visible");
        assert_eq!(sync_pkg.version().as_str(), "5.5-1");
    });
    Ok(())
}

// ===========================================================================
// get_update_list (production ALPM path)
// ===========================================================================

#[test]
#[serial]
fn get_update_list_reports_sync_newer_than_local_with_repo_attribution() -> anyhow::Result<()> {
    if !common::TestConfig::default().is_arch() {
        common::report_skip("requires Arch Linux");
        return Ok(());
    }

    let fake = FakePacman::new(&pacman_conf(&[], &["core"]))?;
    add_local_pkg(fake.root(), "alpha", "1.0-1")?;
    // Installed-only and sync-only packages must not leak into the list.
    add_local_pkg(fake.root(), "solo", "9.0-1")?;
    fake.harness.add_sync_pkgs(
        "core",
        &[
            HarnessPkg::new("alpha", "2.0-1"),
            HarnessPkg::new("remote-only", "1.0-1"),
        ],
    )?;

    fake.run(|| {
        let updates =
            get_update_list().expect("update collection must succeed on the harness root");
        assert_eq!(
            updates.len(),
            1,
            "exactly one update expected, got: {updates:?}"
        );
        let update = &updates[0];
        assert_eq!(update.name, "alpha");
        assert_eq!(update.old_version, "1.0-1");
        assert_eq!(update.new_version, "2.0-1");
        assert_eq!(update.repo, "core");
    });
    Ok(())
}

#[test]
#[serial]
fn get_update_list_honors_ignorepkg_from_pacman_conf() -> anyhow::Result<()> {
    if !common::TestConfig::default().is_arch() {
        common::report_skip("requires Arch Linux");
        return Ok(());
    }

    let conf = pacman_conf(&[("IgnorePkg", "beta")], &["core"]);
    let fake = FakePacman::new(&conf)?;
    add_local_pkg(fake.root(), "alpha", "1.0-1")?;
    add_local_pkg(fake.root(), "beta", "1.0-1")?;
    fake.harness.add_sync_pkgs(
        "core",
        &[
            HarnessPkg::new("alpha", "2.0-1"),
            HarnessPkg::new("beta", "2.0-1"),
        ],
    )?;

    fake.run(|| {
        let updates =
            get_update_list().expect("update collection must succeed on the harness root");
        assert_eq!(
            updates.len(),
            1,
            "ignored package must be filtered out, got: {updates:?}"
        );
        assert_eq!(updates[0].name, "alpha");
    });
    Ok(())
}

#[test]
#[serial]
fn get_update_list_first_registered_repo_wins_for_duplicate_packages() -> anyhow::Result<()> {
    if !common::TestConfig::default().is_arch() {
        common::report_skip("requires Arch Linux");
        return Ok(());
    }

    let fake = FakePacman::new(&pacman_conf(&[], &["core", "extra"]))?;
    add_local_pkg(fake.root(), "gamma", "1.0-1")?;
    fake.harness
        .add_sync_pkg("core", &HarnessPkg::new("gamma", "3.0-1"))?;
    fake.harness
        .add_sync_pkg("extra", &HarnessPkg::new("gamma", "4.0-1"))?;

    fake.run(|| {
        let updates =
            get_update_list().expect("update collection must succeed on the harness root");
        assert_eq!(updates.len(), 1, "got: {updates:?}");
        // core is listed (and therefore registered) first: its version and
        // repo name win over extra's newer duplicate.
        assert_eq!(updates[0].repo, "core");
        assert_eq!(updates[0].new_version, "3.0-1");
    });
    Ok(())
}

// ===========================================================================
// execute_transaction (fresh-handle path)
// ===========================================================================

#[test]
#[serial]
fn execute_transaction_bails_when_pacman_conf_has_no_repositories() -> anyhow::Result<()> {
    if !common::TestConfig::default().is_arch() {
        common::report_skip("requires Arch Linux");
        return Ok(());
    }

    let fake = FakePacman::new(&pacman_conf(&[], &[]))?;

    fake.run(|| {
        let error = execute_transaction(vec!["anything".to_string()], false, false, None)
            .expect_err("transaction must fail when pacman.conf declares no repositories");
        let rendered = error.to_string();
        assert!(
            rendered.contains("pacman configuration contains no repositories"),
            "unexpected error: {rendered}"
        );
    });
    Ok(())
}

#[test]
#[serial]
fn execute_transaction_refuses_to_remove_holdpkg_protected_package() -> anyhow::Result<()> {
    if !common::TestConfig::default().is_arch() {
        common::report_skip("requires Arch Linux");
        return Ok(());
    }

    let conf = pacman_conf(&[("HoldPkg", "glibc systemd")], &["core"]);
    let fake = FakePacman::new(&conf)?;
    fake.harness
        .add_sync_pkg("core", &HarnessPkg::new("bash", "5.2-1"))?;

    fake.run(|| {
        let error = execute_transaction(vec!["glibc".to_string()], true, false, None)
            .expect_err("removing a HoldPkg-protected package must fail");
        let rendered = error.to_string();
        assert!(
            rendered.contains("Package 'glibc' is protected by HoldPkg"),
            "unexpected error: {rendered}"
        );
        assert!(
            rendered.contains("cannot be removed"),
            "error must state the action was refused: {rendered}"
        );
    });
    Ok(())
}

#[test]
#[serial]
fn execute_transaction_names_missing_package_with_recovery_steps() -> anyhow::Result<()> {
    if !common::TestConfig::default().is_arch() {
        common::report_skip("requires Arch Linux");
        return Ok(());
    }

    let fake = FakePacman::new(&pacman_conf(&[], &["core"]))?;
    fake.harness
        .add_sync_pkg("core", &HarnessPkg::new("bash", "5.2-1"))?;

    fake.run(|| {
        let error = execute_transaction(
            vec!["cov6-definitely-missing-pkg".to_string()],
            false,
            false,
            None,
        )
        .expect_err("installing an unknown package must fail");
        let rendered = error.to_string();
        assert!(
            rendered
                .contains("'cov6-definitely-missing-pkg' not found in any configured repository"),
            "error must name the missing package: {rendered}"
        );
        assert!(
            rendered.contains("Run 'omg sync'"),
            "error must carry the recovery remedy: {rendered}"
        );
        assert!(
            rendered.contains("https://archlinux.org/packages/"),
            "error must point at the package lookup page: {rendered}"
        );
    });
    Ok(())
}

#[test]
#[serial]
fn execute_transaction_sysupgrade_on_current_system_succeeds_without_touching_anything()
-> anyhow::Result<()> {
    if !common::TestConfig::default().is_arch() {
        common::report_skip("requires Arch Linux");
        return Ok(());
    }

    let fake = FakePacman::new(&pacman_conf(&[], &["core"]))?;
    add_local_pkg(fake.root(), "uptodate", "2.0-1")?;
    fake.harness
        .add_sync_pkg("core", &HarnessPkg::new("uptodate", "2.0-1"))?;

    fake.run(|| {
        execute_transaction(Vec::new(), false, true, None)
            .expect("sysupgrade with no pending updates must report success (nothing to do)");
    });
    Ok(())
}

#[test]
#[serial]
fn execute_transaction_sysupgrade_with_pending_update_fails_at_commit_without_server()
-> anyhow::Result<()> {
    if !common::TestConfig::default().is_arch() {
        common::report_skip("requires Arch Linux");
        return Ok(());
    }

    let fake = FakePacman::new(&pacman_conf(&[], &["core"]))?;
    add_local_pkg(fake.root(), "stale", "2.0-1")?;
    // Installable metadata (%FILENAME%/%CSIZE%/%ISIZE%) so trans_prepare can
    // resolve the upgrade; commit then fails because the configured server is
    // unreachable and the cached placeholder lacks a required signature.
    fake.harness
        .add_installable_sync_pkg("core", &HarnessPkg::new("stale", "3.0-1"))?;
    // Remove the harness' cached placeholder so commit cannot short-circuit:
    // it must attempt a real download from the unreachable server and fail.
    fs::remove_file(
        fake.root()
            .join("var/cache/pacman/pkg/stale-3.0-1-x86_64.pkg.tar.gz"),
    )?;

    fake.run(|| {
        let error = execute_transaction(Vec::new(), false, true, None)
            .expect_err("committing an upgrade with no reachable server must fail");
        let rendered = error.to_string();
        // Either phase may fail first depending on keyring state: preparation
        // rejects unsigned packages, commit fails on unreachable servers.
        let commit_contract = rendered.contains("Transaction failed to commit");
        let prepare_contract = rendered.contains("preparation failed");
        assert!(
            commit_contract || prepare_contract,
            "failure must surface a transaction contract: {rendered}"
        );
        if commit_contract {
            assert!(
                rendered.contains("Run 'omg cleanup'"),
                "failure must carry its recovery hint: {rendered}"
            );
        }
    });
    Ok(())
}
