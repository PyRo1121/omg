#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
#[cfg(feature = "arch")]
use omg_lib::package_managers::pacman_db;

#[cfg(feature = "arch")]
#[test]
fn bench_pure_rust_update_check() {
    use std::time::Instant;

    println!("\n=== Pure Rust DB Parser Benchmark ===");

    // First load (cold)
    let start = Instant::now();
    let updates = pacman_db::check_updates_fast().unwrap();
    let first_load = start.elapsed();
    println!("First load (cold): {first_load:?}");
    println!("Updates found: {}", updates.len());

    // Second call (cached)
    let start = Instant::now();
    let _ = pacman_db::check_updates_fast().unwrap();
    let cached = start.elapsed();
    println!("Cached call: {cached:?}");

    // Third call (cached)
    let start = Instant::now();
    let _ = pacman_db::check_updates_fast().unwrap();
    let cached2 = start.elapsed();
    println!("Cached call 2: {cached2:?}");

    assert!(
        cached.as_millis() < 20,
        "Cached call should be <20ms, was {}ms",
        cached.as_millis()
    );
}
