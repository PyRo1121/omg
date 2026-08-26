//! Contract tests for `src/package_managers/arch.rs` (cov-16).
//!
//! Pins observable contracts of the `ArchPackageManager` `PackageManager`
//! trait implementation — search, info, list_installed, list_explicit,
//! list_updates, get_status, is_installed, and the input-validation /
//! empty-input behavior of install/remove — against a fully isolated ALPM
//! harness root. No system database is read or modified.
//!
//! Run: cargo test --features arch --test coverage_16

#![cfg(feature = "arch")]
#![expect(clippy::unwrap_used, clippy::expect_used)]

pub mod alpm_harness;
pub mod common;

use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};

use alpm_harness::{AlpmHarness, HarnessPkg};
use common::{TestProject, serial, with_test_env};
use omg_lib::core::Package;
use omg_lib::core::PackageSource;
use omg_lib::package_managers::ArchPackageManager;
use omg_lib::package_managers::PackageManager;

// ═══════════════════════════════════════════════════════════════════════════════
// Fixture: one shared, fully isolated pacman root per test-binary process.
//
// Every test redirects OMG at this root via OMG_PACMAN_ROOT / OMG_PACMAN_CONF /
// OMG_CACHE_DIR, so all queries hit only the packages seeded below.
//
// Seeded state (repo `cov16main`):
//   sync-only : cov16-zebra-utils 1.2.3-1   (never installed locally)
//   explicit  : cov16-explicit   3.3.3-1    (installed explicitly, no update)
//   upgrade   : cov16-upgrade-me local 1.0.1-1 / sync 2.0.1-1 (one update)
//   orphan    : cov16-deplib     1.0.0-1    (dependency reason, required by nothing)
// ═══════════════════════════════════════════════════════════════════════════════

const REPO: &str = "cov16main";

/// Present ONLY in the sync db — never installed locally.
const SYNC_ONLY: &str = "cov16-zebra-utils";
/// Installed explicitly AND present in sync at the SAME version (no update).
const EXPLICIT: &str = "cov16-explicit";
/// Installed explicitly at 1.0.1-1 while sync offers 2.0.1-1 (one pending update).
const UPGRADE_ME: &str = "cov16-upgrade-me";
/// Local-only dependency (reason 1) required by nothing — the canonical orphan.
const DEP_ORPHAN: &str = "cov16-deplib";

const SYNC_ONLY_VER: &str = "1.2.3-1";
const SYNC_ONLY_DESC: &str = "Zebra daemon for omg coverage tests";
const EXPLICIT_VER: &str = "3.3.3-1";
const EXPLICIT_LOCAL_DESC: &str = "Explicitly installed local seed for omg coverage";
const UPGRADE_OLD_VER: &str = "1.0.1-1";
const UPGRADE_NEW_VER: &str = "2.0.1-1";
const DEP_ORPHAN_VER: &str = "1.0.0-1";

struct Fixture {
    root: PathBuf,
    conf: PathBuf,
    cache: PathBuf,
}

fn fixture() -> &'static Fixture {
    static FIXTURE: std::sync::OnceLock<Fixture> = std::sync::OnceLock::new();
    FIXTURE.get_or_init(build_fixture)
}

