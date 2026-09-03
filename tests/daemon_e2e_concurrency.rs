#![cfg(feature = "arch")]

//! Daemon concurrent clients, request queuing, races, and thread safety.

use anyhow::Result;
use omg_lib::daemon::handlers::handle_request;
use omg_lib::daemon::protocol::{Request, Response, ResponseResult};
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;

/// Helper to extract response ID
const fn response_id(response: &Response) -> u64 {
    match response {
        Response::Success { id, .. } | Response::Error { id, .. } => *id,
    }
}

pub mod common;

use common::DaemonTestFixture as ConcurrencyTestFixture;

// ============================================================================
// Concurrent Read Operations
// ============================================================================

#[tokio::test]
#[serial]
async fn test_concurrent_search_requests() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new()?;

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

    // Wait for all requests to complete. Valid short queries are always
    // answerable (handle_search returns Success for them, and the per-state
    // quota is 100/s with burst 200, so every
    // one of the 50 requests must succeed.
    let mut success_count = 0;
    for handle in handles {
        let (id, response) = handle.await?;
        match response {
            Response::Success { .. } => success_count += 1,
            Response::Error { code, message, .. } => {
                panic!("concurrent search {id} failed: code={code} message={message}")
            }
        }
        // Verify response ID matches request ID
        assert_eq!(response_id(&response), id);
    }

    assert_eq!(success_count, 50, "every concurrent search should succeed");

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_concurrent_status_requests() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new()?;

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

    // All should return the same result (cached).
    let mut results = Vec::with_capacity(handles.len());
    for handle in handles {
        let response = handle.await?;
        match response {
            Response::Success {
                result: ResponseResult::Status(status),
                ..
            } => results.push(status),
            response => panic!("Concurrent status request failed: {response:?}"),
        }
    }
    assert_eq!(
        results.len(),
        20,
        "Every status request should return a result"
    );

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
// Concurrent Read + Write Operations
// ============================================================================

#[tokio::test]
#[serial]
async fn test_concurrent_read_and_cache_clear() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new()?;

    // Start continuous read requests
    let mut read_handles = vec![];
    for i in 0..20 {
        let state = Arc::clone(&fixture.state);
        let handle = tokio::spawn(async move {
            let mut completed = 0;
            for j in 0..5 {
                let request = Request::Search {
                    id: (i * 5 + j) as u64,
                    query: "test".to_string(),
                    limit: Some(10),
                };
                let _response = handle_request(Arc::clone(&state), request).await;
                completed += 1;
            }
            completed
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

    // All operations should complete without deadlock.
    let mut completed_reads = 0;
    for handle in read_handles {
        completed_reads += handle.await?;
    }
    assert_eq!(
        completed_reads, 100,
        "Every concurrent read should complete"
    );

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
    let fixture = ConcurrencyTestFixture::new()?;

    // Multiple threads updating cache with different queries
    let queries = ["query1", "query2", "query3", "query4", "query5"];

    let mut handles = vec![];
    for (i, query) in queries.iter().enumerate() {
        let state = Arc::clone(&fixture.state);
        let query = query.to_string();
        let handle = tokio::spawn(async move {
            let mut completed = 0;
            for _ in 0..10 {
                let request = Request::Search {
                    id: i as u64,
                    query: query.clone(),
                    limit: Some(10),
                };
                let _response = handle_request(Arc::clone(&state), request).await;
                completed += 1;
            }
            completed
        });
        handles.push(handle);
    }

    // All should complete without race conditions.
    let mut completed = 0;
    for handle in handles {
        completed += handle.await?;
    }
    assert_eq!(
        completed, 50,
        "Every concurrent cache update should complete"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_no_deadlock_with_recursive_locks() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new()?;

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
// Race Condition Testing
// ============================================================================

#[tokio::test]
#[serial]
async fn test_no_race_in_cache_updates() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new()?;

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

    // All 50 hits on the same key must succeed AND agree: the cached result
    // set for a given query is immutable once inserted.
    let mut results = vec![];
    let mut total_responses = 0;
    for handle in handles {
        total_responses += 1;
        match handle.await? {
            Response::Success {
                result: ResponseResult::Search(search_result),
                ..
            } => results.push(search_result),
            Response::Error { code, message, .. } => {
                panic!("concurrent same-key search failed: code={code} message={message}")
            }
            other @ Response::Success { .. } => panic!("unexpected response to Search: {other:?}"),
        }
    }

    assert_eq!(
        total_responses, 50,
        "every same-key search must receive a response"
    );
    assert_eq!(results.len(), 50, "every same-key search must succeed");

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
    let fixture = ConcurrencyTestFixture::new()?;

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

    let metrics = match metrics_response {
        Response::Success {
            result: ResponseResult::Metrics(metrics),
            ..
        } => metrics,
        response => panic!("Metrics request failed: {response:?}"),
    };
    // Should have processed all 101 requests (100 pings + 1 metrics)
    assert!(
        metrics.requests_total >= 101,
        "Metrics should accurately count concurrent requests: got {}",
        metrics.requests_total
    );

    Ok(())
}

// ============================================================================
// Thread Safety Verification
// ============================================================================

#[tokio::test]
#[serial]
async fn test_shared_state_thread_safety() -> Result<()> {
    let fixture = ConcurrencyTestFixture::new()?;

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

    // All five request types are infallible for these inputs and the fresh
    // state's rate-limit budget (burst 200) covers all 50 requests, so every
    // response must be a Success carrying its request's id. The previous
    // `matches!(Success | Error)` accepted literally every possible response
    // and proved nothing.
    let mut successes = 0;
    for handle in handles {
        let response = handle.await?;
        match &response {
            Response::Success { .. } => successes += 1,
            Response::Error { id, code, message } => {
                panic!("request {id} failed under mixed concurrency: code={code} message={message}")
            }
        }
    }
    assert_eq!(successes, 50, "all mixed-workload requests must succeed");

    Ok(())
}
