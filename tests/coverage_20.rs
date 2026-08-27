//! Contract tests for `src/hooks/mod.rs` (cov-20).
//!
//! Pins observable contracts of the shell-hook PATH machinery:
//! - `build_path_additions` runtime branches (node / python / go / ruby /
//!   java / bun / rust / unsupported) against a seeded `OMG_DATA_DIR`.
//! - The `nvm_node_bin` traversal guard: hostile multi-component pins are
//!   rejected by version validation, symlink escapes are rejected by the
//!   canonical-path containment check, and aliases resolve inside the nvm
//!   tree only.
//! - POSIX/fish quoting end-to-end through the real `omg hook-env` binary.
//!
//! Every assertion is falsifiable: mutating the protected product code must
//! fail these tests (each test below was verified against a targeted
//! mutation during development; see cov-20.md).

pub mod common;

use common::*;
use omg_lib::hooks::{build_path_additions, detect_versions, hook_env, print_hook};
use std::collections::HashMap;
use std::path::PathBuf;

/// Build a runtime→version map from pairs.
fn pin_map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(r, v)| (r.to_string(), v.to_string()))
        .collect()
}

/// `<data>/versions/node/<version>/bin`
fn node_bin(data: &std::path::Path, version: &str) -> PathBuf {
    data.join("versions/node").join(version).join("bin")
}

// ══════════════════════════════════════════════════════════════════════════
// Native resolution branches of build_path_additions
// ══════════════════════════════════════════════════════════════════════════

/// Contract: a node pin whose exact version directory exists under
/// `<data>/versions/node/<v>/bin` resolves to exactly that directory.
#[test]
#[serial]
fn node_pin_resolves_to_exact_installed_bin_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path();
    fs_extra_or_std::create_dir_all(&node_bin(data, "20.11.1"));
    with_test_env(
        &[
            ("OMG_DATA_DIR", data.to_str().unwrap()),
            ("NVM_DIR", "/nonexistent-nvm-for-tests"),
        ],
        || {
            let additions =
                build_path_additions(&pin_map(&[("node", "20.11.1")])).expect("resolution");
            assert_eq!(
                additions,
                vec![node_bin(data, "20.11.1").display().to_string()],
                "exact node bin dir must be returned"
            );
        },
    );
}

/// Contract: a `v`-prefixed node pin normalizes away the prefix before the
/// filesystem lookup (`v20.11.0` finds `versions/node/20.11.0/bin`).
#[test]
#[serial]
fn v_prefixed_node_pin_normalizes_before_lookup() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path();
    fs_extra_or_std::create_dir_all(&node_bin(data, "20.11.0"));
    with_test_env(
        &[
            ("OMG_DATA_DIR", data.to_str().unwrap()),
            ("NVM_DIR", "/nonexistent-nvm-for-tests"),
        ],
        || {
            let additions =
                build_path_additions(&pin_map(&[("node", "v20.11.0")])).expect("resolution");
            assert_eq!(
                additions,
                vec![node_bin(data, "20.11.0").display().to_string()],
                "v-prefixed pin must resolve like the bare version"
            );
        },
    );
}

/// Contract: an npm-style requirement (`^20`) resolves to the *highest*
/// installed matching version, never a lower one or an unrelated major.
#[test]
#[serial]
fn caret_requirement_resolves_highest_matching_installed_version() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path();
    for v in ["18.19.0", "20.11.1", "20.12.0"] {
        fs_extra_or_std::create_dir_all(&node_bin(data, v));
    }
    with_test_env(
        &[
            ("OMG_DATA_DIR", data.to_str().unwrap()),
            ("NVM_DIR", "/nonexistent-nvm-for-tests"),
        ],
        || {
            let additions = build_path_additions(&pin_map(&[("node", "^20")])).expect("resolution");
            assert_eq!(
                additions,
                vec![node_bin(data, "20.12.0").display().to_string()],
                "^20 must pick the highest installed 20.x"
            );
        },
    );
}

