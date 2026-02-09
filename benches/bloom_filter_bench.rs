//! Benchmark bloom filter performance for package lookups
//!
//! Tests the impact of bloom filter optimization on info and search operations

use criterion::{Criterion, criterion_group, criterion_main};

#[cfg(feature = "arch")]
use omg_lib::daemon::index::PackageIndex;

#[cfg(feature = "arch")]
fn bench_package_lookups(c: &mut Criterion) {
    use std::hint::black_box;

    // Build index once for all benchmarks
    let index = match PackageIndex::new() {
        Ok(idx) => idx,
        Err(e) => {
            eprintln!("Failed to build package index: {e}");
            return;
        }
    };

    let mut group = c.benchmark_group("package_lookups");
    group.measurement_time(std::time::Duration::from_secs(10));

    // Test existing package (bloom filter + HashMap hit)
    group.bench_function("get_existing_package", |b| {
        b.iter(|| {
            black_box(index.get(black_box("firefox")));
        });
    });

    // Test non-existent package (bloom filter rejects without HashMap lookup)
    group.bench_function("get_nonexistent_package", |b| {
        b.iter(|| {
            black_box(index.get(black_box("this-package-does-not-exist-xyz123")));
        });
    });

    // Batch lookups - mix of existing and non-existing
    let test_packages = vec![
        "firefox",
        "nonexistent1",
        "linux",
        "nonexistent2",
        "vim",
        "nonexistent3",
        "gcc",
        "nonexistent4",
    ];

    group.bench_function("get_mixed_batch", |b| {
        b.iter(|| {
            for pkg in &test_packages {
                black_box(index.get(black_box(pkg)));
            }
        });
    });

    // Search with bloom filter pre-check
    group.bench_function("search_short_query", |b| {
        b.iter(|| {
            black_box(index.search(black_box("fire"), 50));
        });
    });

    group.bench_function("search_long_query", |b| {
        b.iter(|| {
            black_box(index.search(black_box("firefox-developer-edition"), 50));
        });
    });

    group.finish();
}

#[cfg(not(feature = "arch"))]
fn bench_package_lookups(_c: &mut Criterion) {
    eprintln!("Skipping bloom filter benchmarks - arch feature not enabled");
}

criterion_group!(benches, bench_package_lookups);
criterion_main!(benches);
