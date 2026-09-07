//! Coverage tests for `src/package_managers/aur/client.rs` (agent cov-1).
//!
//! Contracts pinned here, all falsifiable:
//!
//! 1. `AurClient::build_concurrency()` clamps a configured concurrency of 0
//!    up to 1 (`settings.aur.build_concurrency.max(1)`) and passes a positive
//!    value through unchanged.
//! 2. `AurClient::install()` rejects injection-style and empty package names
//!    with EXACT validation-error messages before any network or filesystem
//!    side effect.
//! 3. `AurClient::downgrade_from_history()` validates its VERSION argument
//!    with the bounded package-version grammar before performing I/O.
//! 4. `AurClient::downgrade_from_history()` validates its PACKAGE argument
//!    before anything else (leading `-` rejected: option injection guard).
//! 5. The manual `Debug` impl never leaks `Settings` (user-specific paths,
//!    review flags, makeflags) — a documented security property.
//!
//! Every assertion pins exact observable output (error Display strings),
//! so any mutation of the guarded validation lines, the clamp, or the
//! Debug field set changes the observed value and fails the test.

#![cfg(all(feature = "arch", target_os = "linux"))]
#![expect(clippy::unwrap_used, clippy::expect_used, clippy::pedantic)]

pub mod common;

use std::fs;

use common::*;
use tempfile::TempDir;

/// Exact error strings from `ValidationError` (src/core/security/validation.rs).
const ERR_INVALID_CHAR_SEMICOLON: &str =
    "Invalid character ';' in package name. Only alphanumeric, -, _, ., +, @, / allowed";
const ERR_EMPTY_NAME: &str = "Package name cannot be empty";
const ERR_LEADING_DASH: &str = "Package name cannot start with '-' (option injection protection)";
const ERR_INVALID_VERSION_CHAR: &str = "Invalid character ' ' in version string";

use omg_lib::config::{AurBuildMethod, Settings};
use omg_lib::package_managers::AurClient;

#[test]
fn default_aur_policy_sandboxes_without_interactive_review() {
    let settings = Settings::default();

    assert!(
        !settings.aur.review_pkgbuild,
        "PKGBUILD review is opt-in (--review or aur.review_pkgbuild)"
    );
    assert!(
        matches!(settings.aur.build_method, AurBuildMethod::Bubblewrap),
        "default AUR builds must use the supported sandbox"
    );
    assert!(
        !settings.aur.allow_unsafe_builds,
        "missing sandbox support must fail closed by default"
    );
}

/// Run `f` with `OMG_CONFIG_DIR` pointing at a fresh temp dir whose only file
/// is `config.toml` = `config_toml`, and `OMG_CACHE_DIR` at another fresh
/// empty temp dir. The client MUST be constructed inside `f`: settings and
/// cache paths are resolved from these variables at construction time.
fn with_hermetic_dirs<R>(config_toml: &str, f: impl FnOnce() -> R) -> R {
    let config_dir = TempDir::new().expect("temp config dir");
    let cache_dir = TempDir::new().expect("temp cache dir");
    fs::write(config_dir.path().join("config.toml"), config_toml)
        .expect("write hermetic config.toml");

    with_test_env(
        &[
            (
                "OMG_CONFIG_DIR",
                config_dir.path().to_str().expect("utf8 temp path"),
            ),
            (
                "OMG_CACHE_DIR",
                cache_dir.path().to_str().expect("utf8 temp path"),
            ),
        ],
        f,
    )
}

/// Current-thread runtime for the async client entry points.
fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime")
}

// ============================================================================
// Contract 1: build_concurrency clamp
// ============================================================================

/// `build_concurrency()` must never return 0 — a zero configured value would
/// make `buffer_unordered(0)` deadlock every parallel AUR build wave forever.
#[test]
#[serial]
fn build_concurrency_clamps_configured_zero_to_one() {
    with_hermetic_dirs("[aur]\nbuild_concurrency = 0\n", || {
        let client = AurClient::new().expect("AurClient::new loads hermetic settings");
        assert_eq!(
            client.build_concurrency(),
            1,
            "a configured build_concurrency of 0 must be clamped to 1; \
             buffer_unordered(0) would hang parallel builds forever"
        );
    });
}

