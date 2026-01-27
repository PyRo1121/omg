//! Performance benchmarks for core package operations
//!
//! These benchmarks measure the performance of critical operations
//! to ensure Phase 3 architectural changes haven't introduced regressions.

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use std::hint::black_box;
use std::time::Duration;

#[cfg(feature = "arch")]
use omg_lib::package_managers;

/// Benchmark package search operations
#[cfg(feature = "arch")]
fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");
    group.measurement_time(Duration::from_secs(10));

    // Test various search patterns
    let search_terms = vec!["rust", "python", "vim", "gcc", "kernel"];

    for term in search_terms {
        group.bench_with_input(
            BenchmarkId::new("search_sync_fast", term),
            &term,
            |b, &term| {
                b.iter(|| {
                    let _ = black_box(package_managers::pacman_db::search_sync_fast(black_box(term)));
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("search_local_cached", term),
            &term,
            |b, &term| {
                b.iter(|| {
                    let _ = black_box(package_managers::pacman_db::search_local_cached(black_box(term)));
                });
            },
        );
    }

    group.finish();
}

#[cfg(not(feature = "arch"))]
fn bench_search(_c: &mut Criterion) {
    eprintln!("Skipping search benchmarks - arch feature not enabled");
}

/// Benchmark explicit package listing
#[cfg(feature = "arch")]
fn bench_explicit(c: &mut Criterion) {
    let mut group = c.benchmark_group("explicit");
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("list_explicit_fast", |b| {
        b.iter(|| {
            let _ = black_box(package_managers::list_explicit_fast());
        });
    });

    group.finish();
}

#[cfg(not(feature = "arch"))]
fn bench_explicit(_c: &mut Criterion) {
    eprintln!("Skipping explicit benchmarks - arch feature not enabled");
}

/// Benchmark unified search operations
#[cfg(feature = "arch")]
fn bench_unified_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("unified_search");
    group.measurement_time(Duration::from_secs(10));

    let search_terms = vec!["rust", "python", "vim"];

    for term in search_terms {
        group.bench_with_input(
            BenchmarkId::new("search_sync", term),
            &term,
            |b, &term| {
                b.iter(|| {
                    let _ = black_box(package_managers::search_sync(black_box(term)));
                });
            },
        );
    }

    group.finish();
}

#[cfg(not(feature = "arch"))]
fn bench_unified_search(_c: &mut Criterion) {
    eprintln!("Skipping unified search benchmarks - arch feature not enabled");
}

criterion_group!(
    package_ops,
    bench_search,
    bench_explicit,
    bench_unified_search,
);
criterion_main!(package_ops);
