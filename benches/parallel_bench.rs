//! Debian Parallel Operations Benchmarks
//!
//! Tests different concurrency levels for downloads and unpacking to determine
//! optimal settings for `MAX_CONCURRENT` values.
//!
//! Run with: `cargo bench --features debian-pure --bench parallel_bench`

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use std::time::Duration;

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_parallel_stream_buffering(c: &mut Criterion) {
    use futures::stream::{self, StreamExt};

    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("parallel_stream");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(30);

    // Test different buffer_unordered sizes
    let concurrency_levels = vec![8, 16, 24, 32, 48, 64];
    let item_count = 100;

    for concurrency in concurrency_levels {
        group.throughput(Throughput::Elements(item_count as u64));
        group.bench_with_input(
            BenchmarkId::new("buffer_unordered", concurrency),
            &concurrency,
            |b, &n| {
                b.iter(|| {
                    rt.block_on(async {
                        let items: Vec<usize> = (0..item_count).collect();

                        let results: Vec<usize> = stream::iter(items)
                            .map(|i| async move {
                                // Simulate async work (lightweight)
                                tokio::time::sleep(Duration::from_micros(10)).await;
                                i * 2
                            })
                            .buffer_unordered(n)
                            .collect()
                            .await;

                        std::hint::black_box(results)
                    })
                });
            },
        );
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_parallel_rayon_processing(c: &mut Criterion) {
    use rayon::prelude::*;

    let mut group = c.benchmark_group("parallel_rayon");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(30);

    // Test different rayon thread pool configurations
    let item_counts = vec![10, 50, 100, 500, 1000];

    for count in item_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("par_iter_map", count), &count, |b, &n| {
            b.iter(|| {
                let items: Vec<usize> = (0..n).collect();

                let results: Vec<usize> = items
                    .par_iter()
                    .map(|&i| {
                        // Simulate CPU work
                        let mut x = i;
                        for _ in 0..1000 {
                            x = x.wrapping_mul(31).wrapping_add(17);
                        }
                        x
                    })
                    .collect();

                std::hint::black_box(results)
            });
        });

        group.bench_with_input(
            BenchmarkId::new("sequential_map", count),
            &count,
            |b, &n| {
                b.iter(|| {
                    let items: Vec<usize> = (0..n).collect();

                    let results: Vec<usize> = items
                        .iter()
                        .map(|&i| {
                            // Simulate CPU work
                            let mut x = i;
                            for _ in 0..1000 {
                                x = x.wrapping_mul(31).wrapping_add(17);
                            }
                            x
                        })
                        .collect();

                    std::hint::black_box(results)
                });
            },
        );
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_parallel_batch_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("parallel_batching");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(30);

    // Test different batch sizes for processing
    let batch_sizes = vec![4, 8, 16, 32, 64];
    let total_items = 256;

    for batch_size in batch_sizes {
        group.throughput(Throughput::Elements(total_items as u64));
        group.bench_with_input(
            BenchmarkId::new("batched_processing", batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    let items: Vec<usize> = (0..total_items).collect();
                    let mut results = Vec::with_capacity(total_items);

                    for chunk in items.chunks(size) {
                        let batch_results: Vec<usize> =
                            chunk.iter().map(|&i| i.wrapping_mul(2)).collect();
                        results.extend(batch_results);
                    }

                    std::hint::black_box(results)
                });
            },
        );
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_parallel_channel_throughput(c: &mut Criterion) {
    use tokio::sync::mpsc;

    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("parallel_channels");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(30);

    // Test different channel buffer sizes
    let buffer_sizes = vec![1, 8, 16, 32, 64, 128];
    let message_count = 1000;

    for buffer_size in buffer_sizes {
        group.throughput(Throughput::Elements(message_count as u64));
        group.bench_with_input(
            BenchmarkId::new("mpsc_channel", buffer_size),
            &buffer_size,
            |b, &size| {
                b.iter(|| {
                    rt.block_on(async {
                        let (tx, mut rx) = mpsc::channel::<usize>(size);

                        // Spawn sender task
                        let sender = tokio::spawn(async move {
                            for i in 0..message_count {
                                let _ = tx.send(i).await;
                            }
                        });

                        // Receive all messages
                        let mut received = 0;
                        while rx.recv().await.is_some() {
                            received += 1;
                            if received >= message_count {
                                break;
                            }
                        }

                        sender.await.unwrap();
                        std::hint::black_box(received)
                    })
                });
            },
        );
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_parallel_concurrent_downloads(c: &mut Criterion) {
    use futures::stream::{self, StreamExt};

    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("concurrent_downloads");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(20);

    let concurrency_levels = vec![8, 16, 24, 32, 48, 64];
    let download_count = 50;

    for concurrency in concurrency_levels {
        group.throughput(Throughput::Elements(download_count as u64));
        group.bench_with_input(
            BenchmarkId::new("simulated_downloads", concurrency),
            &concurrency,
            |b, &n| {
                b.iter(|| {
                    rt.block_on(async {
                        let urls: Vec<usize> = (0..download_count).collect();

                        let results: Vec<Vec<u8>> = stream::iter(urls)
                            .map(|i| async move {
                                // Simulate download latency + data transfer
                                tokio::time::sleep(Duration::from_millis(5)).await;

                                // Simulate downloaded data
                                vec![0u8; 1024 * i.min(100)]
                            })
                            .buffer_unordered(n)
                            .collect()
                            .await;

                        std::hint::black_box(results)
                    })
                });
            },
        );
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_parallel_decompression_workers(c: &mut Criterion) {
    use rayon::prelude::*;

    let mut group = c.benchmark_group("decompression_workers");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(20);

    // Simulate parallel decompression of packages
    let worker_counts = vec![4, 8, 12, 16, 20, 24];
    let package_count = 32;

    for workers in worker_counts {
        group.throughput(Throughput::Elements(package_count as u64));
        group.bench_with_input(
            BenchmarkId::new("parallel_decompress", workers),
            &workers,
            |b, &n| {
                b.iter(|| {
                    rayon::ThreadPoolBuilder::new()
                        .num_threads(n)
                        .build()
                        .unwrap()
                        .install(|| {
                            let packages: Vec<usize> = (0..package_count).collect();

                            let results: Vec<Vec<u8>> = packages
                                .par_iter()
                                .map(|&i| {
                                    // Simulate decompression work (CPU-intensive)
                                    let mut data = Vec::with_capacity(1024);
                                    for j in 0..1024 {
                                        #[expect(clippy::cast_possible_truncation)]
                                        let value = ((i * 31) + j) as u8;
                                        data.push(value);
                                    }

                                    // Simulate compression/decompression work
                                    for _ in 0..100 {
                                        data.sort_unstable();
                                    }

                                    data
                                })
                                .collect();

                            std::hint::black_box(results)
                        })
                });
            },
        );
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_parallel_task_spawning(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("task_spawning");
    group.measurement_time(Duration::from_secs(15));
    group.sample_size(30);

    let task_counts = vec![10, 50, 100, 200];

    for count in task_counts {
        group.throughput(Throughput::Elements(count as u64));
        group.bench_with_input(BenchmarkId::new("spawn_join", count), &count, |b, &n| {
            b.iter(|| {
                rt.block_on(async {
                    let mut handles = Vec::with_capacity(n);

                    for i in 0..n {
                        let handle = tokio::spawn(async move {
                            // Simulate lightweight task
                            tokio::time::sleep(Duration::from_micros(10)).await;
                            i * 2
                        });
                        handles.push(handle);
                    }

                    let results: Vec<usize> = futures::future::join_all(handles)
                        .await
                        .into_iter()
                        .flatten()
                        .collect();

                    std::hint::black_box(results)
                })
            });
        });

        group.bench_with_input(BenchmarkId::new("spawn_channel", count), &count, |b, &n| {
            b.iter(|| {
                rt.block_on(async {
                    use tokio::sync::mpsc;

                    let (tx, mut rx) = mpsc::channel::<usize>(n);

                    for i in 0..n {
                        let tx = tx.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(Duration::from_micros(10)).await;
                            let _ = tx.send(i * 2).await;
                        });
                    }
                    drop(tx);

                    let mut results = Vec::with_capacity(n);
                    while let Some(value) = rx.recv().await {
                        results.push(value);
                    }

                    std::hint::black_box(results)
                })
            });
        });
    }

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
criterion_group!(
    parallel_benches,
    bench_parallel_stream_buffering,
    bench_parallel_rayon_processing,
    bench_parallel_batch_sizes,
    bench_parallel_channel_throughput,
    bench_parallel_concurrent_downloads,
    bench_parallel_decompression_workers,
    bench_parallel_task_spawning
);

#[cfg(any(feature = "debian", feature = "debian-pure"))]
criterion_main!(parallel_benches);

#[cfg(not(any(feature = "debian", feature = "debian-pure")))]
fn main() {
    println!("Parallel benchmarks require --features debian-pure or --features debian");
}
