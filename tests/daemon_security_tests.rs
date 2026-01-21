use std::sync::Arc;
use tempfile::TempDir;
use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::protocol::{Request, Response, error_codes};
use serial_test::serial;

#[tokio::test]
#[serial]
async fn test_global_rate_limiting() {
    // Setup temporary environment
    let temp_dir = TempDir::new().unwrap();
    // SAFETY: We are running tests serially now, so this is safe
    unsafe {
        std::env::set_var("OMG_DAEMON_DATA_DIR", temp_dir.path());
        std::env::set_var("OMG_DATA_DIR", temp_dir.path());
    }

    // Initialize audit logger (needed for handlers)
    omg_lib::core::security::init_audit_logger().expect("Failed to init audit logger");

    // Initialize daemon state
    // We need to handle potential failure if the index fails to build,
    // but for tests we expect it to work or fail gracefully
    let state = match DaemonState::new() {
        Ok(s) => Arc::new(s),
        Err(_) => return, // Skip if we can't init (e.g. no package manager)
    };

    // The global rate limit is 100/s with burst 200.
    // We need to exhaust the burst to trigger the limit.

    let req = Request::Ping { id: 1 };

    // Send 250 requests to ensure we hit the limit (burst is 200)
    let mut limit_hit = false;
    for _i in 0..250 {
        let response = handle_request(Arc::clone(&state), req.clone()).await;
        if let Response::Error { code, .. } = response
            && code == error_codes::RATE_LIMITED
        {
            limit_hit = true;
            break;
        }
    }

    assert!(limit_hit, "Should have hit global rate limit");
}

#[tokio::test]
#[serial]
async fn test_input_validation_audit() {
    // Setup temporary environment
    let temp_dir = TempDir::new().unwrap();
    unsafe {
        std::env::set_var("OMG_DAEMON_DATA_DIR", temp_dir.path());
        std::env::set_var("OMG_DATA_DIR", temp_dir.path());
    }

    // Initialize audit logger
    omg_lib::core::security::init_audit_logger().expect("Failed to init audit logger");

    let state = match DaemonState::new() {
        Ok(s) => Arc::new(s),
        Err(_) => return,
    };

    // Send request with invalid package name to trigger audit log
    let invalid_pkg = "invalid; rm -rf /";
    let req = Request::Info {
        id: 1,
        package: invalid_pkg.to_string()
    };

    let response = handle_request(Arc::clone(&state), req).await;

    // Verify rejection
    if let Response::Error { message, .. } = response {
        assert!(message.contains("Invalid package name"), "Should reject invalid package name");
    } else {
        panic!("Should have returned error response");
    }

    // Verify audit log entry
    // The audit log is written to OMG_DATA_DIR/audit/audit.jsonl
    let audit_dir = temp_dir.path().join("audit");
    let audit_file = audit_dir.join("audit.jsonl");

    // Wait a brief moment for async writing if necessary (though audit logging is blocking currently)
    std::thread::sleep(std::time::Duration::from_millis(100));

    if audit_file.exists() {
        let content = std::fs::read_to_string(&audit_file).expect("Audit log file should exist");
        assert!(content.contains("policy_violation"), "Log should contain policy_violation");
        assert!(content.contains("Invalid package name"), "Log should contain error details");
    } else {
        panic!("Audit log file not found at {:?}", audit_file);
    }
}

#[tokio::test]
#[serial]
async fn test_batch_size_limit_audit() {
    // Setup temporary environment
    let temp_dir = TempDir::new().unwrap();
    unsafe {
        std::env::set_var("OMG_DAEMON_DATA_DIR", temp_dir.path());
        std::env::set_var("OMG_DATA_DIR", temp_dir.path());
    }

    // Initialize audit logger
    omg_lib::core::security::init_audit_logger().expect("Failed to init audit logger");

    let state = match DaemonState::new() {
        Ok(s) => Arc::new(s),
        Err(_) => return,
    };

    // Create oversized batch
    let mut requests = Vec::new();
    for _ in 0..150 { // Limit is 100
        requests.push(Request::Ping { id: 1 });
    }

    let req = Request::Batch { id: 1, requests: Box::new(requests) };

    let response = handle_request(Arc::clone(&state), req).await;

    // Verify rejection
    if let Response::Error { message, .. } = response {
        assert!(message.contains("Batch size"), "Should reject oversized batch");
    } else {
        panic!("Should have returned error response");
    }

    // Verify audit log
    let audit_file = temp_dir.path().join("audit").join("audit.jsonl");
    let content = std::fs::read_to_string(audit_file).expect("Audit log file should exist");
    assert!(content.contains("Batch size"), "Log should record batch size violation");
}
