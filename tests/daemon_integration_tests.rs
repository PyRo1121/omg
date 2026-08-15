#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]

//! Comprehensive daemon integration tests
//! Tests IPC protocol, caching, concurrency, error handling

use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::index::PackageIndex;
use omg_lib::daemon::protocol::{Request, Response, ResponseResult, error_codes};
use omg_lib::package_managers::mock::MockPackageManager;
use serial_test::serial;
use std::sync::Arc;
use tempfile::TempDir;

fn setup_test_env() -> (TempDir, Arc<DaemonState>) {
    let temp_dir = TempDir::new().unwrap();

    temp_env::with_var("OMG_DATA_DIR", Some(temp_dir.path()), || {
        omg_lib::core::security::init_audit_logger()
    })
    .expect("Failed to init audit logger");

    let package_manager = Arc::new(MockPackageManager::new_in("arch", temp_dir.path()));
    let state = Arc::new(
        DaemonState::new_isolated(temp_dir.path(), PackageIndex::empty(), package_manager)
            .expect("Failed to create hermetic DaemonState"),
    );

    (temp_dir, state)
}

// ============================================================================
// IPC Protocol Tests - Test all request/response variants
// ============================================================================

#[tokio::test]
#[serial]
async fn test_metrics_endpoint_returns_valid_data() {
    let (_temp, state) = setup_test_env();

    let req = Request::Metrics { id: 100 };
    let response = handle_request(Arc::clone(&state), req).await;

    let Response::Success {
        id: 100,
        result: ResponseResult::Metrics(metrics),
    } = response
    else {
        panic!("expected metrics response, got {response:?}");
    };
    assert!(
        metrics.requests_total >= 1,
        "the metrics request itself must be counted"
    );
}

#[tokio::test]
#[serial]
async fn test_suggest_endpoint_handles_empty_query() {
    let (_temp, state) = setup_test_env();

    let req = Request::Suggest {
        id: 200,
        query: "".to_string(),
        limit: Some(10),
    };
    let response = handle_request(Arc::clone(&state), req).await;

    assert!(matches!(
        response,
        Response::Success {
            id: 200,
            result: ResponseResult::Suggest(ref suggestions),
        } if suggestions.is_empty()
    ));
}

#[tokio::test]
#[serial]
async fn test_batch_request_executes_all_subcommands() {
    let (_temp, state) = setup_test_env();

    let subrequests = vec![
        Request::Ping { id: 1 },
        Request::Ping { id: 2 },
        Request::Ping { id: 3 },
    ];

    let req = Request::Batch {
        id: 300,
        requests: subrequests,
    };
    let response = handle_request(Arc::clone(&state), req).await;

    let Response::Success {
        id: 300,
        result: ResponseResult::Batch(responses),
    } = response
    else {
        panic!("expected batch response, got {response:?}");
    };
    assert_eq!(responses.len(), 3, "Should execute all 3 subrequests");

    let mut response_ids = Vec::with_capacity(responses.len());
    for response in responses {
        let Response::Success {
            id,
            result: ResponseResult::Ping(message),
        } = response
        else {
            panic!("expected successful ping subresponse, got {response:?}");
        };
        assert_eq!(message, "pong");
        response_ids.push(id);
    }
    response_ids.sort_unstable();
    assert_eq!(response_ids, [1, 2, 3]);
}

#[tokio::test]
#[serial]
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
        requests: subrequests,
    };
    let response = handle_request(Arc::clone(&state), req).await;

    let Response::Success {
        id: 301,
        result: ResponseResult::Batch(responses),
    } = response
    else {
        panic!("expected mixed batch response, got {response:?}");
    };
    assert_eq!(responses.len(), 3);

    let mut seen_ids = Vec::with_capacity(responses.len());
    for response in responses {
        match response {
            Response::Success {
                id: id @ (1 | 3),
                result: ResponseResult::Ping(message),
            } => {
                assert_eq!(message, "pong");
                seen_ids.push(id);
            }
            Response::Error {
                id: 2,
                code: error_codes::INVALID_PARAMS,
                message,
            } => {
                assert!(message.contains("Invalid package name"));
                seen_ids.push(2);
            }
            other => panic!("unexpected mixed batch subresponse: {other:?}"),
        }
    }
    seen_ids.sort_unstable();
    assert_eq!(seen_ids, [1, 2, 3]);
}

// ============================================================================
// Caching Logic Tests
// ============================================================================