fn build_fixture() -> Fixture {
    let harness = AlpmHarness::new().expect("failed to build ALPM harness");
    let root = harness.root().to_path_buf();

    // A pacman.conf listing ONLY our synthetic repository, so the ALPM handle
    // created by omg registers cov16main and nothing else.
    let conf = root.join("etc/pacman.conf");
    fs::create_dir_all(conf.parent().unwrap()).unwrap();
    fs::write(
        &conf,
        format!(
            "[options]\nHoldPkg = pacman glibc\nArchitecture = x86_64\n\n[{REPO}]\nServer = file:///nonexistent-cov16\n"
        ),
    )
    .expect("failed to write fixture pacman.conf");

    // Sync database: three packages in one repo (add_sync_pkgs rebuilds the
    // whole .db per call, so all entries must go in together).
    let mut zebra = HarnessPkg::new(SYNC_ONLY, SYNC_ONLY_VER);
    zebra.desc = SYNC_ONLY_DESC.to_string();
    let mut upgrade = HarnessPkg::new(UPGRADE_ME, UPGRADE_NEW_VER);
    upgrade.desc = "Upgrade candidate for omg coverage tests".to_string();
    let mut explicit = HarnessPkg::new(EXPLICIT, EXPLICIT_VER);
    explicit.desc = "Explicitly installed for omg coverage tests".to_string();
    harness
        .add_sync_pkgs(REPO, &[zebra, upgrade, explicit])
        .expect("failed to seed sync db");

    // Local database: two explicit installs, one dependency orphan.
    seed_local_pkg(&root, EXPLICIT, EXPLICIT_VER, EXPLICIT_LOCAL_DESC, "0");
    seed_local_pkg(
        &root,
        UPGRADE_ME,
        UPGRADE_OLD_VER,
        "Upgrade me locally",
        "0",
    );
    seed_local_pkg(
        &root,
        DEP_ORPHAN,
        DEP_ORPHAN_VER,
        "Orphaned dependency",
        "1",
    );

    // The harness owns its TempDir; forgetting it keeps every seeded file
    // alive for the whole test process instead of deleting them now.
    std::mem::forget(harness);

    Fixture {
        cache: root.join("omg-cache"),
        conf,
        root,
    }
}

/// Write a local-db package entry the way pacman does:
/// `<local>/<name>-<version>/desc`.
fn seed_local_pkg(root: &Path, name: &str, version: &str, desc: &str, reason: &str) {
    let dir = root
        .join("var/lib/pacman/local")
        .join(format!("{name}-{version}"));
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("desc"),
        format!(
            "%NAME%\n{name}\n\n%VERSION%\n{version}\n\n%BASE%\n{name}\n\n%DESC%\n{desc}\n\n%ARCH%\nx86_64\n\n%INSTALLDATE%\n1700000000\n\n%REASON%\n{reason}\n"
        ),
    )
    .unwrap();
}

/// Run `f` with every OMG path variable pointed at the isolated fixture.
fn with_fixture_env<T>(f: impl FnOnce() -> T) -> T {
    let fx = fixture();
    with_test_env(
        &[
            ("OMG_PACMAN_ROOT", fx.root.to_str().unwrap()),
            ("OMG_PACMAN_CONF", fx.conf.to_str().unwrap()),
            ("OMG_CACHE_DIR", fx.cache.to_str().unwrap()),
        ],
        f,
    )
}

/// Execute an async PackageManager method on a dedicated single-thread runtime.
fn block_on<F: Future>(fut: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut)
}

fn with_pm<T>(f: impl FnOnce(ArchPackageManager) -> T) -> T {
    with_fixture_env(|| f(ArchPackageManager::new()))
}

/// Tests that drive live ALPM fail on hosts without a pacman database at
/// the fixture root. Callers should `report_skip` when this returns false.
fn alpm_live() -> bool {
    std::path::Path::new(&std::env::var("OMG_PACMAN_ROOT").unwrap_or_else(|_| "/".to_string()))
        .join("var/lib/pacman/local")
        .exists()
}

fn find<'a>(packages: &'a [Package], name: &str) -> &'a Package {
    packages
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| panic!("package '{name}' not found in results: {packages:?}"))
}

