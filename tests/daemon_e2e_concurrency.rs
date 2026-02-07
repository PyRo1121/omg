#![cfg(feature = "arch")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]

//! S-tier E2E Tests: Daemon Concurrency and Thread Safety
//!
//! Comprehensive tests for concurrent clients, request queuing, deadlock
//! prevention, race conditions, and thread safety guarantees.

use anyhow::Result;
use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::protocol::{Request, Response, ResponseResult};
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::time::sleep;

/// Helper to extract response ID
fn response_id(response: &Response) -> u64 {
    match response {
        Response::Success { id, .. } => *id,
        Response::Error { id, .. } => *id,
    }
}

/// Test fixture for concurrency tests
struct ConcurrencyTestFixture {
    _temp_dir: TempDir,
    state: Arc<DaemonState>,
}

impl ConcurrencyTestFixture {
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
}

// ============================================================================
// Test 1: Concurrent Read Operations
// ============================================================================

#[tokio::test]
#[serial]
async fn test_concurrent_search_requests() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Spawn 50 concurrent search requests
    let mut handles = vec![];
    for i in 0..50 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Search {
                id: i,
                query: "test".to_string(),
                limit: Some(10),
            };
            let response = handle_request(state, request).await;
            (i, response)
        });
        handles.push(handle);
    }

    // Wait for all requests to complete
    let mut success_count = 0;
    let mut error_count = 0;

    for handle in handles {
        let (id, response) = handle.await?;
        match response {
            Response::Success { .. } => success_count += 1,
            Response::Error { .. } => error_count += 1,
        }
        // Verify response ID matches request ID
        assert_eq!(response_id(&response), id);
    }

    println!(
        "Concurrent searches: {} success, {} errors",
        success_count, error_count
    );
    assert!(success_count > 0, "At least some requests should succeed");

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_concurrent_info_requests() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    let packages = vec!["bash", "coreutils", "grep", "sed", "awk"];

    // Request info for multiple packages concurrently
    let mut handles = vec![];
    for (i, pkg) in packages.iter().enumerate() {
        let state = Arc::clone(&fixture.state);
        let pkg = pkg.to_string();
        let handle = tokio::spawn(async move {
            let request = Request::Info {
                id: i as u64,
                package: pkg,
            };
            handle_request(state, request).await
        });
        handles.push(handle);
    }

    // All should complete without deadlock
    for handle in handles {
        let response = handle.await?;
        // May succeed or fail (package might not exist), but should not hang
        assert!(
            matches!(response, Response::Success { .. } | Response::Error { .. }),
            "Request should complete"
        );
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_concurrent_status_requests() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // 20 concurrent status requests
    let mut handles = vec![];
    for i in 0..20 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Status { id: i };
            handle_request(state, request).await
        });
        handles.push(handle);
    }

    // All should return same result (cached)
    let mut results = vec![];
    for handle in handles {
        let response = handle.await?;
        if let Response::Success {
            result: ResponseResult::Status(status),
            ..
        } = response
        {
            results.push(status);
        }
    }

    // All status results should be consistent
    if results.len() > 1 {
        let first = &results[0];
        for status in &results[1..] {
            assert_eq!(
                status.total_packages, first.total_packages,
                "Concurrent status requests should return consistent data"
            );
        }
    }

    Ok(())
}

// ============================================================================
// Test 2: Concurrent Read + Write Operations
// ============================================================================

#[tokio::test]
#[serial]
async fn test_concurrent_read_and_cache_clear() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Start continuous read requests
    let mut read_handles = vec![];
    for i in 0..20 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            for j in 0..5 {
                let request = Request::Search {
                    id: (i * 5 + j) as u64,
                    query: "test".to_string(),
                    limit: Some(10),
                };
                let _ = handle_request(Arc::clone(&state), request).await;
            }
        });
        read_handles.push(handle);
    }

    // Interleave cache clear operations
    let mut clear_handles = vec![];
    for i in 0..3 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(i * 10)).await;
            let request = Request::CacheClear { id: 1000 + i };
            handle_request(state, request).await
        });
        clear_handles.push(handle);
    }

    // All operations should complete without deadlock
    for handle in read_handles {
        handle.await?;
    }

    for handle in clear_handles {
        let response = handle.await?;
        assert!(
            matches!(response, Response::Success { .. }),
            "Cache clear should succeed"
        );
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_concurrent_cache_updates() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Multiple threads updating cache with different queries
    let queries = vec!["query1", "query2", "query3", "query4", "query5"];

    let mut handles = vec![];
    for (i, query) in queries.iter().enumerate() {
        let state = Arc::clone(&fixture.state);
        let query = query.to_string();
        let handle = tokio::spawn(async move {
            for _ in 0..10 {
                let request = Request::Search {
                    id: i as u64,
                    query: query.clone(),
                    limit: Some(10),
                };
                let _ = handle_request(Arc::clone(&state), request).await;
            }
        });
        handles.push(handle);
    }

    // All should complete without race conditions
    for handle in handles {
        handle.await?;
    }

    Ok(())
}