/// Contract: runtimes without a native branch (`deno`, …) never contribute a
/// PATH entry, even when a matching-looking directory exists on disk.
#[test]
#[serial]
fn unsupported_runtime_pins_never_reach_path() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path();
    // A decoy directory that a naive implementation might return.
    fs_extra_or_std::create_dir_all(&data.join("versions/deno/1.40.0/bin"));
    with_test_env(
        &[
            ("OMG_DATA_DIR", data.to_str().unwrap()),
            ("NVM_DIR", "/nonexistent-nvm-for-tests"),
        ],
        || {
            let additions =
                build_path_additions(&pin_map(&[("deno", "1.40.0"), ("zig", "0.11.0")]))
                    .expect("resolution");
            assert!(
                additions.is_empty(),
                "unsupported runtimes must not reach PATH, got {additions:?}"
            );
        },
    );
}

/// Contract: every validated-runtime branch (python/go/ruby/java) resolves a
/// well-formed existing pin to exactly `<data>/versions/<rt>/<v>/bin`.
#[test]
#[serial]
fn validated_runtime_pins_resolve_for_all_native_runtimes() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path();
    with_test_env(
        &[
            ("OMG_DATA_DIR", data.to_str().unwrap()),
            ("NVM_DIR", "/nonexistent-nvm-for-tests"),
        ],
        || {
            for (runtime, version) in [
                ("python", "3.12.1"),
                ("go", "1.22.0"),
                ("ruby", "3.3.1"),
                ("java", "21.0.2"),
            ] {
                let expected = data
                    .join("versions")
                    .join(runtime)
                    .join(version)
                    .join("bin");
                fs_extra_or_std::create_dir_all(&expected);
                let additions =
                    build_path_additions(&pin_map(&[(runtime, version)])).expect("resolution");
                assert_eq!(
                    additions,
                    vec![expected.display().to_string()],
                    "{runtime} pin must resolve to its exact bin dir"
                );
            }
        },
    );
}

/// Contract: a rust channel pin resolves to
/// `<data>/versions/rust/<channel>-<host-triple>/bin` (no date segment).
#[test]
#[serial]
fn rust_channel_pin_resolves_to_toolchain_bin_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path();
    let host = match (std::env::consts::ARCH, std::env::consts::OS) {
        ("x86_64", "linux") => "x86_64-unknown-linux-gnu",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("aarch64", "macos") => "aarch64-apple-darwin",
        _ => panic!("unsupported test platform"),
    };
    let expected = data
        .join("versions/rust")
        .join(format!("stable-{host}"))
        .join("bin");
    fs_extra_or_std::create_dir_all(&expected);
    with_test_env(
        &[
            ("OMG_DATA_DIR", data.to_str().unwrap()),
            ("NVM_DIR", "/nonexistent-nvm-for-tests"),
        ],
        || {
            let additions =
                build_path_additions(&pin_map(&[("rust", "stable")])).expect("resolution");
            assert_eq!(
                additions,
                vec![expected.display().to_string()],
                "rust channel pin must resolve to the toolchain bin dir"
            );
        },
    );
}

/// Contract: bun pins resolve to the *version root*
/// `<data>/versions/bun/<v>` (bun keeps binaries at the top level — no
/// `/bin` suffix may be appended).
#[test]
#[serial]
fn bun_pin_resolves_to_version_root_without_bin_suffix() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path();
    let expected = data.join("versions/bun/1.1.4");
    fs_extra_or_std::create_dir_all(&expected);
    with_test_env(
        &[
            ("OMG_DATA_DIR", data.to_str().unwrap()),
            ("NVM_DIR", "/nonexistent-nvm-for-tests"),
        ],
        || {
            let additions =
                build_path_additions(&pin_map(&[("bun", "1.1.4")])).expect("resolution");
            assert_eq!(
                additions,
                vec![expected.display().to_string()],
                "bun must resolve to the version root dir"
            );
            assert!(
                !additions[0].ends_with("/bin"),
                "bun path must not gain a /bin suffix"
            );
        },
    );
}

