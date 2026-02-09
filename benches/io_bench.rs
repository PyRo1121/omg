//! I/O Performance Benchmarks for Debian Operations
//!
//! Compares different I/O strategies:
//! - Direct write vs buffered vs mmap
//! - Streaming vs buffered downloads
//! - File size impact on different strategies
//!
//! Run with: `cargo bench --features debian-pure --bench io_bench`

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Duration;
use tempfile::TempDir;

fn bench_write_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_write");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    // Test different file sizes: 1KB, 10KB, 100KB, 1MB, 10MB
    let sizes = vec![
        (1 * 1024, "1KB"),
        (10 * 1024, "10KB"),
        (100 * 1024, "100KB"),
        (1024 * 1024, "1MB"),
        (10 * 1024 * 1024, "10MB"),
    ];

    for (size, name) in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        // Generate test data
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

        // Direct write (unbuffered)
        group.bench_with_input(
            BenchmarkId::new("direct_write", name),
            &data,
            |b, test_data| {
                let temp_dir = TempDir::new().unwrap();

                b.iter(|| {
                    let path = temp_dir.path().join("direct.bin");
                    let mut file = File::create(&path).unwrap();
                    file.write_all(test_data).unwrap();
                    file.sync_all().unwrap();
                    std::hint::black_box(&path);
                });
            },
        );

        // Buffered write
        group.bench_with_input(
            BenchmarkId::new("buffered_write", name),
            &data,
            |b, test_data| {
                let temp_dir = TempDir::new().unwrap();

                b.iter(|| {
                    let path = temp_dir.path().join("buffered.bin");
                    let file = File::create(&path).unwrap();
                    let mut writer = BufWriter::new(file);
                    writer.write_all(test_data).unwrap();
                    writer.flush().unwrap();
                    std::hint::black_box(&path);
                });
            },
        );

        // Buffered write with custom buffer size
        group.bench_with_input(
            BenchmarkId::new("buffered_write_64k", name),
            &data,
            |b, test_data| {
                let temp_dir = TempDir::new().unwrap();

                b.iter(|| {
                    let path = temp_dir.path().join("buffered_64k.bin");
                    let file = File::create(&path).unwrap();
                    let mut writer = BufWriter::with_capacity(64 * 1024, file);
                    writer.write_all(test_data).unwrap();
                    writer.flush().unwrap();
                    std::hint::black_box(&path);
                });
            },
        );

        // Chunked write (simulate streaming)
        group.bench_with_input(
            BenchmarkId::new("chunked_write_4k", name),
            &data,
            |b, test_data| {
                let temp_dir = TempDir::new().unwrap();

                b.iter(|| {
                    let path = temp_dir.path().join("chunked.bin");
                    let mut file = File::create(&path).unwrap();

                    for chunk in test_data.chunks(4096) {
                        file.write_all(chunk).unwrap();
                    }
                    file.sync_all().unwrap();
                    std::hint::black_box(&path);
                });
            },
        );
    }

    group.finish();
}