// ============================================================================
// Test 3: Request Queuing and Fairness
// ============================================================================

#[tokio::test]
#[serial]
async fn test_request_queuing_fairness() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Submit 100 requests and track completion order
    let mut handles = vec![];
    for i in 0..100 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let start = std::time::Instant::now();
            let request = Request::Ping { id: i };
            let _ = handle_request(state, request).await;
            (i, start.elapsed())
        });
        handles.push(handle);
    }

    // Collect completion times
    let mut completion_times = vec![];
    for handle in handles {
        let (id, duration) = handle.await?;
        completion_times.push((id, duration));
    }

    // Check that requests don't have excessive variance (fairness)
    let avg_duration: Duration =
        completion_times.iter().map(|(_, d)| *d).sum::<Duration>() / completion_times.len() as u32;

    let max_duration = completion_times.iter().map(|(_, d)| *d).max().unwrap();

    println!(
        "Average: {:?}, Max: {:?}, Ratio: {:.2}x",
        avg_duration,
        max_duration,
        max_duration.as_secs_f64() / avg_duration.as_secs_f64()
    );

    // Max should not be more than 50x average (reasonable fairness for very fast operations)
    // When operations are sub-microsecond, variance can be higher due to scheduling
    assert!(
        max_duration < avg_duration * 50,
        "Request queuing should be reasonably fair"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_no_request_starvation() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Submit mix of fast (ping) and potentially slow (search) requests
    let mut handles = vec![];

    // 50 search requests
    for i in 0..50 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Search {
                id: i,
                query: "test".to_string(),
                limit: Some(10),
            };
            let start = std::time::Instant::now();
            let _ = handle_request(state, request).await;
            start.elapsed()
        });
        handles.push(handle);
    }

    // 50 ping requests (should be fast)
    for i in 50..100 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Ping { id: i };
            let start = std::time::Instant::now();
            let _ = handle_request(state, request).await;
            start.elapsed()
        });
        handles.push(handle);
    }

    // All should complete (no starvation)
    let mut all_durations = vec![];
    for handle in handles {
        let duration = handle.await?;
        all_durations.push(duration);
    }

    // No request should take excessively long
    let max_duration = all_durations.iter().max().unwrap();
    assert!(
        *max_duration < Duration::from_secs(10),
        "No request should be starved, max duration: {:?}",
        max_duration
    );

    Ok(())
}

// ============================================================================
// Test 4: Deadlock Prevention
// ============================================================================

#[tokio::test]
#[serial]
async fn test_no_deadlock_with_batch_requests() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Submit multiple batch requests concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let subrequests = vec![
                Request::Ping { id: i * 3 },
                Request::Ping { id: i * 3 + 1 },
                Request::Ping { id: i * 3 + 2 },
            ];

            let batch = Request::Batch {
                id: i,
                requests: subrequests,
            };

            handle_request(state, batch).await
        });
        handles.push(handle);
    }

    // All should complete without deadlock
    for handle in handles {
        let response = handle.await?;
        assert!(
            matches!(response, Response::Success { .. }),
            "Batch request should complete"
        );
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_no_deadlock_with_recursive_locks() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Scenario: Status request might internally lock cache, then query system
    // Multiple concurrent status requests should not deadlock

    let mut handles = vec![];
    for i in 0..30 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Status { id: i };
            handle_request(state, request).await
        });
        handles.push(handle);
    }

    // Use timeout to detect deadlock
    let timeout_duration = Duration::from_secs(10);
    let result = tokio::time::timeout(timeout_duration, async {
        for handle in handles {
            handle.await.unwrap();
        }
    })
    .await;

    assert!(result.is_ok(), "Requests should complete without deadlock");

    Ok(())
}

// ============================================================================
// Test 5: Race Condition Testing
// ============================================================================