/// Positive configured values pass through untouched (the clamp is a floor,
/// not a rewrite of user intent).
#[test]
#[serial]
fn build_concurrency_passes_positive_values_through() {
    with_hermetic_dirs("[aur]\nbuild_concurrency = 5\n", || {
        let client = AurClient::new().expect("AurClient::new loads hermetic settings");
        assert_eq!(client.build_concurrency(), 5);
    });
}

// ============================================================================
// Contracts 2-4: validate-before-any-IO ordering in install()/downgrade
// ============================================================================

/// `install()` rejects injection-style names with the exact validation error,
/// BEFORE contacting AUR RPC, cloning git repos, creating directories, or
/// acquiring sudo. The name below would be catastrophic if it reached shell
/// or URL interpolation downstream.
#[test]
#[serial]
fn install_rejects_injection_style_names_before_any_io() {
    with_hermetic_dirs("[aur]\nreview_pkgbuild = true\n", || {
        let client = AurClient::new().expect("client construction");
        let runtime = rt();

        let error = runtime
            .block_on(client.install("pkg; rm -rf /"))
            .expect_err("injection-style package name must be rejected");
        assert_eq!(
            error.to_string(),
            ERR_INVALID_CHAR_SEMICOLON,
            "install() must fail at the package-name boundary itself"
        );

        let empty_error = runtime
            .block_on(client.install(""))
            .expect_err("empty package name must be rejected");
        assert_eq!(empty_error.to_string(), ERR_EMPTY_NAME);
    });
}

/// `downgrade_from_history()` validates the VERSION argument before
/// resolve_package_base(), directory creation under `_rollback/`, or any git
/// invocation. The package name here is deliberately valid so only the
/// version-validation line can produce this error.
#[test]
#[serial]
fn downgrade_rejects_invalid_version_character_before_any_io() {
    with_hermetic_dirs("[aur]\n", || {
        let client = AurClient::new().expect("client construction");
        let runtime = rt();

        let error = runtime
            .block_on(client.downgrade_from_history("zzz-omg-nonexistent-9x7", "1 0"))
            .expect_err("unsafe version character must be rejected before any IO");
        assert_eq!(
            error.to_string(),
            ERR_INVALID_VERSION_CHAR,
            "downgrade_from_history must validate its version argument"
        );
    });
}

/// `downgrade_from_history()` rejects a leading-dash PACKAGE name before
/// touching git or the network. A dash-prefixed string reaching a git or
/// pacman command line would parse as an option flag.
#[test]
#[serial]
fn downgrade_rejects_leading_dash_package_name_before_any_io() {
    with_hermetic_dirs("[aur]\n", || {
        let client = AurClient::new().expect("client construction");
        let runtime = rt();

        let error = runtime
            .block_on(client.downgrade_from_history("-pwn", "1.0"))
            .expect_err("dash-leading package name must be rejected");
        assert_eq!(
            error.to_string(),
            ERR_LEADING_DASH,
            "downgrade_from_history must reject option-injection package names"
        );
    });
}

// ============================================================================
// Contract 5: Debug output never leaks settings
// ============================================================================

/// AurClient's hand-written Debug impl documents that settings are excluded
/// because they carry user-specific paths. If someone re-adds the field (or
/// switches to derive(Debug)), secrets like pkgdest paths, makeflags, and
/// security-review toggles start appearing in logs and panic messages.
#[test]
#[serial]
fn debug_output_never_leaks_settings() {
    with_hermetic_dirs("[aur]\nmakeflags = \"-j42\"\n", || {
        let client = AurClient::new().expect("AurClient::new loads hermetic settings");

        let debug_output = format!("{client:?}");

        // Positive: this IS the AurClient debug output, naming its build dir.
        assert!(
            debug_output.contains("AurClient"),
            "Debug output must identify the struct: {debug_output}"
        );
        assert!(
            debug_output.contains("build_dir"),
            "Debug output must include the non-sensitive build_dir field: {debug_output}"
        );

        // Negative: none of the excluded settings may appear, including the
        // makeflags planted via the hermetic config above.
        assert!(
            !debug_output.contains("Settings"),
            "Debug output must not serialize Settings: {debug_output}"
        );
        assert!(
            !debug_output.contains("AurBuildSettings"),
            "Debug output must not serialize AurBuildSettings: {debug_output}"
        );
        assert!(
            !debug_output.contains("-j42"),
            "Debug output leaked configured makeflags: {debug_output}"
        );
        assert!(
            !debug_output.contains("review_pkgbuild"),
            "Debug output leaked security-review flags: {debug_output}"
        );
    });
}