fn bench_read_strategies(c: &mut Criterion) {
    use std::io::{BufReader, Read};

    let mut group = c.benchmark_group("io_read");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    let sizes = vec![
        (10 * 1024, "10KB"),
        (100 * 1024, "100KB"),
        (1024 * 1024, "1MB"),
        (10 * 1024 * 1024, "10MB"),
    ];

    for (size, name) in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        // Create test file
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.bin");
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        std::fs::write(&test_file, &data).unwrap();

        // Direct read
        group.bench_with_input(
            BenchmarkId::new("direct_read", name),
            &test_file,
            |b, path| {
                b.iter(|| {
                    let mut file = File::open(path).unwrap();
                    let mut buffer = Vec::new();
                    file.read_to_end(&mut buffer).unwrap();
                    std::hint::black_box(buffer)
                });
            },
        );

        // Buffered read
        group.bench_with_input(
            BenchmarkId::new("buffered_read", name),
            &test_file,
            |b, path| {
                b.iter(|| {
                    let file = File::open(path).unwrap();
                    let mut reader = BufReader::new(file);
                    let mut buffer = Vec::new();
                    reader.read_to_end(&mut buffer).unwrap();
                    std::hint::black_box(buffer)
                });
            },
        );

        // Buffered read with custom buffer size
        group.bench_with_input(
            BenchmarkId::new("buffered_read_64k", name),
            &test_file,
            |b, path| {
                b.iter(|| {
                    let file = File::open(path).unwrap();
                    let mut reader = BufReader::with_capacity(64 * 1024, file);
                    let mut buffer = Vec::new();
                    reader.read_to_end(&mut buffer).unwrap();
                    std::hint::black_box(buffer)
                });
            },
        );

        // Memory-mapped read
        group.bench_with_input(
            BenchmarkId::new("mmap_read", name),
            &test_file,
            |b, path| {
                b.iter(|| {
                    let file = File::open(path).unwrap();
                    // SAFETY: File is opened read-only and mmap lifetime is bounded by this closure
                    #[allow(unsafe_code)]
                    let mmap = unsafe { memmap2::Mmap::map(&file).unwrap() };
                    let len = mmap.len();
                    std::hint::black_box(len)
                });
            },
        );

        // Chunked read
        group.bench_with_input(
            BenchmarkId::new("chunked_read_4k", name),
            &test_file,
            |b, path| {
                b.iter(|| {
                    let mut file = File::open(path).unwrap();
                    let mut buffer = Vec::with_capacity(size);
                    let mut chunk = vec![0u8; 4096];

                    loop {
                        match file.read(&mut chunk) {
                            Ok(0) => break,
                            Ok(n) => buffer.extend_from_slice(&chunk[..n]),
                            Err(_) => break,
                        }
                    }
                    std::hint::black_box(buffer)
                });
            },
        );
    }

    group.finish();
}

fn bench_async_io(c: &mut Criterion) {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("io_async");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    let sizes = vec![
        (10 * 1024, "10KB"),
        (100 * 1024, "100KB"),
        (1024 * 1024, "1MB"),
    ];

    for (size, name) in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

        // Async write
        group.bench_with_input(
            BenchmarkId::new("async_write", name),
            &data,
            |b, test_data| {
                let temp_dir = TempDir::new().unwrap();

                b.iter(|| {
                    rt.block_on(async {
                        let path = temp_dir.path().join("async.bin");
                        let mut file = tokio::fs::File::create(&path).await.unwrap();
                        file.write_all(test_data).await.unwrap();
                        file.flush().await.unwrap();
                        std::hint::black_box(test_data.len())
                    })
                });
            },
        );

        // Create test file for read benchmarks
        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.bin");
        std::fs::write(&test_file, &data).unwrap();

        // Async read
        group.bench_with_input(
            BenchmarkId::new("async_read", name),
            &test_file,
            |b, path| {
                b.iter(|| {
                    rt.block_on(async {
                        let mut file = tokio::fs::File::open(path).await.unwrap();
                        let mut buffer = Vec::new();
                        file.read_to_end(&mut buffer).await.unwrap();
                        std::hint::black_box(buffer)
                    })
                });
            },
        );

        // Async buffered read
        group.bench_with_input(
            BenchmarkId::new("async_buffered_read", name),
            &test_file,
            |b, path| {
                b.iter(|| {
                    rt.block_on(async {
                        use tokio::io::BufReader;

                        let file = tokio::fs::File::open(path).await.unwrap();
                        let mut reader = BufReader::new(file);
                        let mut buffer = Vec::new();
                        reader.read_to_end(&mut buffer).await.unwrap();
                        std::hint::black_box(buffer)
                    })
                });
            },
        );
    }

    group.finish();
}