// ══════════════════════════════════════════════════════════════════════════
// nvm_node_bin: alias fallback and traversal guards
// ══════════════════════════════════════════════════════════════════════════

/// Contract: when no native install matches, a non-numeric pin falls back to
/// `$NVM_DIR`: `alias/default` is resolved to a concrete version and the
/// nvm-managed bin dir is placed on PATH.
#[test]
#[serial]
fn nvm_alias_pin_falls_back_to_nvm_managed_install() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path();
    let nvm = tmp.path().join("nvm-home");
    fs_extra_or_std::create_dir_all(&nvm.join("alias"));
    std::fs::write(nvm.join("alias/default"), "v20.10.0\n").unwrap();
    let nvm_bin = nvm.join("versions/node/v20.10.0/bin");
    fs_extra_or_std::create_dir_all(&nvm_bin);
    with_test_env(
        &[
            ("OMG_DATA_DIR", data.to_str().unwrap()),
            ("NVM_DIR", nvm.to_str().unwrap()),
        ],
        || {
            let additions =
                build_path_additions(&pin_map(&[("node", "default")])).expect("resolution");
            assert_eq!(
                additions,
                vec![nvm_bin.display().to_string()],
                "`default` alias must resolve through $NVM_DIR"
            );
        },
    );
}

/// Contract (audit sec14 F1, validator layer): a repo-supplied pin containing
/// parent-directory components is refused outright, even though the escape
/// target really exists inside the nvm tree — i.e. if the validation guard
/// were removed, this test would see the escaped dir placed on PATH.
#[test]
#[serial]
fn nvm_hostile_multicomponent_pin_refused_by_validator() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path();
    let nvm = tmp.path().join("nvm-home");
    // Make the unguarded traversal fully resolvable: intermediate dir exists
    // and the final target is a real dir inside the nvm tree, so ONLY the
    // validator stands between the pin and PATH.
    fs_extra_or_std::create_dir_all(&nvm.join("versions/node/v20.11.1"));
    let inner_target = nvm.join("versions/node/v20.12.0/bin");
    fs_extra_or_std::create_dir_all(&inner_target);
    with_test_env(
        &[
            ("OMG_DATA_DIR", data.to_str().unwrap()),
            ("NVM_DIR", nvm.to_str().unwrap()),
        ],
        || {
            let additions = build_path_additions(&pin_map(&[("node", "20.11.1/../20.12.0")]))
                .expect("resolution");
            assert!(
                additions.is_empty(),
                "traversal pin must be rejected even when the target exists, got {additions:?}"
            );
        },
    );
}

/// Contract (audit sec14 F1, containment layer): a valid-version nvm entry
/// that is actually a symlink pointing outside `$NVM_DIR/versions/node` must
/// be refused by the canonical-path containment check.
#[test]
#[serial]
fn nvm_symlink_escape_refused_by_containment_check() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path();
    let nvm = tmp.path().join("nvm-home");
    let outside = tmp.path().join("outside-tree");
    fs_extra_or_std::create_dir_all(&outside.join("bin"));
    fs_extra_or_std::create_dir_all(&nvm.join("versions/node"));
    #[cfg(unix)]
    std::os::unix::fs::symlink(&outside, nvm.join("versions/node/v20.11.1")).unwrap();
    with_test_env(
        &[
            ("OMG_DATA_DIR", data.to_str().unwrap()),
            ("NVM_DIR", nvm.to_str().unwrap()),
        ],
        || {
            let additions =
                build_path_additions(&pin_map(&[("node", "20.11.1")])).expect("resolution");
            assert!(
                additions.is_empty(),
                "symlink escaping the nvm tree must be refused, got {additions:?}"
            );
        },
    );
}

// ══════════════════════════════════════════════════════════════════════════
// Version-file discovery precedence (detect_versions)
// ══════════════════════════════════════════════════════════════════════════

