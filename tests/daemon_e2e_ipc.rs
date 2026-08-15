#![cfg(feature = "arch")]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::pedantic,
    clippy::nursery
)]

//! S-tier E2E Tests: IPC Communication
//!
//! Comprehensive tests for Unix socket communication, message serialization,
//! request/response handling, timeouts, and connection pooling.

use anyhow::Result;
use omg_lib::daemon::handlers::{DaemonState, handle_request};
use omg_lib::daemon::protocol::{Request, Response, ResponseResult, error_codes};
use serial_test::serial;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::time::{sleep, timeout};

/// Helper to extract response ID
#[expect(dead_code)]
fn response_id(response: &Response) -> u64 {
    match response {
        Response::Success { id, .. } => *id,
        Response::Error { id, .. } => *id,
    }
}

/// Test fixture for IPC tests
struct IpcTestFixture {
    _temp_dir: TempDir,
    socket_path: std::path::PathBuf,
    state: Arc<DaemonState>,
}

impl IpcTestFixture {
    async fn new() -> Result<Self> {
        let temp_dir = TempDir::new()?;
        let socket_path = temp_dir.path().join("test.sock");
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
            socket_path,
            state,
        })
    }

    /// Start a simple IPC server for testing
    async fn start_server(&self) -> Result<tokio::task::JoinHandle<()>> {
        let listener = UnixListener::bind(&self.socket_path)?;
        let state = Arc::clone(&self.state);

        let handle = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let state = Arc::clone(&state);
                tokio::spawn(async move {
                    if let Err(e) = Self::handle_connection(stream, state).await {
                        eprintln!("Connection error: {}", e);
                    }
                });
            }
        });

        // Wait for socket to be ready
        sleep(Duration::from_millis(100)).await;

        Ok(handle)
    }

    async fn handle_connection(stream: UnixStream, state: Arc<DaemonState>) -> Result<()> {
        use futures::{SinkExt, StreamExt};
        use tokio_util::codec::{Framed, LengthDelimitedCodec};

        let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

        while let Some(request_bytes) = framed.next().await {
            let bytes = request_bytes?;
            let request: Request = bitcode::deserialize(&bytes)?;
            let response = handle_request(Arc::clone(&state), request).await;
            let response_bytes = bitcode::serialize(&response)?;
            framed.send(response_bytes.into()).await?;
        }

        Ok(())
    }

    async fn connect(&self) -> Result<UnixStream> {
        Ok(UnixStream::connect(&self.socket_path).await?)
    }

    async fn send_request(&self, stream: &mut UnixStream, request: &Request) -> Result<Response> {
        // Serialize request
        let request_bytes = bitcode::serialize(request)?;
        let len = u32::try_from(request_bytes.len())?;

        // Send length + data
        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(&request_bytes).await?;

        // Read response length
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;

        // Read response data
        let mut resp_bytes = vec![0u8; resp_len];
        stream.read_exact(&mut resp_bytes).await?;

        // Deserialize response
        Ok(bitcode::deserialize(&resp_bytes)?)
    }
}

// ============================================================================
// Test 1: Basic Request-Response
// ============================================================================

#[tokio::test]
#[serial]
async fn test_basic_ping_request() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;
    let request = Request::Ping { id: 1 };
    let response = fixture.send_request(&mut stream, &request).await?;

    match response {
        Response::Success { id, result } => {
            assert_eq!(id, 1);
            assert!(matches!(result, ResponseResult::Ping(_)));
        }
        Response::Error { message, .. } => unreachable!("Ping failed: {}", message),
    }

    Ok(())
}

// ============================================================================
// Test 2: Message Serialization Edge Cases
// ============================================================================

#[tokio::test]
#[serial]
async fn test_large_request_handling() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;

    // Create a large but valid search query
    let large_query = "a".repeat(400); // Within MAX_QUERY_LENGTH (500)
    let request = Request::Search {
        id: 2,
        query: large_query,
        limit: Some(10),
    };

    let response = fixture.send_request(&mut stream, &request).await?;

    let Response::Success { id, result } = response else {
        unreachable!("A valid query within the size limit should succeed");
    };
    assert_eq!(id, 2);
    assert!(
        matches!(result, ResponseResult::Search(_)),
        "A search request should return search results"
    );

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_oversized_query_rejection() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;

    // Create query exceeding MAX_QUERY_LENGTH (500)
    let oversized_query = "x".repeat(600);
    let request = Request::Search {
        id: 3,
        query: oversized_query,
        limit: Some(10),
    };

    let response = fixture.send_request(&mut stream, &request).await?;

    match response {
        Response::Error { code, .. } => {
            assert_eq!(
                code,
                error_codes::INVALID_PARAMS,
                "Oversized query should be rejected"
            );
        }
        Response::Success { .. } => {
            unreachable!("Oversized query should be rejected");
        }
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_empty_request_handling() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;

    // Empty search query
    let request = Request::Search {
        id: 4,
        query: String::new(),
        limit: Some(10),
    };

    let response = fixture.send_request(&mut stream, &request).await?;

    let Response::Success { id, result } = response else {
        unreachable!("An empty search query should return an empty result set");
    };
    assert_eq!(id, 4);
    let ResponseResult::Search(search_result) = result else {
        unreachable!("A search request should return search results");
    };
    assert!(
        search_result.packages.is_empty(),
        "Empty query should return no results"
    );

    Ok(())
}

