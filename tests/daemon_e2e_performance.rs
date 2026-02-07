#![cfg(feature = "arch")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]

//! S-tier E2E Tests: Daemon Performance Benchmarks
//!
//! Comprehensive performance tests for latency, throughput, memory usage,
//! cache effectiveness, and index performance under various load conditions.

use anyhow::Result;
use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::protocol::{Request, Response, ResponseResult};
use serial_test::serial;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Performance test fixture with timing utilities
struct PerformanceTestFixture {
    _temp_dir: TempDir,
    state: Arc<DaemonState>,
}

impl PerformanceTestFixture {
    async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&data_dir)?;

        #[expect(unsafe_code)]
        unsafe {
            std::env::set_var("OMG_DAEMON_DATA_DIR", &data_dir);
            std::env::set_var("OMG_DATA_DIR", &data_dir);
        }

        omg_lib::core::security::init_audit_logger()?;

        let state = Arc::new(DaemonState::new()?);

        Ok(Self {
            _temp_dir: temp_dir,
            state,
        })
    }

    async fn measure_request(&self, request: Request) -> (Response, Duration) {
        let start = Instant::now();
        let response = handle_request(Arc::clone(&self.state), request).await;
        let duration = start.elapsed();
        (response, duration)
    }

    #[expect(dead_code)]
    async fn measure_batch(&self, requests: Vec<Request>) -> (Vec<Response>, Duration) {
        let start = Instant::now();
        let mut responses = Vec::with_capacity(requests.len());
        for req in requests {
            let resp = handle_request(Arc::clone(&self.state), req).await;
            responses.push(resp);
        }
        let duration = start.elapsed();
        (responses, duration)
    }
}

/// Performance statistics helper
struct PerfStats {
    measurements: Vec<Duration>,
}

impl PerfStats {
    fn new() -> Self {
        Self {
            measurements: Vec::new(),
        }
    }

    fn record(&mut self, duration: Duration) {
        self.measurements.push(duration);
    }

    fn mean(&self) -> Duration {
        let sum: Duration = self.measurements.iter().sum();
        sum / self.measurements.len() as u32
    }

    fn median(&self) -> Duration {
        let mut sorted = self.measurements.clone();
        sorted.sort();
        sorted[sorted.len() / 2]
    }

    fn p95(&self) -> Duration {
        let mut sorted = self.measurements.clone();
        sorted.sort();
        sorted[(sorted.len() as f64 * 0.95) as usize]
    }

    fn p99(&self) -> Duration {
        let mut sorted = self.measurements.clone();
        sorted.sort();
        sorted[(sorted.len() as f64 * 0.99) as usize]
    }

    fn max(&self) -> Duration {
        *self.measurements.iter().max().unwrap()
    }

    fn min(&self) -> Duration {
        *self.measurements.iter().min().unwrap()
    }

    fn print_summary(&self, label: &str) {
        println!("\n{} Performance Statistics:", label);
        println!("  Mean:   {:?}", self.mean());
        println!("  Median: {:?}", self.median());
        println!("  P95:    {:?}", self.p95());
        println!("  P99:    {:?}", self.p99());
        println!("  Min:    {:?}", self.min());
        println!("  Max:    {:?}", self.max());
    }
}

// ============================================================================
// Test 1: Latency Benchmarks
// ============================================================================

