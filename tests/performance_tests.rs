//! Performance tests for Phase 3 - Measure critical operations
//!
//! Run with: cargo test --release --test performance_tests --features arch -- --nocapture

use std::time::Instant;

#[cfg(feature = "arch")]
use omg_lib::package_managers;

/// Helper to measure operation time
fn measure<F, T>(name: &str, f: F) -> T
where
    F: FnOnce() -> T,
{
    let start = Instant::now();
    let result = f();
    let elapsed = start.elapsed();
    println!("{}: {:.2}ms", name, elapsed.as_secs_f64() * 1000.0);
    result
}

#[test]
#[cfg(feature = "arch")]
#[ignore = "Performance test - run with --ignored --nocapture"]
fn test_search_performance() {
    println!("\n=== Search Performance Tests ===");

    // Warmup - initialize caches
    let _ = package_managers::pacman_db::search_sync_fast("rust");

    // Test various search terms
    for term in &["rust", "python", "vim", "gcc", "kernel"] {
        measure(
            &format!("search_sync_fast({})", term),
            || {
                package_managers::pacman_db::search_sync_fast(term)
                    .expect("Search failed")
            },
        );
    }
}

#[test]
#[cfg(feature = "arch")]
#[ignore = "Performance test - run with --ignored --nocapture"]
fn test_search_local_performance() {
    println!("\n=== Local Search Performance Tests ===");

    // Warmup
    let _ = package_managers::pacman_db::search_local_cached("rust");

    // Test various search terms
    for term in &["rust", "python", "vim", "gcc", "kernel"] {
        measure(
            &format!("search_local_cached({})", term),
            || {
                package_managers::pacman_db::search_local_cached(term)
                    .expect("Local search failed")
            },
        );
    }
}

#[test]
#[cfg(feature = "arch")]
#[ignore = "Performance test - run with --ignored --nocapture"]
fn test_explicit_list_performance() {
    println!("\n=== Explicit List Performance Tests ===");

    // Warmup
    let _ = package_managers::list_explicit_fast();

    // Run multiple iterations
    for i in 1..=5 {
        measure(
            &format!("list_explicit_fast (iteration {})", i),
            || {
                package_managers::list_explicit_fast()
                    .expect("List explicit failed")
            },
        );
    }
}

#[test]
#[cfg(feature = "arch")]
#[ignore = "Performance test - run with --ignored --nocapture"]
fn test_unified_search_performance() {
    println!("\n=== Unified Search Performance Tests ===");

    // Warmup
    let _ = package_managers::search_sync("rust");

    // Test various search terms
    for term in &["rust", "python", "vim"] {
        measure(
            &format!("search_sync({})", term),
            || {
                package_managers::search_sync(term)
                    .expect("Unified search failed")
            },
        );
    }
}

#[test]
#[cfg(feature = "arch")]
#[ignore = "Performance test - run with --ignored --nocapture"]
fn test_get_package_info_performance() {
    println!("\n=== Package Info Performance Tests ===");

    // Warmup
    let _ = package_managers::get_package_info("glibc");

    // Test info retrieval for common packages
    for pkg in &["glibc", "systemd", "linux", "gcc", "rust"] {
        measure(
            &format!("get_package_info({})", pkg),
            || {
                package_managers::get_package_info(pkg)
                    .expect("Get package info failed")
            },
        );
    }
}

#[test]
#[cfg(feature = "arch")]
#[ignore = "Performance test - run with --ignored --nocapture"]
fn test_is_installed_performance() {
    println!("\n=== Is Installed Performance Tests ===");

    // Warmup
    let _ = package_managers::is_installed_fast("glibc");

    // Test installation check for various packages
    for pkg in &["glibc", "rust", "python", "nonexistent-package-xyz"] {
        measure(
            &format!("is_installed_fast({})", pkg),
            || {
                package_managers::is_installed_fast(pkg)
                    .expect("Is installed check failed")
            },
        );
    }
}

#[test]
#[cfg(not(feature = "arch"))]
fn test_arch_feature_required() {
    eprintln!("Performance tests require --features arch");
}
