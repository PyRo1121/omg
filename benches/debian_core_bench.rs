//! Core Debian Function Benchmarks
//!
//! Criterion benchmarks for critical Debian package manager functions
//! to identify performance bottlenecks and optimization opportunities.
//!
//! Run with: `cargo bench --features debian-pure --bench debian_core_bench`

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use std::time::Duration;

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use omg_lib::package_managers::debian_db::{
    check_updates_available, compare_versions, get_info_fast, get_package_dependencies,
    list_installed_fast, search_fast,
};

/// Benchmark the search_fast function with various query patterns
#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_search_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("debian_search_patterns");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    // Warm up the index
    let _ = search_fast("vim");

    let test_queries = vec![
        // Short exact matches
        ("exact_short", "vim"),
        ("exact_short_2", "gcc"),
        ("exact_short_3", "git"),
        // Medium exact matches
        ("exact_medium", "firefox"),
        ("exact_medium_2", "systemd"),
        // Long exact matches
        ("exact_long", "build-essential"),
        ("exact_long_2", "libreoffice-common"),
        // Prefix searches (most common)
        ("prefix_short", "lib"),
        ("prefix_medium", "python"),
        ("prefix_long", "x11-utils"),
        // Wildcard/fuzzy searches
        ("fuzzy_common", "fire"),
        ("fuzzy_lib", "libssl"),
        // Edge cases
        ("single_char", "x"),
        ("numeric", "gcc-11"),
        ("hyphenated", "command-not-found"),
    ];

    for (name, query) in test_queries {
        group.bench_with_input(BenchmarkId::new("search", name), &query, |b, &q| {
            b.iter(|| {
                let results = search_fast(q).expect("search should succeed");
                std::hint::black_box(results)
            });
        });
    }

    group.finish();
}

/// Benchmark get_info_fast for different package types
#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_info_package_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("debian_info_types");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    // Warm up
    let _ = get_info_fast("vim");

    let package_types = vec![
        // Small packages
        ("small_binary", "hello"),
        ("small_utility", "htop"),
        // Medium packages
        ("medium_app", "vim"),
        ("medium_tool", "gcc"),
        // Large packages (many dependencies)
        ("large_lib", "libc6"),
        ("large_app", "firefox-esr"),
        // System packages
        ("system_core", "systemd"),
        ("system_shell", "bash"),
        // Multi-arch packages
        ("multiarch", "zlib1g"),
        // Virtual packages (if applicable)
        ("common", "coreutils"),
    ];

    for (name, pkg) in package_types {
        group.bench_with_input(BenchmarkId::new("info", name), &pkg, |b, &p| {
            b.iter(|| {
                let info = get_info_fast(p);
                std::hint::black_box(info)
            });
        });
    }

    group.finish();
}