#[tokio::test]
#[serial]
async fn bench_ping_latency() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;
    let mut stats = PerfStats::new();

    // Warmup
    for i in 0..10 {
        fixture.measure_request(Request::Ping { id: i }).await;
    }

    // Measure
    for i in 0..100 {
        let (response, duration) = fixture.measure_request(Request::Ping { id: i }).await;

        assert!(
            matches!(response, Response::Success { .. }),
            "Ping should succeed"
        );
        stats.record(duration);
    }

    stats.print_summary("Ping");

    // Assertions
    assert!(
        stats.mean() < Duration::from_millis(5),
        "Mean ping latency should be < 5ms"
    );
    assert!(
        stats.p99() < Duration::from_millis(10),
        "P99 ping latency should be < 10ms"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn bench_search_latency_cold_cache() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;
    let mut stats = PerfStats::new();

    // Clear cache before each search
    for i in 0..20 {
        fixture.state.cache.clear();

        let (response, duration) = fixture
            .measure_request(Request::Search {
                id: i,
                query: format!("query-{}", i),
                limit: Some(10),
            })
            .await;

        if matches!(response, Response::Success { .. }) {
            stats.record(duration);
        }
    }

    stats.print_summary("Search (Cold Cache)");

    // Cold cache search should still be reasonably fast
    assert!(
        stats.mean() < Duration::from_millis(50),
        "Mean cold search should be < 50ms"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn bench_search_latency_warm_cache() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;
    let mut stats = PerfStats::new();

    let query = "test-package";

    // Warmup - populate cache
    fixture
        .measure_request(Request::Search {
            id: 0,
            query: query.to_string(),
            limit: Some(10),
        })
        .await;

    // Measure cached searches
    for i in 1..100 {
        let (response, duration) = fixture
            .measure_request(Request::Search {
                id: i,
                query: query.to_string(),
                limit: Some(10),
            })
            .await;

        assert!(
            matches!(response, Response::Success { .. }),
            "Cached search should succeed"
        );
        stats.record(duration);
    }

    stats.print_summary("Search (Warm Cache)");

    // Warm cache should be very fast
    assert!(
        stats.mean() < Duration::from_millis(1),
        "Mean cached search should be < 1ms"
    );
    assert!(
        stats.p99() < Duration::from_millis(5),
        "P99 cached search should be < 5ms"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn bench_info_latency() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;
    let mut stats = PerfStats::new();

    let package = "bash";

    // Warmup
    for i in 0..5 {
        fixture
            .measure_request(Request::Info {
                id: i,
                package: package.to_string(),
            })
            .await;
    }

    // Measure
    for i in 0..50 {
        let (response, duration) = fixture
            .measure_request(Request::Info {
                id: i,
                package: package.to_string(),
            })
            .await;

        if matches!(response, Response::Success { .. }) {
            stats.record(duration);
        }
    }

    if !stats.measurements.is_empty() {
        stats.print_summary("Info (Cached)");

        assert!(
            stats.mean() < Duration::from_millis(2),
            "Mean cached info should be < 2ms"
        );
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn bench_status_latency() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;
    let mut stats = PerfStats::new();

    // Warmup
    for i in 0..5 {
        fixture.measure_request(Request::Status { id: i }).await;
    }

    // Measure
    for i in 0..50 {
        let (response, duration) = fixture.measure_request(Request::Status { id: i }).await;

        assert!(
            matches!(response, Response::Success { .. }),
            "Status should succeed"
        );
        stats.record(duration);
    }

    stats.print_summary("Status");

    // Status should be fast (cached)
    assert!(
        stats.mean() < Duration::from_millis(10),
        "Mean status should be < 10ms"
    );

    Ok(())
}

// ============================================================================
// Test 2: Throughput Benchmarks
// ============================================================================

#[tokio::test]
#[serial]
async fn bench_sequential_throughput() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;

    let start = Instant::now();
    let request_count = 1000;

    for i in 0..request_count {
        let _ = handle_request(Arc::clone(&fixture.state), Request::Ping { id: i }).await;
    }

    let duration = start.elapsed();
    let throughput = request_count as f64 / duration.as_secs_f64();

    println!(
        "\nSequential Throughput: {:.2} req/s ({} requests in {:?})",
        throughput, request_count, duration
    );

    assert!(
        throughput > 500.0,
        "Sequential throughput should be > 500 req/s"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn bench_concurrent_throughput() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;

    let start = Instant::now();
    let request_count = 1000;

    // Submit all requests concurrently
    let mut handles = vec![];
    for i in 0..request_count {
        let state = Arc::clone(&fixture.state);
        let handle =
            tokio::spawn(async move { handle_request(state, Request::Ping { id: i }).await });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await?;
    }

    let duration = start.elapsed();
    let throughput = request_count as f64 / duration.as_secs_f64();

    println!(
        "\nConcurrent Throughput: {:.2} req/s ({} requests in {:?})",
        throughput, request_count, duration
    );

    assert!(
        throughput > 1000.0,
        "Concurrent throughput should be > 1000 req/s"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn bench_batch_throughput() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;

    let batch_size = 50;
    let batch_count = 20;

    let start = Instant::now();

    for batch_id in 0..batch_count {
        let subrequests: Vec<Request> = (0..batch_size)
            .map(|i| Request::Ping {
                id: (batch_id * batch_size + i) as u64,
            })
            .collect();

        let batch_request = Request::Batch {
            id: batch_id as u64,
            requests: subrequests,
        };

        let _ = handle_request(Arc::clone(&fixture.state), batch_request).await;
    }

    let duration = start.elapsed();
    let total_requests = batch_size * batch_count;
    let throughput = total_requests as f64 / duration.as_secs_f64();

    println!(
        "\nBatch Throughput: {:.2} req/s ({} requests in {} batches, {:?})",
        throughput, total_requests, batch_count, duration
    );

    assert!(
        throughput > 2000.0,
        "Batch throughput should be > 2000 req/s"
    );

    Ok(())
}

// ============================================================================
// Test 3: Memory Usage Benchmarks
// ============================================================================

#[tokio::test]
#[serial]
async fn bench_cache_memory_efficiency() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;

    // Populate cache with many different queries
    for i in 0..100 {
        let _ = handle_request(
            Arc::clone(&fixture.state),
            Request::Search {
                id: i,
                query: format!("query-{}", i),
                limit: Some(10),
            },
        )
        .await;
    }

    // Check cache stats
    let stats_response =
        handle_request(Arc::clone(&fixture.state), Request::CacheStats { id: 999 }).await;

    if let Response::Success {
        result: ResponseResult::CacheStats { size, max_size },
        ..
    } = stats_response
    {
        println!(
            "\nCache Memory: {} entries / {} max ({:.1}% full)",
            size,
            max_size,
            (size as f64 / max_size as f64) * 100.0
        );

        assert!(size <= max_size, "Cache should respect max size");
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn bench_state_memory_footprint() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;

    // Perform various operations
    for i in 0..50 {
        let _ = handle_request(
            Arc::clone(&fixture.state),
            Request::Search {
                id: i,
                query: "test".to_string(),
                limit: Some(10),
            },
        )
        .await;
    }

    // State should still be valid and responsive
    let response = handle_request(Arc::clone(&fixture.state), Request::Ping { id: 999 }).await;

    assert!(
        matches!(response, Response::Success { .. }),
        "State should remain valid under load"
    );

    Ok(())
}

// ============================================================================
// Test 4: Cache Effectiveness
// ============================================================================

#[tokio::test]
#[serial]
async fn bench_cache_hit_rate() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;
    fixture.state.cache.clear();

    // Simulate realistic workload: 20% unique queries, 80% repeated
    let unique_queries = vec!["query1", "query2", "query3", "query4", "query5"];

    for i in 0..100 {
        let query = if i < 20 {
            // First 20: unique queries
            format!("unique-{}", i)
        } else {
            // Remaining 80: repeated from pool
            unique_queries[i % unique_queries.len()].to_string()
        };

        let _ = handle_request(
            Arc::clone(&fixture.state),
            Request::Search {
                id: i as u64,
                query,
                limit: Some(10),
            },
        )
        .await;
    }

    // Check metrics
    let metrics_response =
        handle_request(Arc::clone(&fixture.state), Request::Metrics { id: 999 }).await;

    if let Response::Success {
        result: ResponseResult::Metrics(m),
        ..
    } = metrics_response
    {
        let total = m.cache_hits + m.cache_misses;
        let hit_rate = if total > 0 {
            (m.cache_hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        println!(
            "\nCache Effectiveness: {:.1}% hit rate ({} hits, {} misses)",
            hit_rate, m.cache_hits, m.cache_misses
        );

        // With 80% repeated queries, hit rate should be high
        assert!(
            hit_rate > 50.0,
            "Cache hit rate should be > 50% with repeated queries"
        );
    }

    Ok(())
}

// ============================================================================
// Test 5: Index Performance
// ============================================================================

#[tokio::test]
#[serial]
async fn bench_index_search_performance() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;
    fixture.state.cache.clear();

    let mut stats = PerfStats::new();

    // Test various query patterns
    let queries = vec![
        "a",                       // Very short (many matches)
        "lib",                     // Common prefix
        "python",                  // Specific package
        "fire",                    // Partial match
        "nonexistent-package-xyz", // No matches
    ];

    for (i, query) in queries.iter().enumerate() {
        let (response, duration) = fixture
            .measure_request(Request::Search {
                id: i as u64,
                query: (*query).to_string(),
                limit: Some(50),
            })
            .await;

        if let Response::Success {
            result: ResponseResult::Search(result),
            ..
        } = response
        {
            println!(
                "Query '{}': {} results in {:?}",
                query,
                result.packages.len(),
                duration
            );
            stats.record(duration);
        }
    }

    stats.print_summary("Index Search");

    // Index search should be fast even for cold cache
    assert!(
        stats.mean() < Duration::from_millis(20),
        "Mean index search should be < 20ms"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn bench_index_lookup_performance() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;
    fixture.state.cache.clear();

    let packages = vec!["bash", "coreutils", "grep", "sed", "awk"];
    let mut stats = PerfStats::new();

    for (i, pkg) in packages.iter().enumerate() {
        let (response, duration) = fixture
            .measure_request(Request::Info {
                id: i as u64,
                package: (*pkg).to_string(),
            })
            .await;

        if matches!(response, Response::Success { .. }) {
            println!("Lookup '{}': {:?}", pkg, duration);
            stats.record(duration);
        }
    }

    if !stats.measurements.is_empty() {
        stats.print_summary("Index Lookup");

        assert!(
            stats.mean() < Duration::from_millis(10),
            "Mean index lookup should be < 10ms"
        );
    }

    Ok(())
}

// ============================================================================
// Test 6: Stress Testing
// ============================================================================

#[tokio::test]
#[serial]
async fn stress_high_volume_requests() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;

    let start = Instant::now();
    let mut handles = vec![];

    // 2000 concurrent requests
    for i in 0..2000 {
        let state = Arc::clone(&fixture.state);
        let handle =
            tokio::spawn(async move { handle_request(state, Request::Ping { id: i }).await });
        handles.push(handle);
    }

    // Wait for all
    let mut success = 0;
    let mut failure = 0;

    for handle in handles {
        match handle.await? {
            Response::Success { .. } => success += 1,
            Response::Error { .. } => failure += 1,
        }
    }

    let duration = start.elapsed();

    println!(
        "\nStress Test: {} success, {} failure in {:?}",
        success, failure, duration
    );

    // Rate limiter allows 100 req/s with burst of 200, so with 2000 requests
    // we expect many to be rate limited. Verify that at least the burst amount succeeds.
    assert!(
        success >= 190,
        "Should handle at least burst capacity (200) under stress, got {}",
        success
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn stress_rapid_cache_churn() -> Result<()> {
    let fixture = PerformanceTestFixture::new().await?;

    // Rapidly populate and clear cache
    for cycle in 0..10 {
        // Populate
        for i in 0..20 {
            let _ = handle_request(
                Arc::clone(&fixture.state),
                Request::Search {
                    id: (cycle * 20 + i) as u64,
                    query: format!("query-{}", i),
                    limit: Some(10),
                },
            )
            .await;
        }

        // Clear
        let _ = handle_request(
            Arc::clone(&fixture.state),
            Request::CacheClear { id: cycle as u64 },
        )
        .await;
    }

    // System should still be responsive
    let response = handle_request(Arc::clone(&fixture.state), Request::Ping { id: 999 }).await;

    assert!(
        matches!(response, Response::Success { .. }),
        "System should remain responsive after cache churn"
    );

    Ok(())
}
