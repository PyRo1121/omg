//! Benchmark package counting operations
//!
//! Tests the performance of optimized counting functions

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;

#[cfg(feature = "arch")]
fn bench_count_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("count_operations");
    group.measurement_time(std::time::Duration::from_secs(10));

    // Benchmark get_counts (all counts in single pass)
    group.bench_function("get_counts_single_pass", |b| {
        b.iter(|| {
            let _ = black_box(omg_lib::package_managers::alpm_direct::get_counts());
        });
    });

    // Benchmark get_explicit_count_fast (count only, no list)
    group.bench_function("get_explicit_count_fast", |b| {
        b.iter(|| {
            let _ = black_box(omg_lib::package_managers::alpm_direct::get_explicit_count_fast());
        });
    });

    // Compare with full list generation
    group.bench_function("list_explicit_then_count", |b| {
        b.iter(|| {
            let list = omg_lib::package_managers::alpm_direct::list_explicit_fast();
            let _ = black_box(list.map(|l| l.len()));
        });
    });

    group.finish();
}

#[cfg(not(feature = "arch"))]
fn bench_count_operations(_c: &mut Criterion) {
    eprintln!("Skipping count benchmarks - arch feature not enabled");
}

criterion_group!(benches, bench_count_operations);
criterion_main!(benches);