// ============================================================================
// Test 3: Connection Pooling and Concurrency
// ============================================================================

#[tokio::test]
#[serial]
async fn test_multiple_concurrent_connections() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    // Spawn 10 concurrent clients
    let mut handles = vec![];
    for client_id in 0..10 {
        let socket_path = fixture.socket_path.clone();
        let handle = tokio::spawn(async move {
            let mut stream = UnixStream::connect(&socket_path).await?;
            let request = Request::Ping { id: client_id };

            // Send request
            let request_bytes = bitcode::serialize(&request)?;
            let len = u32::try_from(request_bytes.len())?;
            stream.write_all(&len.to_be_bytes()).await?;
            stream.write_all(&request_bytes).await?;

            // Read response
            let mut len_buf = [0u8; 4];
            stream.read_exact(&mut len_buf).await?;
            let resp_len = u32::from_be_bytes(len_buf) as usize;
            let mut resp_bytes = vec![0u8; resp_len];
            stream.read_exact(&mut resp_bytes).await?;

            let response: Response = bitcode::deserialize(&resp_bytes)?;
            Ok::<_, anyhow::Error>(response)
        });
        handles.push(handle);
    }

    // Wait for all clients
    for handle in handles {
        let response = handle.await??;
        assert!(
            matches!(response, Response::Success { .. }),
            "All clients should succeed"
        );
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_sequential_requests_on_single_connection() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;

    // Send 5 sequential requests on same connection
    for i in 0..5 {
        let request = Request::Ping { id: i };
        let response = fixture.send_request(&mut stream, &request).await?;

        match response {
            Response::Success { id, .. } => {
                assert_eq!(id, i, "Response ID should match request ID");
            }
            Response::Error { message, .. } => {
                unreachable!("Request {} failed: {}", i, message);
            }
        }
    }

    Ok(())
}

// ============================================================================
// Test 4: Timeout Handling
// ============================================================================

#[tokio::test]
#[serial]
async fn test_client_disconnect_during_request() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    {
        let mut stream = fixture.connect().await?;
        let request = Request::Status { id: 200 };

        // Send request
        let request_bytes = bitcode::serialize(&request)?;
        let len = u32::try_from(request_bytes.len())?;
        stream.write_all(&len.to_be_bytes()).await?;
        stream.write_all(&request_bytes).await?;

        // Disconnect immediately without reading response
        drop(stream);
    }

    // Server should handle disconnect gracefully
    sleep(Duration::from_millis(200)).await;

    // New connection should work fine
    let mut stream = fixture.connect().await?;
    let request = Request::Ping { id: 201 };
    let response = fixture.send_request(&mut stream, &request).await?;

    assert!(
        matches!(response, Response::Success { .. }),
        "Server should recover from client disconnect"
    );

    Ok(())
}

// ============================================================================
// Test 5: Protocol Error Handling
// ============================================================================

#[tokio::test]
#[serial]
async fn test_malformed_request_handling() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;

    // Send garbage data
    let garbage = vec![0xFF; 100];
    let len = u32::try_from(garbage.len())?;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(&garbage).await?;

    // Server should either reject or close connection
    // Try to send valid request after
    sleep(Duration::from_millis(100)).await;

    let request = Request::Ping { id: 300 };
    let result = timeout(
        Duration::from_secs(1),
        fixture.send_request(&mut stream, &request),
    )
    .await;

    // Connection might be closed, which is acceptable
    if result.is_err() {
        // Connection closed due to malformed data - expected
        return Ok(());
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_invalid_package_name_injection() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;

    // Attempt command injection
    let request = Request::Info {
        id: 400,
        package: "test; rm -rf /".to_string(),
    };

    let response = fixture.send_request(&mut stream, &request).await?;

    match response {
        Response::Error { code, .. } => {
            assert_eq!(
                code,
                error_codes::INVALID_PARAMS,
                "Injection attempt should be rejected"
            );
        }
        Response::Success { .. } => {
            unreachable!("Injection attempt should be rejected");
        }
    }

    Ok(())
}