#[tokio::test]
#[serial]
async fn test_cache_clear_invalidates_cache() {
    let (_temp, state) = setup_test_env();
    state.cache.insert("fixture".to_string(), Vec::new());
    state.cache.sync();

    let stats_before = handle_request(Arc::clone(&state), Request::CacheStats { id: 500 }).await;
    let Response::Success {
        id: 500,
        result: ResponseResult::CacheStats { size: 1, .. },
    } = stats_before
    else {
        panic!("expected one cached entry before clear, got {stats_before:?}");
    };

    let clear_response = handle_request(Arc::clone(&state), Request::CacheClear { id: 501 }).await;
    assert!(matches!(
        clear_response,
        Response::Success {
            id: 501,
            result: ResponseResult::Message(ref message),
        } if message == "cleared"
    ));

    let stats_after = handle_request(state, Request::CacheStats { id: 502 }).await;
    assert!(matches!(
        stats_after,
        Response::Success {
            id: 502,
            result: ResponseResult::CacheStats { size: 0, .. },
        }
    ));
}

// ============================================================================
// Error Recovery Tests
// ============================================================================

#[tokio::test]
#[serial]
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
            Response::Error { id, code, message } => {
                assert_eq!(id, 700);
                assert_eq!(code, error_codes::INVALID_PARAMS);
                assert!(
                    message.contains("Invalid package name"),
                    "Error should mention invalid name"
                );
            }
            Response::Success { .. } => {
                unreachable!("Should reject invalid package name: {}", invalid)
            }
        }
    }
}

#[tokio::test]
#[serial]
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
        Response::Error { id, code, message } => {
            assert_eq!(id, 800);
            assert_eq!(code, error_codes::INVALID_PARAMS);
            assert!(
                message.contains("too long") || message.contains("Query"),
                "Error should mention query length"
            );
        }
        Response::Success { .. } => unreachable!("Should reject oversized query"),
    }
}

#[tokio::test]
#[serial]
async fn test_state_recovery_after_error() {
    let (_temp, state) = setup_test_env();

    // Trigger an error
    let bad_req = Request::Info {
        id: 900,
        package: "invalid; rm -rf /".to_string(),
    };
    let error_response = handle_request(Arc::clone(&state), bad_req).await;
    assert!(matches!(
        error_response,
        Response::Error {
            id: 900,
            code: error_codes::INVALID_PARAMS,
            ..
        }
    ));

    let response = handle_request(state, Request::Ping { id: 901 }).await;
    assert!(matches!(
        response,
        Response::Success {
            id: 901,
            result: ResponseResult::Ping(ref message),
        } if message == "pong"
    ));
}

// ============================================================================
// State Management Tests
// ============================================================================

#[tokio::test]
#[serial]
async fn test_health_status_reflects_cache_size() {
    let (_temp, state) = setup_test_env();

    let req = Request::Health { id: 1000 };
    let response = handle_request(Arc::clone(&state), req).await;

    let Response::Success {
        id: 1000,
        result: ResponseResult::Health(health),
    } = response
    else {
        panic!("expected health response, got {response:?}");
    };
    assert_eq!(health.cache_size, 0);
    assert_eq!(health.status, "healthy");
}

#[tokio::test]
#[serial]
async fn test_metrics_increment_on_requests() {
    let (_temp, state) = setup_test_env();

    // Get initial metrics
    let metrics_req = Request::Metrics { id: 1100 };
    let initial = handle_request(Arc::clone(&state), metrics_req.clone()).await;

    let Response::Success {
        id: 1100,
        result: ResponseResult::Metrics(initial_metrics),
    } = initial
    else {
        panic!("expected initial metrics response, got {initial:?}");
    };

    for id in 0..5 {
        let response = handle_request(Arc::clone(&state), Request::Ping { id }).await;
        assert!(matches!(
            response,
            Response::Success {
                id: response_id,
                result: ResponseResult::Ping(ref message),
            } if response_id == id && message == "pong"
        ));
    }

    let updated = handle_request(Arc::clone(&state), metrics_req).await;
    let Response::Success {
        id: 1100,
        result: ResponseResult::Metrics(updated_metrics),
    } = updated
    else {
        panic!("expected updated metrics response, got {updated:?}");
    };
    assert_eq!(
        updated_metrics.requests_total,
        initial_metrics.requests_total + 6,
        "five pings and the final metrics request must be counted"
    );
}
