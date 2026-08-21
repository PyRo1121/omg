//! Debian Install Performance Benchmarks
//!
//! Benchmarks installation with different package counts to identify optimal
//! batch sizes and concurrency levels.
//!
//! Run with: `cargo bench --features debian-pure --bench install_bench`

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use std::time::Duration;

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use omg_lib::package_managers::debian_db::{DependencyResolver, Transaction};

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_install_resolution(c: &mut Criterion) {
    let mut group = c.benchmark_group("install_resolution");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(20);

    // Test different package counts: 1, 5, 10, 20
    let test_cases = vec![
        ("single", vec!["vim"]),
        ("small", vec!["vim", "curl", "git", "htop", "tmux"]),
        (
            "medium",
            vec![
                "vim",
                "curl",
                "git",
                "htop",
                "tmux",
                "build-essential",
                "gcc",
                "make",
                "python3",
                "nodejs",
            ],
        ),
        (
            "large",
            vec![
                "vim",
                "curl",
                "git",
                "htop",
                "tmux",
                "build-essential",
                "gcc",
                "make",
                "python3",
                "nodejs",
                "gcc-multilib",
                "gdb",
                "valgrind",
                "strace",
                "ltrace",
                "binutils",
                "automake",
                "autoconf",
                "libtool",
                "pkg-config",
            ],
        ),
    ];

    for (name, packages) in test_cases {
        group.throughput(Throughput::Elements(packages.len() as u64));
        group.bench_with_input(BenchmarkId::new("resolve", name), &packages, |b, pkgs| {
            b.iter(|| {
                let mut resolver =
                    DependencyResolver::new().expect("resolver creation should succeed");
                for pkg in pkgs {
                    let _ = resolver.add_package(pkg);
                }
                let result = resolver.resolve();
                std::hint::black_box(result)
            });
        });
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_install_transaction_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("install_transaction");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(30);

    let package_counts = vec![1, 5, 10, 20];

    for count in package_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("transaction_create", count),
            &count,
            |b, &n| {
                b.iter(|| {
                    let mut resolver =
                        DependencyResolver::new().expect("resolver creation should succeed");

                    // Add n packages
                    let packages = ["vim", "curl", "git", "htop", "tmux"];
                    for pkg in packages.iter().take(n) {
                        let _ = resolver.add_package(pkg);
                    }

                    let resolution = resolver.resolve().expect("resolution should succeed");
                    let tx = Transaction::from_resolution(resolution).expect("content store init");
                    std::hint::black_box(tx)
                });
            },
        );
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_install_dependency_graph_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("dependency_graph");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(20);

    // Test packages with different dependency tree complexities
    let test_cases = vec![
        ("minimal_deps", "hello"),            // Minimal dependencies
        ("moderate_deps", "vim"),             // Moderate dependency tree
        ("complex_deps", "build-essential"),  // Complex dependency tree
        ("large_deps", "libreoffice-common"), // Large dependency tree (if available)
    ];

    for (name, pkg) in test_cases {
        group.bench_with_input(BenchmarkId::new("resolve", name), &pkg, |b, &p| {
            b.iter(|| {
                let mut resolver =
                    DependencyResolver::new().expect("resolver creation should succeed");
                let _ = resolver.add_package(p);
                let result = resolver.resolve();
                std::hint::black_box(result)
            });
        });
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_install_batch_processing(c: &mut Criterion) {
    use std::collections::HashSet;

    let mut group = c.benchmark_group("batch_processing");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(30);

    let batch_sizes = vec![1, 5, 10, 20, 50];

    for size in batch_sizes {
        #[expect(clippy::cast_sign_loss)]
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("batch_collect", size), &size, |b, &n| {
            b.iter(|| {
                let mut seen = HashSet::new();
                let packages: Vec<String> = (0..n).map(|i| format!("pkg-{i}")).collect();

                for pkg in &packages {
                    seen.insert(pkg.clone());
                }

                std::hint::black_box(seen)
            });
        });
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_install_conflict_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("conflict_detection");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    // Test conflict detection with different package counts
    let package_counts = vec![1, 5, 10, 20];

    for count in package_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(
            BenchmarkId::new("file_conflict_check", count),
            &count,
            |b, &n| {
                b.iter(|| {
                    let mut resolver =
                        DependencyResolver::new().expect("resolver creation should succeed");

                    let packages = ["vim", "curl", "git", "htop", "tmux"];
                    for pkg in packages.iter().take(n) {
                        let _ = resolver.add_package(pkg);
                    }

                    let resolution = resolver.resolve().expect("resolution should succeed");
                    let tx = Transaction::from_resolution(resolution).expect("content store init");

                    // Note: check_file_conflicts requires actual file system access
                    // This benchmark measures transaction creation only
                    std::hint::black_box(tx)
                });
            },
        );
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_install_memory_allocation(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(50);

    // Test memory allocation patterns for different collection sizes
    let sizes = vec![10, 50, 100, 500, 1000];

    for size in sizes {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("vec_allocation", size), &size, |b, &n| {
            b.iter(|| {
                let mut vec = Vec::with_capacity(n);
                for i in 0..n {
                    vec.push(format!("package-{i}"));
                }
                std::hint::black_box(vec)
            });
        });

        group.bench_with_input(BenchmarkId::new("vec_no_capacity", size), &size, |b, &n| {
            b.iter(|| {
                let mut vec = Vec::new();
                for i in 0..n {
                    vec.push(format!("package-{i}"));
                }
                std::hint::black_box(vec)
            });
        });
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
criterion_group!(
    install_benches,
    bench_install_resolution,
    bench_install_transaction_creation,
    bench_install_dependency_graph_sizes,
    bench_install_batch_processing,
    bench_install_conflict_detection,
    bench_install_memory_allocation
);

#[cfg(any(feature = "debian", feature = "debian-pure"))]
criterion_main!(install_benches);

#[cfg(not(any(feature = "debian", feature = "debian-pure")))]
fn main() {
    println!("Install benchmarks require --features debian-pure or --features debian");
}
