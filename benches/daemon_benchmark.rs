//! Daemon Performance Benchmarks
//!
//! Comprehensive benchmarks for daemon critical paths:
//! - Cache operations (hit/miss/Arc cloning)
//! - Index search performance
//! - Handler dispatch overhead
//! - Runtime resolution functions

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::sync::Arc;
use std::time::Duration;

use omg_lib::core::runtime_resolver;
use omg_lib::daemon::cache::PackageCache;
use omg_lib::daemon::index::PackageIndex;
use omg_lib::daemon::protocol::PackageInfo;

/// Benchmark cache operations with Arc optimization
fn bench_cache_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("cache");
    group.measurement_time(Duration::from_secs(5));

    let cache = PackageCache::default();

    // Pre-populate cache
    let test_packages: Vec<PackageInfo> = (0..100)
        .map(|i| PackageInfo {
            name: format!("package{i}"),
            version: "1.0.0".to_string(),
            description: format!("Test package {i}"),
            source: "test".to_string(),
        })
        .collect();

    for (i, pkg) in test_packages.iter().enumerate() {
        cache.insert(format!("query{i}"), vec![pkg.clone()]);
    }

    // Benchmark cache hit (Arc clone - should be ~nanoseconds)
    group.bench_function("cache_hit_arc_clone", |b| {
        b.iter(|| {
            let result = cache.get(black_box("query50"));
            black_box(result);
        });
    });

    // Benchmark cache miss
    group.bench_function("cache_miss", |b| {
        b.iter(|| {
            let result = cache.get(black_box("nonexistent"));
            black_box(result);
        });
    });

    // Benchmark cache insert with Arc (optimal path)
    group.bench_function("cache_insert_arc", |b| {
        let mut counter = 0;
        b.iter(|| {
            let key = format!("new_query{}", counter);
            let data = Arc::new(test_packages.clone());
            cache.insert_arc(black_box(key), data);
            counter += 1;
        });
    });

    // Benchmark cache insert without Arc (sub-optimal for comparison)
    group.bench_function("cache_insert_vec", |b| {
        let mut counter = 100000;
        b.iter(|| {
            let key = format!("new_query{}", counter);
            cache.insert(black_box(key), test_packages.clone());
            counter += 1;
        });
    });

    // Benchmark explicit count cache (hot path)
    cache.update_explicit_count(1234);
    group.bench_function("get_explicit_count", |b| {
        b.iter(|| {
            let count = cache.get_explicit_count();
            black_box(count);
        });
    });

    group.finish();
}

/// Benchmark package index search performance
#[cfg(feature = "arch")]
fn bench_index_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_search");
    group.measurement_time(Duration::from_secs(5));

    // Build real index from system packages
    let index = match PackageIndex::new() {
        Ok(idx) => idx,
        Err(_) => {
            eprintln!("Skipping index benchmarks - failed to build index");
            return;
        }
    };

    let search_terms = vec!["rust", "python", "vim", "gcc", "linux"];

    for term in search_terms {
        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("search", term), &term, |b, &term| {
            b.iter(|| {
                let results = index.search(black_box(term), black_box(50));
                black_box(results);
            });
        });

        // Benchmark exact package lookup (hash map lookup)
        group.bench_with_input(BenchmarkId::new("get_exact", term), &term, |b, &term| {
            b.iter(|| {
                let result = index.get(black_box(term));
                black_box(result);
            });
        });
    }

    // Benchmark index.len() (should be inlined)
    group.bench_function("len", |b| {
        b.iter(|| {
            let len = index.len();
            black_box(len);
        });
    });

    group.finish();
}

#[cfg(not(feature = "arch"))]
fn bench_index_search(_c: &mut Criterion) {
    eprintln!("Skipping index benchmarks - arch feature not enabled");
}

/// Benchmark runtime resolution functions
fn bench_runtime_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("runtime_resolution");
    group.measurement_time(Duration::from_secs(5));

    // Benchmark find_in_path (hot path during task execution)
    group.bench_function("find_in_path_sh", |b| {
        b.iter(|| {
            let result = runtime_resolver::find_in_path(black_box("sh"));
            black_box(result);
        });
    });

    group.bench_function("find_in_path_nonexistent", |b| {
        b.iter(|| {
            let result =
                runtime_resolver::find_in_path(black_box("definitely-does-not-exist-12345"));
            black_box(result);
        });
    });

    // Benchmark mise_available (called frequently)
    group.bench_function("mise_available", |b| {
        b.iter(|| {
            let result = runtime_resolver::mise_available();
            black_box(result);
        });
    });

    // Benchmark native runtime path resolution
    group.bench_function("native_runtime_bin_path", |b| {
        b.iter(|| {
            let result =
                runtime_resolver::native_runtime_bin_path(black_box("node"), black_box("20.0.0"));
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark Arc cloning overhead (should be ~nanoseconds)
fn bench_arc_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("arc_overhead");
    group.measurement_time(Duration::from_secs(3));

    let test_data: Vec<PackageInfo> = (0..1000)
        .map(|i| PackageInfo {
            name: format!("pkg{i}"),
            version: "1.0.0".to_string(),
            description: "test".to_string(),
            source: "test".to_string(),
        })
        .collect();

    let arc_data = Arc::new(test_data.clone());

    // Benchmark Arc clone (reference counting increment)
    group.bench_function("arc_clone_1000_items", |b| {
        b.iter(|| {
            let cloned = Arc::clone(&arc_data);
            black_box(cloned);
        });
    });

    // Compare with Vec clone (full data copy)
    group.bench_function("vec_clone_1000_items", |b| {
        b.iter(|| {
            let cloned = test_data.clone();
            black_box(cloned);
        });
    });

    group.finish();
}

criterion_group!(
    daemon_benches,
    bench_cache_operations,
    bench_index_search,
    bench_runtime_resolution,
    bench_arc_clone,
);
criterion_main!(daemon_benches);
