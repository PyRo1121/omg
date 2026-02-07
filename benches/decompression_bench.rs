//! Decompression Performance Benchmarks
//!
//! Compares different compression formats for .deb data extraction.
//! Run with: `cargo bench --features debian-pure --bench decompression_bench`

#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};

#[cfg(any(feature = "debian", feature = "debian-pure"))]
use std::io::Read;
#[cfg(any(feature = "debian", feature = "debian-pure"))]
use std::time::Duration;

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn create_test_data() -> Vec<u8> {
    // Create a 1MB test file (typical .deb data.tar size)
    let mut data = Vec::with_capacity(1024 * 1024);
    for i in 0..1024 {
        data.extend_from_slice(format!("Test data line {i}\n").repeat(64).as_bytes());
    }
    data
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_xz_decompress(c: &mut Criterion) {
    let test_data = create_test_data();

    // Compress with xz
    let mut compressed = Vec::new();
    lzma_rs::xz_compress(&mut std::io::Cursor::new(&test_data), &mut compressed)
        .expect("compression failed");

    let mut group = c.benchmark_group("xz_decompression");
    group.throughput(Throughput::Bytes(compressed.len() as u64));
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("lzma_rs_xz_decompress", |b| {
        b.iter(|| {
            let mut decompressed = Vec::new();
            lzma_rs::xz_decompress(&mut std::io::Cursor::new(&compressed), &mut decompressed)
                .expect("decompression failed");
            std::hint::black_box(decompressed)
        });
    });

    // Benchmark with pre-allocated buffer (current optimization)
    group.bench_function("lzma_rs_xz_decompress_preallocated", |b| {
        b.iter(|| {
            let mut decompressed = Vec::with_capacity(compressed.len() * 7);
            lzma_rs::xz_decompress(&mut std::io::Cursor::new(&compressed), &mut decompressed)
                .expect("decompression failed");
            std::hint::black_box(decompressed)
        });
    });

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_zstd_decompress(c: &mut Criterion) {
    let test_data = create_test_data();

    // Compress with zstd (level 3 - default)
    let compressed =
        zstd::encode_all(std::io::Cursor::new(&test_data), 3).expect("compression failed");

    let mut group = c.benchmark_group("zstd_decompression");
    group.throughput(Throughput::Bytes(compressed.len() as u64));
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("ruzstd_decompress", |b| {
        b.iter(|| {
            let mut decoder =
                ruzstd::decoding::StreamingDecoder::new(std::io::Cursor::new(&compressed))
                    .expect("decoder creation failed");
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .expect("decompression failed");
            std::hint::black_box(decompressed)
        });
    });

    // Benchmark with pre-allocated buffer
    group.bench_function("ruzstd_decompress_preallocated", |b| {
        b.iter(|| {
            let mut decoder =
                ruzstd::decoding::StreamingDecoder::new(std::io::Cursor::new(&compressed))
                    .expect("decoder creation failed");
            let mut decompressed = Vec::with_capacity(compressed.len() * 5);
            decoder
                .read_to_end(&mut decompressed)
                .expect("decompression failed");
            std::hint::black_box(decompressed)
        });
    });

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_gzip_decompress(c: &mut Criterion) {
    let test_data = create_test_data();

    // Compress with gzip
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::copy(&mut std::io::Cursor::new(&test_data), &mut encoder).expect("compression failed");
    let compressed = encoder.finish().expect("finish failed");

    let mut group = c.benchmark_group("gzip_decompression");
    group.throughput(Throughput::Bytes(compressed.len() as u64));
    group.measurement_time(Duration::from_secs(5));

    group.bench_function("flate2_gzip_decompress", |b| {
        b.iter(|| {
            let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
            let mut decompressed = Vec::new();
            decoder
                .read_to_end(&mut decompressed)
                .expect("decompression failed");
            std::hint::black_box(decompressed)
        });
    });

    // Benchmark with pre-allocated buffer
    group.bench_function("flate2_gzip_decompress_preallocated", |b| {
        b.iter(|| {
            let mut decoder = flate2::read::GzDecoder::new(&compressed[..]);
            let mut decompressed = Vec::with_capacity(compressed.len() * 4);
            decoder
                .read_to_end(&mut decompressed)
                .expect("decompression failed");
            std::hint::black_box(decompressed)
        });
    });

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
fn bench_compression_formats_comparison(c: &mut Criterion) {
    let test_data = create_test_data();

    // Create compressed versions
    let mut xz_compressed = Vec::new();
    lzma_rs::xz_compress(&mut std::io::Cursor::new(&test_data), &mut xz_compressed)
        .expect("xz compression failed");

    let zstd_compressed =
        zstd::encode_all(std::io::Cursor::new(&test_data), 3).expect("zstd compression failed");

    let mut gzip_encoder =
        flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    std::io::copy(&mut std::io::Cursor::new(&test_data), &mut gzip_encoder)
        .expect("gzip compression failed");
    let gzip_compressed = gzip_encoder.finish().expect("gzip finish failed");

    let mut group = c.benchmark_group("compression_format_comparison");
    group.measurement_time(Duration::from_secs(10));

    // Report compression ratios
    println!(
        "\nCompression ratios (1MB test data):\n  XZ:   {:.2}x ({} bytes)\n  Zstd: {:.2}x ({} bytes)\n  Gzip: {:.2}x ({} bytes)",
        test_data.len() as f64 / xz_compressed.len() as f64,
        xz_compressed.len(),
        test_data.len() as f64 / zstd_compressed.len() as f64,
        zstd_compressed.len(),
        test_data.len() as f64 / gzip_compressed.len() as f64,
        gzip_compressed.len()
    );

    group.bench_with_input(
        BenchmarkId::new("decompress", "xz"),
        &xz_compressed,
        |b, data| {
            b.iter(|| {
                let mut decompressed = Vec::with_capacity(data.len() * 7);
                lzma_rs::xz_decompress(&mut std::io::Cursor::new(data), &mut decompressed)
                    .expect("decompression failed");
                std::hint::black_box(decompressed)
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("decompress", "zstd"),
        &zstd_compressed,
        |b, data| {
            b.iter(|| {
                let mut decoder =
                    ruzstd::decoding::StreamingDecoder::new(std::io::Cursor::new(data))
                        .expect("decoder creation failed");
                let mut decompressed = Vec::with_capacity(data.len() * 5);
                decoder
                    .read_to_end(&mut decompressed)
                    .expect("decompression failed");
                std::hint::black_box(decompressed)
            });
        },
    );

    group.bench_with_input(
        BenchmarkId::new("decompress", "gzip"),
        &gzip_compressed,
        |b, data| {
            b.iter(|| {
                let mut decoder = flate2::read::GzDecoder::new(&data[..]);
                let mut decompressed = Vec::with_capacity(data.len() * 4);
                decoder
                    .read_to_end(&mut decompressed)
                    .expect("decompression failed");
                std::hint::black_box(decompressed)
            });
        },
    );

    group.finish();
}

#[cfg(any(feature = "debian", feature = "debian-pure"))]
criterion_group!(
    decompression_benches,
    bench_xz_decompress,
    bench_zstd_decompress,
    bench_gzip_decompress,
    bench_compression_formats_comparison
);

#[cfg(any(feature = "debian", feature = "debian-pure"))]
criterion_main!(decompression_benches);

#[cfg(not(any(feature = "debian", feature = "debian-pure")))]
fn main() {
    println!("Decompression benchmarks require --features debian-pure or --features debian");
}
