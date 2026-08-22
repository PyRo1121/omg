//! Shared setup and scenario data for the Debian benchmark targets
//! (`debian_bench` and `debian_core_bench`).
//!
//! Include from a bench crate root with:
//! ```ignore
//! #[cfg(any(feature = "debian", feature = "debian-pure"))]
//! #[path = "debian_common/mod.rs"]
//! mod debian_common;
//! ```

use omg_lib::package_managers::debian_db;

/// Warm the debian_db index and caches so benchmarks measure steady state
/// instead of first-load cost.
pub fn warm_index() {
    let _ = debian_db::search_fast("vim");
    let _ = debian_db::get_info_fast("vim");
}

/// Dependency-resolution scenarios shared by both Debian bench targets:
/// `(label, package)` ordered from few dependencies to complex trees.
pub const RESOLVE_SCENARIOS: &[(&str, &str)] = &[
    ("no_deps", "hello"),
    ("few_deps", "htop"),
    ("moderate_deps", "vim"),
    ("many_deps", "gcc"),
    ("complex_deps", "build-essential"),
    ("deep_tree", "firefox-esr"),
    ("large_tree", "gimp"),
];

/// Debian version-comparison scenarios shared by both targets:
/// `(label, version_a, version_b)` covering epochs, revisions, tilde
/// pre-releases, and real-world package versions.
pub const VERSION_SCENARIOS: &[(&str, &str, &str)] = &[
    // Simple bumps
    ("simple_patch", "1.0.0", "1.0.1"),
    ("simple_minor", "1.0.0", "1.1.0"),
    ("simple_major", "1.0.0", "2.0.0"),
    // Epoch handling
    ("epoch_vs_no_epoch", "1.0", "1:0.9"),
    ("epoch_same", "2:1.5", "2:1.6"),
    ("epoch_different", "1:2.0", "2:1.0"),
    // Debian revisions
    ("debian_rev", "1.0-1", "1.0-2"),
    ("debian_rev_ubuntu", "1.0-1ubuntu1", "1.0-1ubuntu2"),
    (
        "debian_rev_complex",
        "2:7.4.052-1ubuntu3.1",
        "2:7.4.052-1ubuntu4",
    ),
    ("rev_ubuntu_pair", "2:1.0.5-1ubuntu1", "2:1.0.5-1ubuntu2"),
    // Tilde (pre-release) handling
    ("tilde_beta_rc", "1.0~beta", "1.0~rc"),
    ("tilde_vs_release", "1.0~rc1", "1.0"),
    ("tilde_alpha_pair", "1.0~alpha1", "1.0~alpha2"),
    // Real-world examples
    ("real_vim", "2:8.2.3995-1", "2:8.2.4659-1"),
    ("real_gcc", "4:11.2.0-1ubuntu1", "4:12.1.0-2ubuntu1"),
    ("real_systemd", "249.11-0ubuntu3.7", "249.11-0ubuntu3.9"),
    ("real_kernel", "5.15.0-60.66", "5.15.0-67.74"),
    ("real_patch_pair", "3.14.159", "3.14.160"),
];
