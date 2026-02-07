//! Comparative Benchmarks: OMG vs Pacman
//!
//! Verifies performance claims by comparing OMG against pacman for common operations.
//!
//! Run with: cargo bench --features arch --bench pacman_comparison

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use std::process::Command;
use std::time::Duration;

fn omg_binary() -> String {
    std::env::var("OMG_BINARY").unwrap_or_else(|_| {
        let path = std::env::current_dir()
            .unwrap()
            .join("target/release/omg");
        path.to_str().unwrap().to_string()
    })
}

fn run_omg(args: &[&str]) -> (bool, Duration) {
    let start = std::time::Instant::now();
    let output = Command::new(omg_binary())
        .args(args)
        .output()
        .expect("Failed to run omg");
    let duration = start.elapsed();
    (output.status.success(), duration)
}

fn run_pacman(args: &[&str]) -> (bool, Duration) {
    let start = std::time::Instant::now();
    let output = Command::new("pacman")
        .args(args)
        .output()
        .expect("Failed to run pacman");
    let duration = start.elapsed();
    (output.status.success(), duration)
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
                let (success, _duration) = run_omg(&["search", q]);
                assert!(success, "OMG search should succeed");
                black_box(())
            });
        });

        // Pacman search
        group.bench_with_input(BenchmarkId::new("pacman", query), query, |b, q| {
            b.iter(|| {
                let (success, _duration) = run_pacman(&["-Ss", q]);
                assert!(success, "Pacman search should succeed");
                black_box(())
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
                let (success, _duration) = run_omg(&["info", p]);
                assert!(success, "OMG info should succeed");
                black_box(())
            });
        });

        // Pacman info
        group.bench_with_input(BenchmarkId::new("pacman", pkg), pkg, |b, p| {
            b.iter(|| {
                let (success, _duration) = run_pacman(&["-Si", p]);
                assert!(success, "Pacman info should succeed");
                black_box(())
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
            let (success, _duration) = run_omg(&["explicit"]);
            assert!(success, "OMG explicit should succeed");
            black_box(())
        });
    });

    // Pacman explicit packages
    group.bench_function("pacman_explicit", |b| {
        b.iter(|| {
            let (success, _duration) = run_pacman(&["-Qe"]);
            assert!(success, "Pacman -Qe should succeed");
            black_box(())
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
            let (success, _duration) = run_omg(&["status"]);
            assert!(success, "OMG status should succeed");
            black_box(())
        });
    });

    // Pacman list count
    group.bench_function("pacman_list_count", |b| {
        b.iter(|| {
            let (success, _duration) = run_pacman(&["-Q"]);
            assert!(success, "Pacman -Q should succeed");
            black_box(())
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
            let (success, _duration) = run_omg(&["update", "--check"]);
            assert!(success, "OMG update check should succeed");
            black_box(())
        });
    });

    // Pacman checkupdates (if available)
    if Command::new("checkupdates").output().is_ok() {
        group.bench_function("checkupdates", |b| {
            b.iter(|| {
                let start = std::time::Instant::now();
                let _ = Command::new("checkupdates").output();
                black_box(start.elapsed())
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
            black_box(())
        });
    });

    group.bench_function("pacman_workflow", |b| {
        b.iter(|| {
            run_pacman(&["-Ss", "firefox"]);
            run_pacman(&["-Si", "bash"]);
            run_pacman(&["-Q"]);
            black_box(())
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