// ═══════════════════════════════════════════════════════════════════════════════
// Contracts
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn manager_name_is_pacman() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    assert_eq!(with_pm(|pm| pm.name()), "pacman");
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn search_finds_uninstalled_package_with_exact_fields() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let results = with_pm(|pm| block_on(pm.search(SYNC_ONLY))).unwrap();

    assert_eq!(
        results.len(),
        1,
        "exactly one package must match '{SYNC_ONLY}', got: {results:?}"
    );
    let pkg = &results[0];
    assert_eq!(pkg.name, SYNC_ONLY);
    assert_eq!(pkg.version.to_string(), SYNC_ONLY_VER);
    assert_eq!(pkg.description, SYNC_ONLY_DESC);
    assert_eq!(pkg.source, PackageSource::Official);
    assert!(
        !pkg.installed,
        "sync-only package must not be marked installed"
    );
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn search_marks_locally_installed_packages() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let results = with_pm(|pm| block_on(pm.search(EXPLICIT))).unwrap();

    assert_eq!(
        results.len(),
        1,
        "exactly one package must match '{EXPLICIT}', got: {results:?}"
    );
    assert_eq!(results[0].name, EXPLICIT);
    assert_eq!(results[0].version.to_string(), EXPLICIT_VER);
    assert!(
        results[0].installed,
        "package present in the local db must be reported installed"
    );
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn search_matches_descriptions_case_insensitively() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    // 'OMG COVERAGE' appears only in SYNC_ONLY's %DESC%, in mixed case there.
    let results = with_pm(|pm| block_on(pm.search("OMG COVERAGE"))).unwrap();

    assert_eq!(
        results.len(),
        1,
        "description match must be unique, got: {results:?}"
    );
    assert_eq!(results[0].name, SYNC_ONLY);
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn search_prefix_returns_exactly_the_seeded_set() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let results = with_pm(|pm| block_on(pm.search("cov16"))).unwrap();

    let mut names: Vec<&str> = results.iter().map(|p| p.name.as_str()).collect();
    names.sort_unstable();
    let mut expected = vec![SYNC_ONLY, EXPLICIT, UPGRADE_ME];
    expected.sort_unstable();
    assert_eq!(names, expected, "search must return exactly the sync seeds");

    // Cross-check installed flags per package: EXPLICIT and UPGRADE_ME are
    // local, SYNC_ONLY is not.
    assert!(find(&results, EXPLICIT).installed);
    assert!(find(&results, UPGRADE_ME).installed);
    assert!(!find(&results, SYNC_ONLY).installed);

    for pkg in &results {
        assert_eq!(
            pkg.source,
            PackageSource::Official,
            "ALPM search results must always map to PackageSource::Official"
        );
    }
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn search_without_matches_returns_empty_vec() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let results = with_pm(|pm| block_on(pm.search("zzqq-no-such-package-ever"))).unwrap();
    assert!(
        results.is_empty(),
        "no-match search must return an empty vector, got: {results:?}"
    );
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn info_local_package_returns_exact_details() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let info = with_pm(|pm| block_on(pm.info(EXPLICIT)))
        .unwrap()
        .expect("info on a locally installed package must return Some");

    assert_eq!(info.name, EXPLICIT);
    assert_eq!(info.version.to_string(), EXPLICIT_VER);
    assert_eq!(
        info.description, EXPLICIT_LOCAL_DESC,
        "info must prefer the LOCAL desc for installed packages"
    );
    assert!(info.installed);
    assert_eq!(info.source, PackageSource::Official);
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn info_falls_back_to_sync_db_for_uninstalled_packages() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let info = with_pm(|pm| block_on(pm.info(SYNC_ONLY)))
        .unwrap()
        .expect("info on a sync-only package must return Some");

    assert_eq!(info.name, SYNC_ONLY);
    assert_eq!(info.version.to_string(), SYNC_ONLY_VER);
    assert_eq!(info.description, SYNC_ONLY_DESC);
    assert!(!info.installed);
    assert_eq!(info.source, PackageSource::Official);
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn info_unknown_package_is_ok_none() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let info = with_pm(|pm| block_on(pm.info("cov16-does-not-exist"))).unwrap();
    assert!(
        info.is_none(),
        "unknown package must yield Ok(None), got: {info:?}"
    );
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn info_rejects_shell_metachar_names_with_named_error() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let err = with_pm(|pm| block_on(pm.info("pkg; rm -rf /")))
        .expect_err("info must reject injection-style names");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("Invalid character ';'"),
        "error must name the offending character, got: {msg}"
    );
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn list_installed_returns_exactly_the_local_seeds() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let installed = with_pm(|pm| block_on(pm.list_installed())).unwrap();

    assert_eq!(
        installed.len(),
        3,
        "the isolated local db has exactly 3 packages, got: {installed:?}"
    );
    assert!(installed.iter().all(|p| p.installed));
    for (name, version) in [
        (EXPLICIT, EXPLICIT_VER),
        (UPGRADE_ME, UPGRADE_OLD_VER),
        (DEP_ORPHAN, DEP_ORPHAN_VER),
    ] {
        let pkg = find(&installed, name);
        assert_eq!(pkg.version.to_string(), version, "{name} version mismatch");
    }
    assert!(
        !installed.iter().any(|p| p.name == SYNC_ONLY),
        "sync-only package must not appear in list_installed"
    );
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn list_explicit_excludes_dependency_reason_packages() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let mut explicit = with_pm(|pm| block_on(pm.list_explicit())).unwrap();
    explicit.sort();

    let mut expected = vec![EXPLICIT.to_string(), UPGRADE_ME.to_string()];
    expected.sort();
    assert_eq!(
        explicit, expected,
        "explicit list must contain exactly the two reason-0 packages (orphan dep excluded)"
    );
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn is_installed_reflects_local_db_state() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let yes = with_pm(|pm| block_on(pm.is_installed(EXPLICIT))).unwrap();
    let sync_only = with_pm(|pm| block_on(pm.is_installed(SYNC_ONLY))).unwrap();
    let missing = with_pm(|pm| block_on(pm.is_installed("cov16-not-there"))).unwrap();

    assert!(yes, "{EXPLICIT} is in the local db");
    assert!(!sync_only, "{SYNC_ONLY} is sync-only");
    assert!(!missing, "unknown package is not installed");
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn get_status_fast_counts_match_seeded_database() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let (total, explicit, orphans, updates) = with_pm(|pm| block_on(pm.get_status(true))).unwrap();

    assert_eq!(total, 3, "total installed count");
    assert_eq!(explicit, 2, "two explicit installs");
    assert_eq!(orphans, 1, "{DEP_ORPHAN} is the sole unrequired dependency");
    assert_eq!(updates, 1, "exactly one pending version bump");
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn list_updates_reports_the_exact_pending_update() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let updates = with_pm(|pm| block_on(pm.list_updates())).unwrap();

    assert_eq!(
        updates.len(),
        1,
        "exactly one update is pending, got: {updates:?}"
    );
    let update = &updates[0];
    assert_eq!(update.name, UPGRADE_ME);
    assert_eq!(update.old_version, UPGRADE_OLD_VER);
    assert_eq!(update.new_version, UPGRADE_NEW_VER);
    assert_eq!(update.repo, REPO);
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn install_and_remove_of_empty_input_are_successful_noops() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    // TestProject only owns isolated OMG_DATA/CONFIG temp dirs here; the
    // no-op calls never spawn a privileged child, so no pacman state matters.
    let project = TestProject::new();

    let both_ok = with_test_env(
        &[
            ("OMG_DATA_DIR", project.data_dir.path().to_str().unwrap()),
            (
                "OMG_CACHE_DIR",
                project.data_dir.path().join("cache").to_str().unwrap(),
            ),
        ],
        || {
            let removed =
                with_pm(|pm| block_on(pm.remove(Vec::<String>::new().as_slice()))).is_ok();
            let installed =
                with_pm(|pm| block_on(pm.install(Vec::<String>::new().as_slice()))).is_ok();
            removed && installed
        },
    );
    assert!(
        both_ok,
        "install/remove with zero packages must succeed as a no-op without elevation"
    );
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn install_rejects_invalid_name_before_any_privileged_operation() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let err = with_pm(|pm| block_on(pm.install(&["pkg; rm -rf /".to_string()])))
        .expect_err("install must reject shell-metachar package names");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("Invalid character ';'"),
        "error must come from name validation naming the character, got: {msg}"
    );
}

#[test]
#[ignore = "requires live ALPM database"]
#[serial]
fn remove_rejects_option_injection_names_with_named_error() {
    if !alpm_live() {
        common::report_skip("live ALPM database required");
        return;
    }
    let err = with_pm(|pm| block_on(pm.remove(&["-evil-flag".to_string()])))
        .expect_err("remove must reject names starting with '-'");
    let msg = format!("{err:#}");
    assert!(
        msg.contains("cannot start with '-'") && msg.contains("option injection"),
        "error must name option-injection protection, got: {msg}"
    );
}