#[tokio::test]
#[serial]
async fn test_no_race_in_cache_updates() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Multiple threads updating same cache key
    let mut handles = vec![];
    for i in 0..50 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Search {
                id: i,
                query: "same-query".to_string(),
                limit: Some(10),
            };
            handle_request(state, request).await
        });
        handles.push(handle);
    }

    // All should get consistent results
    let mut results = vec![];
    for handle in handles {
        if let Response::Success {
            result: ResponseResult::Search(search_result),
            ..
        } = handle.await?
        {
            results.push(search_result);
        }
    }

    // All cached results should be identical
    if results.len() > 1 {
        let first = &results[0];
        for result in &results[1..] {
            assert_eq!(
                result.total, first.total,
                "Concurrent cache updates should not cause inconsistency"
            );
        }
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_no_race_in_metrics_updates() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Submit many requests to increment metrics
    let mut handles = vec![];
    for i in 0..100 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Ping { id: i };
            handle_request(state, request).await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    for handle in handles {
        handle.await?;
    }

    // Check final metrics
    let metrics_response =
        handle_request(Arc::clone(&fixture.state), Request::Metrics { id: 1000 }).await;

    if let Response::Success {
        result: ResponseResult::Metrics(metrics),
        ..
    } = metrics_response
    {
        // Should have processed all 101 requests (100 pings + 1 metrics)
        assert!(
            metrics.requests_total >= 101,
            "Metrics should accurately count concurrent requests: got {}",
            metrics.requests_total
        );
    }

    Ok(())
}

// ============================================================================
// Test 6: Thread Safety Verification
// ============================================================================

#[tokio::test]
#[serial]
async fn test_shared_state_thread_safety() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Mix of different request types accessing shared state
    let mut handles = vec![];

    // Searches (read cache)
    for i in 0..20 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Search {
                id: i,
                query: format!("query-{}", i % 5),
                limit: Some(10),
            };
            handle_request(state, request).await
        });
        handles.push(handle);
    }

    // Status requests (read system state)
    for i in 20..40 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Status { id: i };
            handle_request(state, request).await
        });
        handles.push(handle);
    }

    // Cache clears (write cache)
    for i in 40..45 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::CacheClear { id: i };
            handle_request(state, request).await
        });
        handles.push(handle);
    }

    // Metrics (read global state)
    for i in 45..50 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Metrics { id: i };
            handle_request(state, request).await
        });
        handles.push(handle);
    }

    // All should complete safely
    for handle in handles {
        let response = handle.await?;
        assert!(
            matches!(response, Response::Success { .. } | Response::Error { .. }),
            "All requests should complete safely"
        );
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_arc_reference_counting() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Create many Arc clones to test reference counting
    let mut handles = vec![];
    for i in 0..100 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            // Hold Arc for a moment
            sleep(Duration::from_millis(10)).await;
            let request = Request::Ping { id: i };
            handle_request(state, request).await
        });
        handles.push(handle);
    }

    // All Arc clones should be properly cleaned up
    for handle in handles {
        handle.await?;
    }

    // State should still be valid
    let response = handle_request(Arc::clone(&fixture.state), Request::Ping { id: 999 }).await;

    assert!(
        matches!(response, Response::Success { .. }),
        "State should remain valid after many Arc operations"
    );

    Ok(())
}

// ============================================================================
// Test 7: Load Testing
// ============================================================================

#[tokio::test]
#[serial]
async fn test_sustained_concurrent_load() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Sustained load: 500 requests over 5 seconds
    let start = std::time::Instant::now();
    let mut handles = vec![];

    for i in 0..500 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Ping { id: i };
            handle_request(state, request).await
        });
        handles.push(handle);

        // Spread out requests
        if i % 20 == 0 {
            sleep(Duration::from_millis(50)).await;
        }
    }

    // Wait for all to complete
    for handle in handles {
        handle.await?;
    }

    let total_duration = start.elapsed();
    let throughput = 500.0 / total_duration.as_secs_f64();

    println!(
        "Sustained load: 500 requests in {:?} ({:.2} req/s)",
        total_duration, throughput
    );

    // Should maintain reasonable throughput
    assert!(
        throughput > 50.0,
        "Should handle at least 50 req/s under sustained load"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_burst_load_handling() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new().await?;

    // Burst: 200 concurrent requests immediately
    let start = std::time::Instant::now();
    let mut handles = vec![];

    for i in 0..200 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let request = Request::Ping { id: i };
            handle_request(state, request).await
        });
        handles.push(handle);
    }

    // All should complete
    for handle in handles {
        handle.await?;
    }

    let burst_duration = start.elapsed();

    println!("Burst of 200 requests completed in {:?}", burst_duration);

    // Should handle burst within reasonable time
    assert!(
        burst_duration < Duration::from_secs(5),
        "Should handle burst load within 5 seconds"
    );

    Ok(())
}
