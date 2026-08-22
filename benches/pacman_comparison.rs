//! Comparative Benchmarks: OMG vs Pacman
//!
//! Verifies performance claims by comparing OMG against pacman for common operations.
//!
//! Run with: `cargo bench --features arch --bench pacman_comparison`

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use std::process::Command;
use std::time::Duration;

fn omg_binary() -> String {
    std::env::var("OMG_BINARY").unwrap_or_else(|_| {
        let path = std::env::current_dir()
            .expect("current dir must be readable")
            .join("target/release/omg");
        path.to_str()
            .expect("binary path must be valid UTF-8")
            .to_string()
    })
}

/// Run `omg` and report success. Timing is criterion's job — it measures the
/// whole iteration, so no internal stopwatch is needed.
fn run_omg(args: &[&str]) -> bool {
    let output = Command::new(omg_binary())
        .args(args)
        .output()
        .expect("Failed to spawn omg; build it first or set OMG_BINARY");
    output.status.success()
}

/// Run `pacman` and report success.
fn run_pacman(args: &[&str]) -> bool {
    let output = Command::new("pacman")
        .args(args)
        .output()
        .expect("Failed to spawn pacman");
    output.status.success()
}

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    let queries = vec!["firefox", "vim", "kernel", "systemd"];

    for query in &queries {
        // OMG search
        group.bench_with_input(BenchmarkId::new("omg", query), query, |b, q| {
            b.iter(|| {
                assert!(run_omg(&["search", q]), "OMG search should succeed");
                black_box(());
            });
        });

        // Pacman search
        group.bench_with_input(BenchmarkId::new("pacman", query), query, |b, q| {
            b.iter(|| {
                assert!(run_pacman(&["-Ss", q]), "Pacman search should succeed");
                black_box(());
            });
        });
    }

    group.finish();
}

fn bench_info(c: &mut Criterion) {
    let mut group = c.benchmark_group("info");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    let packages = vec!["bash", "coreutils", "systemd", "linux"];

    for pkg in &packages {
        // OMG info
        group.bench_with_input(BenchmarkId::new("omg", pkg), pkg, |b, p| {
            b.iter(|| {
                assert!(run_omg(&["info", p]), "OMG info should succeed");
                black_box(());
            });
        });

        // Pacman info
        group.bench_with_input(BenchmarkId::new("pacman", pkg), pkg, |b, p| {
            b.iter(|| {
                assert!(run_pacman(&["-Si", p]), "Pacman info should succeed");
                black_box(());
            });
        });
    }

    group.finish();
}

fn bench_list(c: &mut Criterion) {
    let mut group = c.benchmark_group("list");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(20);

    // OMG explicit packages
    group.bench_function("omg_explicit", |b| {
        b.iter(|| {
            assert!(run_omg(&["explicit"]), "OMG explicit should succeed");
            black_box(());
        });
    });

    // Pacman explicit packages
    group.bench_function("pacman_explicit", |b| {
        b.iter(|| {
            assert!(run_pacman(&["-Qe"]), "Pacman -Qe should succeed");
            black_box(());
        });
    });

    group.finish();
}

fn bench_status(c: &mut Criterion) {
    let mut group = c.benchmark_group("status");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    // OMG status
    group.bench_function("omg_status", |b| {
        b.iter(|| {
            assert!(run_omg(&["status"]), "OMG status should succeed");
            black_box(());
        });
    });

    // Pacman list count
    group.bench_function("pacman_list_count", |b| {
        b.iter(|| {
            assert!(run_pacman(&["-Q"]), "Pacman -Q should succeed");
            black_box(());
        });
    });

    group.finish();
}

fn bench_update_check(c: &mut Criterion) {
    let mut group = c.benchmark_group("update_check");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(10); // Fewer samples for potentially slower operation

    // OMG update check
    group.bench_function("omg_update_check", |b| {
        b.iter(|| {
            assert!(
                run_omg(&["update", "--check"]),
                "OMG update check should succeed"
            );
            black_box(());
        });
    });

    // Pacman checkupdates (if available)
    if Command::new("checkupdates").output().is_ok() {
        group.bench_function("checkupdates", |b| {
            b.iter(|| {
                let output = Command::new("checkupdates")
                    .output()
                    .expect("Failed to spawn checkupdates");
                assert!(output.status.success(), "checkupdates should succeed");
                black_box(output);
            });
        });
    }

    group.finish();
}

/// Comprehensive comparison report
fn bench_comprehensive_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("comprehensive");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10);

    // Workflow: search + info + check installed
    group.bench_function("omg_workflow", |b| {
        b.iter(|| {
            run_omg(&["search", "firefox"]);
            run_omg(&["info", "bash"]);
            run_omg(&["status"]);
            black_box(());
        });
    });

    group.bench_function("pacman_workflow", |b| {
        b.iter(|| {
            run_pacman(&["-Ss", "firefox"]);
            run_pacman(&["-Si", "bash"]);
            run_pacman(&["-Q"]);
            black_box(());
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_search,
    bench_info,
    bench_list,
    bench_status,
    bench_update_check,
    bench_comprehensive_comparison
);
criterion_main!(benches);