fn bench_copy_strategies(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_copy");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    let sizes = vec![
        (100 * 1024, "100KB"),
        (1024 * 1024, "1MB"),
        (10 * 1024 * 1024, "10MB"),
    ];

    for (size, name) in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        // Create source file
        let temp_dir = TempDir::new().unwrap();
        let src = temp_dir.path().join("src.bin");
        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();
        std::fs::write(&src, &data).unwrap();

        // std::fs::copy
        group.bench_with_input(BenchmarkId::new("fs_copy", name), &src, |b, src_path| {
            let dest_dir = TempDir::new().unwrap();

            b.iter(|| {
                let dest = dest_dir.path().join("dest.bin");
                std::fs::copy(src_path, &dest).unwrap();
                std::hint::black_box(&dest);
            });
        });

        // std::io::copy
        group.bench_with_input(BenchmarkId::new("io_copy", name), &src, |b, src_path| {
            let dest_dir = TempDir::new().unwrap();

            b.iter(|| {
                use std::io::copy;

                let dest = dest_dir.path().join("dest.bin");
                let mut src_file = File::open(src_path).unwrap();
                let mut dest_file = File::create(&dest).unwrap();
                copy(&mut src_file, &mut dest_file).unwrap();
                std::hint::black_box(&dest);
            });
        });

        // Buffered copy
        group.bench_with_input(
            BenchmarkId::new("buffered_copy", name),
            &src,
            |b, src_path| {
                use std::io::{BufReader, BufWriter, copy};

                let dest_dir = TempDir::new().unwrap();

                b.iter(|| {
                    let dest = dest_dir.path().join("dest.bin");
                    let src_file = File::open(src_path).unwrap();
                    let dest_file = File::create(&dest).unwrap();
                    let mut reader = BufReader::new(src_file);
                    let mut writer = BufWriter::new(dest_file);
                    copy(&mut reader, &mut writer).unwrap();
                    std::hint::black_box(&dest);
                });
            },
        );
    }

    group.finish();
}

fn bench_stream_processing(c: &mut Criterion) {
    use std::io::Cursor;

    let mut group = c.benchmark_group("io_stream");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    let sizes = vec![
        (10 * 1024, "10KB"),
        (100 * 1024, "100KB"),
        (1024 * 1024, "1MB"),
    ];

    for (size, name) in sizes {
        group.throughput(Throughput::Bytes(size as u64));

        let data: Vec<u8> = (0..size).map(|i| (i % 256) as u8).collect();

        // Process in one go
        group.bench_with_input(
            BenchmarkId::new("all_at_once", name),
            &data,
            |b, test_data| {
                b.iter(|| {
                    let result: Vec<u8> = test_data.iter().map(|&b| b.wrapping_add(1)).collect();
                    std::hint::black_box(result)
                });
            },
        );

        // Process in 4KB chunks
        group.bench_with_input(
            BenchmarkId::new("chunked_4k", name),
            &data,
            |b, test_data| {
                b.iter(|| {
                    let mut result = Vec::with_capacity(test_data.len());
                    for chunk in test_data.chunks(4096) {
                        let processed: Vec<u8> = chunk.iter().map(|&b| b.wrapping_add(1)).collect();
                        result.extend(processed);
                    }
                    std::hint::black_box(result)
                });
            },
        );

        // Cursor-based processing
        group.bench_with_input(BenchmarkId::new("cursor", name), &data, |b, test_data| {
            use std::io::Read;

            b.iter(|| {
                let mut cursor = Cursor::new(test_data);
                let mut buffer = Vec::new();
                cursor.read_to_end(&mut buffer).unwrap();
                let result: Vec<u8> = buffer.iter().map(|&b| b.wrapping_add(1)).collect();
                std::hint::black_box(result)
            });
        });
    }

    group.finish();
}

fn bench_buffer_sizes(c: &mut Criterion) {
    let mut group = c.benchmark_group("io_buffer_size");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(30);

    let data_size = 1024 * 1024; // 1MB
    let data: Vec<u8> = (0..data_size).map(|i| (i % 256) as u8).collect();

    // Test different buffer sizes: 4KB, 8KB, 16KB, 32KB, 64KB, 128KB
    let buffer_sizes = vec![
        4 * 1024,
        8 * 1024,
        16 * 1024,
        32 * 1024,
        64 * 1024,
        128 * 1024,
    ];

    for buf_size in buffer_sizes {
        group.throughput(Throughput::Bytes(data_size as u64));
        group.bench_with_input(
            BenchmarkId::new("write", buf_size),
            &buf_size,
            |b, &size| {
                let temp_dir = TempDir::new().unwrap();

                b.iter(|| {
                    let path = temp_dir.path().join("buffer_test.bin");
                    let file = File::create(&path).unwrap();
                    let mut writer = BufWriter::with_capacity(size, file);
                    writer.write_all(&data).unwrap();
                    writer.flush().unwrap();
                    std::hint::black_box(&path);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    io_benches,
    bench_write_strategies,
    bench_read_strategies,
    bench_async_io,
    bench_copy_strategies,
    bench_stream_processing,
    bench_buffer_sizes
);

criterion_main!(io_benches);