/// Contract: the nearest directory's version pin wins — a runtime pinned in
/// a subdirectory must not be overridden by a different pin in any parent
/// directory, while discovery still walks upward when nothing nearer pins.
#[test]
fn nearest_version_file_wins_over_parent_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let project = tmp.path();
    std::fs::write(project.join(".nvmrc"), "22.0.0\n").unwrap();
    let sub = project.join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join(".tool-versions"), "node 18.0.0\n").unwrap();

    let near = detect_versions(&sub).expect("detection");
    assert_eq!(
        near.get("node"),
        Some(&"18.0.0".to_string()),
        "nearest pin must win; parent-directory files must not override"
    );

    // Upward traversal: with no local pin, the parent's pin is found.
    std::fs::remove_file(sub.join(".tool-versions")).unwrap();
    let inherited = detect_versions(&sub).expect("detection");
    assert_eq!(
        inherited.get("node"),
        Some(&"22.0.0".to_string()),
        "discovery must walk up to parent directories"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// Shell emission (end-to-end through the real binary)
// ══════════════════════════════════════════════════════════════════════════

/// Contract: `omg hook-env -s zsh` emits exactly one `export PATH=` line in
/// which the data-dir path is POSIX single-quoted, with embedded apostrophes
/// rendered as the `'\''` escape sequence and every other byte verbatim.
#[test]
fn hook_env_zsh_quotes_apostrophe_data_dir_posix_style() {
    let tmp = tempfile::tempdir().unwrap();
    // Plant an apostrophe in the resolved path: only correct quoting survives.
    let data = tmp.path().join("omg's da'ta");
    let bin = data.join("versions/node/20.11.1/bin");
    fs_extra_or_std::create_dir_all(&bin);
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join(".nvmrc"), "20.11.1").unwrap();

    let result = run_omg_with_options(
        &["hook-env", "-s", "zsh"],
        Some(project.path()),
        &[("OMG_DATA_DIR", data.to_str().unwrap())],
    );
    result.assert_success();

    let stdout = &result.stdout;
    let inner = stdout
        .strip_prefix("export PATH='")
        .and_then(|rest| rest.strip_suffix("':\"$PATH\"\n"))
        .map(std::string::ToString::to_string);
    let Some(inner) = inner else {
        panic!("stdout must be exactly one quoted export line, got {stdout:?}");
    };
    // Round trip: undoing the '\'' escape reproduces the real path verbatim…
    assert_eq!(
        inner.replace("'\\''", "'"),
        bin.display().to_string(),
        "quoted word must round-trip to the raw path"
    );
    // …and the planted apostrophes were emitted as '\'' escapes.
    assert!(
        inner.contains("'\\''"),
        "embedded apostrophe must be escaped as '\\'', got {inner:?}"
    );
}

/// Contract: `omg hook-env -s fish` emits one `fish_add_path -g '<word>'`
/// line per addition, fish-style quoting (`'` becomes `\'`), everything else
/// verbatim.
#[test]
fn hook_env_fish_emits_fish_quoted_add_path() {
    let tmp = tempfile::tempdir().unwrap();
    let data = tmp.path().join("omg's data");
    let bin = data.join("versions/node/20.11.1/bin");
    fs_extra_or_std::create_dir_all(&bin);
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join(".nvmrc"), "20.11.1").unwrap();

    let result = run_omg_with_options(
        &["hook-env", "-s", "fish"],
        Some(project.path()),
        &[("OMG_DATA_DIR", data.to_str().unwrap())],
    );
    result.assert_success();

    let lines: Vec<&str> = result.stdout.lines().collect();
    assert_eq!(lines.len(), 1, "exactly one fish_add_path line expected");
    let Some(inner) = lines[0]
        .strip_prefix("fish_add_path -g '")
        .and_then(|rest| rest.strip_suffix('\''))
    else {
        panic!("line must be fish_add_path -g '<word>', got {:?}", lines[0]);
    };
    assert_eq!(
        inner.replace("\\'", "'"),
        bin.display().to_string(),
        "fish-quoted word must round-trip to the raw path"
    );
    assert!(inner.contains("\\'"), "embedded apostrophe must become \\'");
}

