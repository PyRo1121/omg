// Real-world benchmark comparing OMG operations to typical Debian package
// manager operations. Requires `apt-cache`, `dpkg`, `dpkg-query`, and an
// installed `omg` binary on PATH (Debian-family system); commands failing at
// runtime abort the benchmark instead of silently measuring spawn failures.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::process::Command;
use std::time::Duration;

/// Helper to run a command and capture its output
fn run_command(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|e| format!("Failed to execute {program}: {e}"))?;

    if output.status.success() {
        String::from_utf8(output.stdout).map_err(|e| format!("Invalid UTF-8: {e}"))
    } else {
        Err(format!("Command failed with status: {}", output.status))
    }
}

/// Run a command inside a benchmark iteration, aborting loudly on failure so a
/// missing/broken tool can never be silently benchmarked as a fast no-op.
fn must_run(program: &str, args: &[&str]) -> String {
    run_command(program, args)
        .unwrap_or_else(|error| panic!("benchmark command {program} {args:?} failed: {error}"))
}

/// Benchmark search operations
fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    // Small search query
    group.bench_with_input(BenchmarkId::new("apt-cache", "vim"), &"vim", |b, query| {
        b.iter(|| {
            black_box(must_run("apt-cache", &["search", black_box(query)]));
        });
    });

    group.bench_with_input(BenchmarkId::new("omg", "vim"), &"vim", |b, query| {
        b.iter(|| {
            black_box(must_run("omg", &["search", black_box(query)]));
        });
    });

    // Larger search query
    group.bench_with_input(
        BenchmarkId::new("apt-cache", "python"),
        &"python",
        |b, query| {
            b.iter(|| {
                black_box(must_run("apt-cache", &["search", black_box(query)]));
            });
        },
    );

    group.bench_with_input(BenchmarkId::new("omg", "python"), &"python", |b, query| {
        b.iter(|| {
            black_box(must_run("omg", &["search", black_box(query)]));
        });
    });

    group.finish();
}

/// Benchmark info/show operations
fn bench_info(c: &mut Criterion) {
    let mut group = c.benchmark_group("info");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    group.bench_with_input(BenchmarkId::new("apt-cache", "curl"), &"curl", |b, pkg| {
        b.iter(|| {
            black_box(must_run("apt-cache", &["show", black_box(pkg)]));
        });
    });

    group.bench_with_input(BenchmarkId::new("omg", "curl"), &"curl", |b, pkg| {
        b.iter(|| {
            black_box(must_run("omg", &["info", black_box(pkg)]));
        });
    });

    group.bench_with_input(BenchmarkId::new("apt-cache", "vim"), &"vim", |b, pkg| {
        b.iter(|| {
            black_box(must_run("apt-cache", &["show", black_box(pkg)]));
        });
    });

    group.bench_with_input(BenchmarkId::new("omg", "vim"), &"vim", |b, pkg| {
        b.iter(|| {
            black_box(must_run("omg", &["info", black_box(pkg)]));
        });
    });

    group.finish();
}

/// Benchmark list operations
fn bench_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("list_installed");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(10);

    group.bench_function("dpkg-l", |b| {
        b.iter(|| {
            black_box(must_run("dpkg", &["-l"]));
        });
    });

    group.bench_function("omg-list", |b| {
        b.iter(|| {
            black_box(must_run("omg", &["list"]));
        });
    });

    group.finish();
}

/// Benchmark package existence checks
fn bench_check_installed(c: &mut Criterion) {
    let mut group = c.benchmark_group("check_installed");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    group.bench_function("dpkg-query-curl", |b| {
        b.iter(|| {
            black_box(must_run("dpkg-query", &["-W", black_box("curl")]));
        });
    });

    group.bench_function("omg-info-curl", |b| {
        b.iter(|| {
            black_box(must_run("omg", &["info", black_box("curl")]));
        });
    });

    group.finish();
}

/// Benchmark policy/version checks
fn bench_policy(c: &mut Criterion) {
    let mut group = c.benchmark_group("policy");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(20);

    group.bench_with_input(BenchmarkId::new("apt-cache", "curl"), &"curl", |b, pkg| {
        b.iter(|| {
            black_box(must_run("apt-cache", &["policy", black_box(pkg)]));
        });
    });

    // OMG doesn't have a direct policy command, so we use info
    group.bench_with_input(BenchmarkId::new("omg-info", "curl"), &"curl", |b, pkg| {
        b.iter(|| {
            black_box(must_run("omg", &["info", black_box(pkg)]));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_search,
    bench_info,
    bench_list,
    bench_check_installed,
    bench_policy
);

criterion_main!(benches);
