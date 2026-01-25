#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]
use omg_lib::core::metrics::GLOBAL_METRICS;
use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::protocol::{Request, Response, ResponseResult};
use serial_test::serial;
use std::sync::Arc;
use tempfile::TempDir;

#[tokio::test]
#[serial]
async fn test_metrics_collection() {
    // Setup
    let temp_dir = TempDir::new().unwrap();
    unsafe {
        std::env::set_var("OMG_DAEMON_DATA_DIR", temp_dir.path());
        std::env::set_var("OMG_DATA_DIR", temp_dir.path());
    }

    // Initialize
    let _ = omg_lib::core::security::init_audit_logger();
    let state = match DaemonState::new() {
        Ok(s) => Arc::new(s),
        Err(_) => return,
    };

    // Get initial metrics
    let initial = GLOBAL_METRICS.snapshot();

    // 1. Send a successful request
    let req_ping = Request::Ping { id: 1 };
    handle_request(Arc::clone(&state), req_ping).await;

    // Check increment
    let after_ping = GLOBAL_METRICS.snapshot();
    assert_eq!(
        after_ping.requests_total,
        initial.requests_total + 1,
        "Total requests should inc by 1"
    );
    assert_eq!(
        after_ping.requests_failed, initial.requests_failed,
        "Failed requests should stay same"
    );

    // 2. Send an invalid request (validation failure)
    let req_invalid = Request::Info {
        id: 2,
        package: "invalid; bad".to_string(),
    };
    handle_request(Arc::clone(&state), req_invalid).await;

    let after_invalid = GLOBAL_METRICS.snapshot();
    assert_eq!(
        after_invalid.requests_total,
        initial.requests_total + 2,
        "Total requests should inc by 2"
    );
    assert_eq!(
        after_invalid.requests_failed,
        initial.requests_failed + 1,
        "Failed requests should inc by 1"
    );
    assert_eq!(
        after_invalid.validation_failures,
        initial.validation_failures + 1,
        "Validation failures should inc by 1"
    );

    // 3. Request metrics via IPC
    let req_metrics = Request::Metrics { id: 3 };
    let response = handle_request(Arc::clone(&state), req_metrics).await;

    if let Response::Success {
        result: ResponseResult::Metrics(snapshot),
        ..
    } = response
    {
        // The snapshot inside the response should reflect at least the previous state
        // Note: It might count the metrics request itself depending on ordering
        assert!(snapshot.requests_total >= after_invalid.requests_total);
    } else {
        panic!("Expected Metrics response");
    }
}

#[tokio::test]
#[serial]
async fn test_security_audit_metrics() {
    let temp_dir = TempDir::new().unwrap();
    unsafe {
        std::env::set_var("OMG_DAEMON_DATA_DIR", temp_dir.path());
    }
    let state = match DaemonState::new() {
        Ok(s) => Arc::new(s),
        Err(_) => return,
    };

    let initial = GLOBAL_METRICS.snapshot();

    // Send security audit request
    let req = Request::SecurityAudit { id: 1 };
    handle_request(Arc::clone(&state), req).await;

    let after = GLOBAL_METRICS.snapshot();
    assert_eq!(
        after.security_audit_requests,
        initial.security_audit_requests + 1
    );
}
