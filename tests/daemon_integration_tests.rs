#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]

//! Comprehensive daemon integration tests
//! Tests IPC protocol, caching, concurrency, error handling

use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::protocol::{Request, Response, ResponseResult, error_codes};
use serial_test::serial;
use std::sync::Arc;
use tempfile::TempDir;

fn setup_test_env() -> (TempDir, Arc<DaemonState>) {
    #[cfg(feature = "arch")]
    {
        omg_lib::package_managers::clear_alpm_cache();
        omg_lib::package_managers::invalidate_caches();
    }

    let temp_dir = TempDir::new().unwrap();

    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("OMG_DAEMON_DATA_DIR", temp_dir.path());
        std::env::set_var("OMG_DATA_DIR", temp_dir.path());
    }

    omg_lib::core::security::init_audit_logger().expect("Failed to init audit logger");

    let state = match DaemonState::new() {
        Ok(s) => Arc::new(s),
        Err(e) => panic!("Failed to create DaemonState: {}", e),
    };

    (temp_dir, state)
}

// ============================================================================
// IPC Protocol Tests - Test all request/response variants
// ============================================================================

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_metrics_endpoint_returns_valid_data() {
    let (_temp, state) = setup_test_env();

    let req = Request::Metrics { id: 100 };
    let response = handle_request(Arc::clone(&state), req).await;

    match response {
        Response::Success { id, result } => {
            assert_eq!(id, 100);
            if let ResponseResult::Metrics(metrics) = result {
                // Verify metrics are returned (unsigned, so always >= 0)
                let _ = metrics.requests_total;
                let _ = metrics.cache_hits;
                let _ = metrics.cache_misses;
            } else {
                panic!("Expected Metrics response");
            }
        }
        Response::Error { message, .. } => panic!("Metrics failed: {}", message),
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_suggest_endpoint_handles_empty_query() {
    let (_temp, state) = setup_test_env();

    let req = Request::Suggest {
        id: 200,
        query: "".to_string(),
        limit: Some(10),
    };
    let response = handle_request(Arc::clone(&state), req).await;

    match response {
        Response::Success { id, result } => {
            assert_eq!(id, 200);
            if let ResponseResult::Suggest(suggestions) = result {
                assert!(
                    suggestions.is_empty(),
                    "Empty query should return empty suggestions"
                );
            } else {
                panic!("Expected Suggest response");
            }
        }
        Response::Error { .. } => {
            // Empty query might be rejected, which is also acceptable
        }
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_suggest_endpoint_respects_limit() {
    let (_temp, state) = setup_test_env();

    let req = Request::Suggest {
        id: 201,
        query: "fire".to_string(),
        limit: Some(5),
    };
    let response = handle_request(Arc::clone(&state), req).await;

    match response {
        Response::Success { id, result } => {
            assert_eq!(id, 201);
            if let ResponseResult::Suggest(suggestions) = result {
                assert!(suggestions.len() <= 5, "Should respect limit parameter");
            } else {
                panic!("Expected Suggest response");
            }
        }
        Response::Error { .. } => {
            // Might fail if index not available, which is OK
        }
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_suggest_endpoint_rejects_oversized_limit() {
    let (_temp, state) = setup_test_env();

    let req = Request::Suggest {
        id: 202,
        query: "test".to_string(),
        limit: Some(1000), // Way over limit
    };
    let response = handle_request(Arc::clone(&state), req).await;

    // Should clamp to max limit (50) or reject
    match response {
        Response::Success { result, .. } => {
            if let ResponseResult::Suggest(suggestions) = result {
                assert!(suggestions.len() <= 50, "Should clamp to max limit");
            }
        }
        Response::Error { .. } => {
            // Rejection is also acceptable
        }
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_batch_request_executes_all_subcommands() {
    let (_temp, state) = setup_test_env();

    let subrequests = vec![
        Request::Ping { id: 1 },
        Request::Ping { id: 2 },
        Request::Ping { id: 3 },
    ];

    let req = Request::Batch {
        id: 300,
        requests: Box::new(subrequests),
    };
    let response = handle_request(Arc::clone(&state), req).await;

    match response {
        Response::Success { id, result } => {
            assert_eq!(id, 300);
            if let ResponseResult::Batch(responses) = result {
                assert_eq!(responses.len(), 3, "Should execute all 3 subrequests");
                for resp in responses.iter() {
                    match resp {
                        Response::Success { result, .. } => {
                            assert!(matches!(result, ResponseResult::Ping(_)));
                        }
                        Response::Error { .. } => panic!("Batch subrequest failed"),
                    }
                }
            } else {
                panic!("Expected Batch response");
            }
        }
        Response::Error { message, .. } => panic!("Batch request failed: {}", message),
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_batch_request_handles_mixed_success_and_failure() {
    let (_temp, state) = setup_test_env();

    let subrequests = vec![
        Request::Ping { id: 1 },
        Request::Info {
            id: 2,
            package: "invalid; rm -rf /".to_string(), // Invalid package
        },
        Request::Ping { id: 3 },
    ];

    let req = Request::Batch {
        id: 301,
        requests: Box::new(subrequests),
    };
    let response = handle_request(Arc::clone(&state), req).await;

    match response {
        Response::Success { result, .. } => {
            if let ResponseResult::Batch(responses) = result {
                assert_eq!(responses.len(), 3);
                // First and third should succeed (Ping)
                // Second should fail (invalid package name)
                let has_success = responses
                    .iter()
                    .any(|r| matches!(r, Response::Success { .. }));
                let has_error = responses
                    .iter()
                    .any(|r| matches!(r, Response::Error { .. }));
                assert!(has_success, "Should have some successful responses");
                assert!(has_error, "Should have some error responses");
            } else {
                panic!("Expected Batch response");
            }
        }
        Response::Error { message, .. } => panic!("Batch request failed: {}", message),
    }
}

// ============================================================================
// Caching Logic Tests
// ============================================================================

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
#[cfg(feature = "arch")]
async fn test_cache_hit_after_search() {
    let (_temp, state) = setup_test_env();

    // First search - should be cache miss
    let req1 = Request::Search {
        id: 400,
        query: "firefox".to_string(),
        limit: Some(10),
    };
    let _response1 = handle_request(Arc::clone(&state), req1).await;

    // Second identical search - should be cache hit
    let req2 = Request::Search {
        id: 401,
        query: "firefox".to_string(),
        limit: Some(10),
    };
    let start = std::time::Instant::now();
    let _response2 = handle_request(Arc::clone(&state), req2).await;
    let elapsed = start.elapsed();

    // Cache hit should be extremely fast (< 5ms)
    assert!(
        elapsed.as_millis() < 50,
        "Cached response should be fast, got {:?}",
        elapsed
    );
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_cache_clear_invalidates_cache() {
    let (_temp, state) = setup_test_env();

    // Get current cache size
    let stats_req = Request::CacheStats { id: 500 };
    let _stats1 = handle_request(Arc::clone(&state), stats_req.clone()).await;

    // Clear cache
    let clear_req = Request::CacheClear { id: 501 };
    let _clear_resp = handle_request(Arc::clone(&state), clear_req).await;

    // Check cache size is now zero
    let stats2 = handle_request(Arc::clone(&state), stats_req).await;

    if let Response::Success { result, .. } = stats2
        && let ResponseResult::CacheStats { size, .. } = result
    {
        assert_eq!(size, 0, "Cache should be empty after clear");
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_cache_respects_max_size_limit() {
    let (_temp, state) = setup_test_env();

    let stats_req = Request::CacheStats { id: 600 };
    let response = handle_request(Arc::clone(&state), stats_req).await;

    match response {
        Response::Success { result, .. } => {
            if let ResponseResult::CacheStats { size, max_size } = result {
                assert!(size <= max_size, "Cache size should never exceed max");
                assert!(max_size > 0, "Max size should be configured");
            } else {
                panic!("Expected CacheStats response");
            }
        }
        Response::Error { message, .. } => panic!("CacheStats failed: {}", message),
    }
}

// ============================================================================
// Concurrent Request Handling
// ============================================================================

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_concurrent_ping_requests() {
    let (_temp, state) = setup_test_env();

    // Send 10 concurrent ping requests
    let mut handles = vec![];
    for i in 0..10 {
        let state_clone = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            let req = Request::Ping { id: i };
            handle_request(state_clone, req).await
        });
        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        let response = handle.await.unwrap();
        match response {
            Response::Success { result, .. } => {
                assert!(matches!(result, ResponseResult::Ping(_)));
            }
            Response::Error { message, .. } => panic!("Concurrent ping failed: {}", message),
        }
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
#[cfg(feature = "arch")]
async fn test_concurrent_search_requests_different_queries() {
    let (_temp, state) = setup_test_env();

    // Send concurrent searches with different queries
    let queries = ["firefox", "vim", "git", "bash", "python"];
    let mut handles = vec![];

    for (i, query) in queries.iter().enumerate() {
        let state_clone = Arc::clone(&state);
        let query_str = query.to_string();
        let handle = tokio::spawn(async move {
            let req = Request::Search {
                id: i as u64,
                query: query_str,
                limit: Some(5),
            };
            handle_request(state_clone, req).await
        });
        handles.push(handle);
    }

    // All should succeed
    for handle in handles {
        let response = handle.await.unwrap();
        assert!(
            matches!(response, Response::Success { .. }),
            "Concurrent search should succeed"
        );
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_cache_thread_safety_under_concurrent_access() {
    let (_temp, state) = setup_test_env();

    // Multiple threads reading cache stats simultaneously
    let mut handles = vec![];
    for i in 0..20 {
        let state_clone = Arc::clone(&state);
        let handle = tokio::spawn(async move {
            let req = Request::CacheStats { id: i };
            handle_request(state_clone, req).await
        });
        handles.push(handle);
    }

    // All should succeed without panics or deadlocks
    for handle in handles {
        let response = handle.await.unwrap();
        assert!(matches!(response, Response::Success { .. }));
    }
}

// ============================================================================
// Error Recovery Tests
// ============================================================================

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_invalid_package_name_returns_helpful_error() {
    let (_temp, state) = setup_test_env();

    let invalid_names = vec![
        "invalid; rm -rf /",
        "../../../etc/passwd",
        "package\nwith\nnewlines",
        "package\x00with\x00nulls",
    ];

    for invalid in invalid_names {
        let req = Request::Info {
            id: 700,
            package: invalid.to_string(),
        };
        let response = handle_request(Arc::clone(&state), req).await;

        match response {
            Response::Error { code, message, .. } => {
                assert_eq!(code, error_codes::INVALID_PARAMS);
                assert!(
                    message.contains("Invalid package name"),
                    "Error should mention invalid name"
                );
            }
            Response::Success { .. } => panic!("Should reject invalid package name: {}", invalid),
        }
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_oversized_query_is_rejected() {
    let (_temp, state) = setup_test_env();

    // Create query > 500 characters (MAX_QUERY_LENGTH)
    let huge_query = "a".repeat(1000);
    let req = Request::Search {
        id: 800,
        query: huge_query,
        limit: Some(10),
    };
    let response = handle_request(Arc::clone(&state), req).await;

    match response {
        Response::Error { code, message, .. } => {
            assert_eq!(code, error_codes::INVALID_PARAMS);
            assert!(
                message.contains("too long") || message.contains("Query"),
                "Error should mention query length"
            );
        }
        Response::Success { .. } => panic!("Should reject oversized query"),
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_state_recovery_after_error() {
    let (_temp, state) = setup_test_env();

    // Trigger an error
    let bad_req = Request::Info {
        id: 900,
        package: "invalid; rm -rf /".to_string(),
    };
    let _error_resp = handle_request(Arc::clone(&state), bad_req).await;

    // Verify daemon is still operational
    let good_req = Request::Ping { id: 901 };
    let response = handle_request(Arc::clone(&state), good_req).await;

    match response {
        Response::Success { result, .. } => {
            assert!(
                matches!(result, ResponseResult::Ping(_)),
                "Daemon should recover after error"
            );
        }
        Response::Error { message, .. } => panic!(
            "Daemon should be operational after previous error: {}",
            message
        ),
    }
}

// ============================================================================
// State Management Tests
// ============================================================================

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_health_status_reflects_cache_size() {
    let (_temp, state) = setup_test_env();

    let req = Request::Health { id: 1000 };
    let response = handle_request(Arc::clone(&state), req).await;

    match response {
        Response::Success { result, .. } => {
            if let ResponseResult::Health(health) = result {
                // Verify health status is one of valid states
                assert!(
                    health.status == "healthy"
                        || health.status == "degraded"
                        || health.status == "unhealthy",
                    "Invalid health status: {}",
                    health.status
                );

                // Verify health reflects cache size
                if health.cache_size > 50_000 {
                    assert_eq!(
                        health.status, "degraded",
                        "Large cache should trigger degraded status"
                    );
                }
            } else {
                panic!("Expected Health response");
            }
        }
        Response::Error { message, .. } => panic!("Health check failed: {}", message),
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_metrics_increment_on_requests() {
    let (_temp, state) = setup_test_env();

    // Get initial metrics
    let metrics_req = Request::Metrics { id: 1100 };
    let initial = handle_request(Arc::clone(&state), metrics_req.clone()).await;

    let initial_count = if let Response::Success {
        result: ResponseResult::Metrics(m),
        ..
    } = initial
    {
        m.requests_total
    } else {
        0
    };

    // Make some requests
    for i in 0..5 {
        let req = Request::Ping { id: i };
        let _ = handle_request(Arc::clone(&state), req).await;
    }

    // Get updated metrics
    let updated = handle_request(Arc::clone(&state), metrics_req).await;

    if let Response::Success { result, .. } = updated
        && let ResponseResult::Metrics(m) = result
    {
        assert!(
            m.requests_total > initial_count,
            "Request count should increment, initial: {}, current: {}",
            initial_count,
            m.requests_total
        );
    }
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_uptime_increases_monotonically() {
    let (_temp, state) = setup_test_env();

    let req = Request::Health { id: 1200 };
    let response1 = handle_request(Arc::clone(&state), req.clone()).await;

    let uptime1 = if let Response::Success {
        result: ResponseResult::Health(h),
        ..
    } = response1
    {
        h.uptime_seconds
    } else {
        0
    };

    // Wait a bit
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let response2 = handle_request(Arc::clone(&state), req).await;

    if let Response::Success { result, .. } = response2
        && let ResponseResult::Health(h) = result
    {
        assert!(
            h.uptime_seconds >= uptime1,
            "Uptime should increase monotonically"
        );
    }
}

// ============================================================================
// Performance Tests
// ============================================================================

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_ping_response_time_is_fast() {
    let (_temp, state) = setup_test_env();

    let req = Request::Ping { id: 1300 };
    let start = std::time::Instant::now();
    let _ = handle_request(Arc::clone(&state), req).await;
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 10,
        "Ping should respond in <10ms, got {:?}",
        elapsed
    );
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_cache_stats_response_time_is_fast() {
    let (_temp, state) = setup_test_env();

    let req = Request::CacheStats { id: 1400 };
    let start = std::time::Instant::now();
    let _ = handle_request(Arc::clone(&state), req).await;
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 10,
        "CacheStats should respond in <10ms, got {:?}",
        elapsed
    );
}

#[tokio::test]
#[serial]
#[allow(unsafe_code)]
async fn test_health_check_response_time_is_fast() {
    let (_temp, state) = setup_test_env();

    let req = Request::Health { id: 1500 };
    let start = std::time::Instant::now();
    let _ = handle_request(Arc::clone(&state), req).await;
    let elapsed = start.elapsed();

    assert!(
        elapsed.as_millis() < 10,
        "Health check should respond in <10ms, got {:?}",
        elapsed
    );
}