/// Contract: with a version file present but nothing resolvable installed,
/// hook-env succeeds and prints NOTHING (an empty output must never mutate
/// the caller's PATH).
#[test]
fn hook_env_unresolvable_pin_prints_nothing() {
    let project = TestProject::new();
    project.create_file(".nvmrc", "99.99.99");

    let result = project.run(&["hook-env", "-s", "zsh"]);
    result.assert_success();
    assert_eq!(
        result.stdout, "",
        "unresolvable pin must produce no PATH modification"
    );
}

/// Contract: tracing diagnostics use stderr, leaving stdout safe for command
/// substitution and other machine-readable consumers.
#[test]
fn tracing_diagnostics_do_not_contaminate_stdout() {
    let config = tempfile::tempdir().unwrap();
    std::fs::write(config.path().join("config.toml"), "[general]\n").unwrap();

    let result = run_omg_with_options(
        &["config", "get", "telemetry.enabled"],
        None,
        &[
            ("OMG_CONFIG_DIR", config.path().to_str().unwrap()),
            ("RUST_LOG", "warn"),
        ],
    );
    result.assert_success();
    result.assert_stderr_contains("config section 'general' is deprecated");
    assert_eq!(
        result.stdout, "true\n",
        "diagnostics must never be mixed into machine-readable stdout"
    );
}

/// Contract: `hook_env` rejects shells outside zsh/bash/fish, naming the
/// offending shell in the error.
#[test]
fn hook_env_rejects_unsupported_shell() {
    let error = hook_env("powershell").expect_err("unsupported shell must error");
    let message = format!("{error:#}");
    assert!(
        message.contains("Unsupported shell") && message.contains("powershell"),
        "error must name the rejected shell, got {message:?}"
    );
}

/// Contract: `print_hook` rejects unknown shells with the exact remediation
/// message listing the supported shells.
#[test]
fn print_hook_rejects_unknown_shell_with_exact_message() {
    let error = print_hook("tcsh").expect_err("unknown shell must error");
    assert_eq!(
        format!("{error:#}"),
        "Unsupported shell: tcsh. Supported: zsh, bash, fish",
        "remediation message is part of the public contract"
    );
}

/// Contract: `omg hook <shell>` prints each shell's integration script with
/// its load-bearing wiring intact (the eval line, prompt registration).
#[test]
fn hook_scripts_expose_required_integration_markers() {
    let zsh = run_omg(&["hook", "zsh"]);
    zsh.assert_success();
    zsh.assert_stdout_contains("_omg_hook()");
    zsh.assert_stdout_contains("\\command omg hook-env -s zsh");
    zsh.assert_stdout_contains("precmd_functions=(_omg_hook ${precmd_functions[@]})");
    zsh.assert_stdout_contains("chpwd_functions=(_omg_hook ${chpwd_functions[@]})");

    let bash = run_omg(&["hook", "bash"]);
    bash.assert_success();
    bash.assert_stdout_contains("_omg_hook()");
    bash.assert_stdout_contains("\\command omg hook-env -s bash");
    bash.assert_stdout_contains("PROMPT_COMMAND=\"_omg_hook${PROMPT_COMMAND:+;$PROMPT_COMMAND}\"");

    let fish = run_omg(&["hook", "fish"]);
    fish.assert_success();
    fish.assert_stdout_contains("--on-variable PWD");
    fish.assert_stdout_contains("omg hook-env -s fish | source");
}

/// Minimal mkdir helper so tests above stay terse (std has no create_dir_all
/// taking a PathBuf reference conveniently chained).
mod fs_extra_or_std {
    pub fn create_dir_all(path: &std::path::Path) {
        std::fs::create_dir_all(path).expect("failed to create test directory");
    }
}