/// Benchmark version comparison with realistic version strings
#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_version_comparison_realistic(c: &mut Criterion) {
    let mut group = c.benchmark_group("debian_version_realistic");
    group.measurement_time(Duration::from_secs(5));
    group.sample_size(200);

    let version_comparisons = vec![
        // Simple version bumps
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
        ("debian_rev_complex", "2:7.4.052-1ubuntu3.1", "2:7.4.052-1ubuntu4"),
        // Tilde (pre-release) handling
        ("tilde_beta", "1.0~beta", "1.0~rc"),
        ("tilde_vs_release", "1.0~rc1", "1.0"),
        ("tilde_complex", "1.0~alpha1", "1.0~alpha2"),
        // Real-world examples
        ("real_vim", "2:8.2.3995-1", "2:8.2.4659-1"),
        ("real_gcc", "4:11.2.0-1ubuntu1", "4:12.1.0-2ubuntu1"),
        ("real_systemd", "249.11-0ubuntu3.7", "249.11-0ubuntu3.9"),
        ("real_kernel", "5.15.0-60.66", "5.15.0-67.74"),
    ];

    for (name, v1, v2) in version_comparisons {
        group.bench_with_input(
            BenchmarkId::new("compare", name),
            &(v1, v2),
            |b, &(ver1, ver2)| {
                b.iter(|| {
                    let result = compare_versions(ver1, ver2);
                    std::hint::black_box(result)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark dependency resolution for packages with varying complexity
#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_dependencies_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("debian_dependencies");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(50);

    let packages = vec![
        ("no_deps", "hello"),
        ("few_deps", "htop"),
        ("moderate_deps", "vim"),
        ("many_deps", "gcc"),
        ("complex_deps", "build-essential"),
        ("deep_tree", "firefox-esr"),
    ];

    for (name, pkg) in packages {
        group.bench_with_input(BenchmarkId::new("get_deps", name), &pkg, |b, &p| {
            b.iter(|| {
                let deps = get_package_dependencies(p);
                std::hint::black_box(deps)
            });
        });
    }

    group.finish();
}

/// Benchmark list_installed_fast
#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_list_installed_variants(c: &mut Criterion) {
    let mut group = c.benchmark_group("debian_list_installed");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    // Benchmark the full list operation
    group.bench_function("full_list", |b| {
        b.iter(|| {
            let packages = list_installed_fast().expect("list should succeed");
            std::hint::black_box(packages)
        });
    });

    // Benchmark list and count (typical use case)
    group.bench_function("list_and_count", |b| {
        b.iter(|| {
            let packages = list_installed_fast().expect("list should succeed");
            let count = packages.len();
            std::hint::black_box((packages, count))
        });
    });

    // Benchmark list and filter (common pattern)
    group.bench_function("list_and_filter", |b| {
        b.iter(|| {
            let packages = list_installed_fast().expect("list should succeed");
            let filtered: Vec<_> = packages.iter().filter(|p| p.starts_with("lib")).collect();
            std::hint::black_box(filtered)
        });
    });

    group.finish();
}

/// Benchmark check_updates_available with realistic scenarios
#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_update_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("debian_update_check");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(30);

    group.bench_function("check_all_updates", |b| {
        b.iter(|| {
            let updates = check_updates_available().expect("update check should succeed");
            std::hint::black_box(updates)
        });
    });

    group.finish();
}

/// Benchmark FST index operations if available
#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_index_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("debian_index");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(100);

    // These benchmarks measure the index lookup performance
    // which is critical for fast search operations

    let queries = vec![
        "a", "ab", "abc", "vim", "python", "lib", "libssl", "gcc", "systemd", "firefox",
    ];

    for query in queries {
        group.bench_with_input(
            BenchmarkId::new("index_lookup", query),
            &query,
            |b, &q| {
                b.iter(|| {
                    // search_fast uses FST index internally
                    let results = search_fast(q).expect("search should succeed");
                    // Measure just the index lookup by only checking count
                    std::hint::black_box(results.len())
                });
            },
        );
    }

    group.finish();
}

/// Benchmark parallel operations (version comparison at scale)
#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_parallel_version_compare(c: &mut Criterion) {
    use rayon::prelude::*;

    let mut group = c.benchmark_group("debian_parallel");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(30);

    // Generate version pairs (simulating update check scenario)
    let version_pairs: Vec<(String, String)> = (0..1000)
        .map(|i| {
            (
                format!("1.{}.{}-1ubuntu1", i / 100, i % 100),
                format!("1.{}.{}-1ubuntu2", i / 100, i % 100),
            )
        })
        .collect();

    group.throughput(Throughput::Elements(version_pairs.len() as u64));

    // Sequential comparison
    group.bench_with_input(
        BenchmarkId::new("sequential", version_pairs.len()),
        &version_pairs,
        |b, pairs| {
            b.iter(|| {
                let results: Vec<_> = pairs
                    .iter()
                    .map(|(v1, v2)| compare_versions(v1, v2))
                    .collect();
                std::hint::black_box(results)
            });
        },
    );

    // Parallel comparison
    group.bench_with_input(
        BenchmarkId::new("parallel", version_pairs.len()),
        &version_pairs,
        |b, pairs| {
            b.iter(|| {
                let results: Vec<_> = pairs
                    .par_iter()
                    .map(|(v1, v2)| compare_versions(v1, v2))
                    .collect();
                std::hint::black_box(results)
            });
        },
    );

    group.finish();
}

/// Benchmark memory allocation patterns in common operations
#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_memory_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("debian_memory");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    let sizes = vec![10, 50, 100, 500, 1000];

    for size in sizes {
        group.throughput(Throughput::Elements(size as u64));

        // Pre-allocated Vec
        group.bench_with_input(
            BenchmarkId::new("vec_with_capacity", size),
            &size,
            |b, &n| {
                b.iter(|| {
                    let mut packages = Vec::with_capacity(n);
                    for i in 0..n {
                        packages.push(format!("package-{i}"));
                    }
                    std::hint::black_box(packages)
                });
            },
        );

        // Standard Vec
        group.bench_with_input(BenchmarkId::new("vec_standard", size), &size, |b, &n| {
            b.iter(|| {
                let mut packages = Vec::new();
                for i in 0..n {
                    packages.push(format!("package-{i}"));
                }
                std::hint::black_box(packages)
            });
        });

        // SmallVec (for small collections)
        use smallvec::SmallVec;

        group.bench_with_input(
            BenchmarkId::new("smallvec_8", size),
            &size,
            |b, &n| {
                b.iter(|| {
                    let mut packages: SmallVec<[String; 8]> = SmallVec::new();
                    for i in 0..n.min(20) {
                        // Limit to avoid excessive stack usage
                        packages.push(format!("package-{i}"));
                    }
                    std::hint::black_box(packages)
                });
            },
        );
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
criterion_group!(
    debian_core_benches,
    bench_search_patterns,
    bench_info_package_types,
    bench_version_comparison_realistic,
    bench_dependencies_complexity,
    bench_list_installed_variants,
    bench_update_check,
    bench_index_operations,
    bench_parallel_version_compare,
    bench_memory_patterns
);

#[cfg(any(feature = "debian", feature = "debian-pure"))]
criterion_main!(debian_core_benches);

#[cfg(not(any(feature = "debian", feature = "debian-pure")))]
fn main() {
    println!("Debian core benchmarks require --features debian-pure or --features debian");
}