// ============================================================================
// Test 6: Batch Request Handling
// ============================================================================

#[tokio::test]
#[serial]
async fn test_batch_request_single_roundtrip() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;

    // Create batch of 5 requests
    let subrequests = vec![
        Request::Ping { id: 1 },
        Request::Ping { id: 2 },
        Request::Ping { id: 3 },
        Request::Ping { id: 4 },
        Request::Ping { id: 5 },
    ];

    let batch_request = Request::Batch {
        id: 500,
        requests: subrequests,
    };

    let start = std::time::Instant::now();
    let response = fixture.send_request(&mut stream, &batch_request).await?;
    let elapsed = start.elapsed();

    match response {
        Response::Success { id, result } => {
            assert_eq!(id, 500);
            if let ResponseResult::Batch(responses) = result {
                assert_eq!(responses.len(), 5, "Batch should contain 5 responses");

                // All should succeed
                for resp in responses.iter() {
                    assert!(matches!(resp, Response::Success { .. }));
                }
            } else {
                unreachable!("Expected Batch response");
            }
        }
        Response::Error { message, .. } => {
            unreachable!("Batch request failed: {}", message);
        }
    }

    // Batch should be faster than 5 individual requests
    println!("Batch request completed in {:?}", elapsed);

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_oversized_batch_rejection() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;

    // Create batch exceeding MAX_BATCH_SIZE (100)
    let subrequests: Vec<Request> = (0..150).map(|i| Request::Ping { id: i }).collect();

    let batch_request = Request::Batch {
        id: 600,
        requests: subrequests,
    };

    let response = fixture.send_request(&mut stream, &batch_request).await?;

    match response {
        Response::Error { code, .. } => {
            assert_eq!(
                code,
                error_codes::INVALID_PARAMS,
                "Oversized batch should be rejected"
            );
        }
        Response::Success { .. } => {
            unreachable!("Oversized batch should be rejected");
        }
    }

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_nested_batch_rejection() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;

    // Create nested batch (batch within batch)
    let inner_batch = Request::Batch {
        id: 1,
        requests: vec![Request::Ping { id: 2 }],
    };

    let outer_batch = Request::Batch {
        id: 700,
        requests: vec![inner_batch],
    };

    let response = fixture.send_request(&mut stream, &outer_batch).await?;

    match response {
        Response::Error { code, .. } => {
            assert_eq!(
                code,
                error_codes::INVALID_PARAMS,
                "Nested batch should be rejected"
            );
        }
        Response::Success { .. } => {
            unreachable!("Nested batch should be rejected");
        }
    }

    Ok(())
}

// ============================================================================
// Test 7: Request ID Validation
// ============================================================================

#[tokio::test]
#[serial]
async fn test_request_id_preservation() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;

    // Test with various ID values
    let test_ids = vec![0, 1, 42, u64::MAX - 1, u64::MAX];

    for test_id in test_ids {
        let request = Request::Ping { id: test_id };
        let response = fixture.send_request(&mut stream, &request).await?;

        match response {
            Response::Success { id, .. } => {
                assert_eq!(id, test_id, "Response ID should match request ID");
            }
            Response::Error { id, .. } => {
                assert_eq!(id, test_id, "Error response ID should match request ID");
            }
        }
    }

    Ok(())
}

// ============================================================================
// Test 8: Connection Lifecycle
// ============================================================================

#[tokio::test]
#[serial]
async fn test_connection_close_after_response() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    {
        let mut stream = fixture.connect().await?;
        let request = Request::Ping { id: 800 };
        let _response = fixture.send_request(&mut stream, &request).await?;

        // Close connection explicitly
        drop(stream);
    }

    // Should be able to create new connection
    let mut stream = fixture.connect().await?;
    let request = Request::Ping { id: 801 };
    let response = fixture.send_request(&mut stream, &request).await?;

    assert!(matches!(response, Response::Success { .. }));

    Ok(())
}

#[tokio::test]
#[serial]
async fn test_idle_connection_stability() -> Result<()> {
    let fixture = IpcTestFixture::new().await?;
    let _server = fixture.start_server().await?;

    let mut stream = fixture.connect().await?;

    // Send request
    let request = Request::Ping { id: 900 };
    let _response = fixture.send_request(&mut stream, &request).await?;

    // Keep connection idle for 2 seconds
    sleep(Duration::from_secs(2)).await;

    // Should still work
    let request = Request::Ping { id: 901 };
    let response = fixture.send_request(&mut stream, &request).await?;

    assert!(
        matches!(response, Response::Success { .. }),
        "Idle connection should remain valid"
    );

    Ok(())
}
