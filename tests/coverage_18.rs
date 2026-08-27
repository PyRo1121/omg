#![cfg(feature = "arch")]
#![expect(clippy::pedantic)]

//! Coverage 18: contract tests for `handle_client` early-reject paths in
//! `src/daemon/server.rs`, driven end-to-end through the REAL server
//! (`server::run`) over a real Unix socket.
//!
//! Each test pins an observable wire contract:
//! - version mismatch   -> PARSE_ERROR with exact message, id 0, then close
//! - malformed header   -> PARSE_ERROR with exact message, id 0, then close
//! - undecodable body   -> PARSE_ERROR naming the failure, id 0, then close
//!   (+ validation_failures metric)
//! - oversized frame    -> silent teardown: NO response frame, connection EOF
//! - rate-limit burst   -> RATE_LIMITED rejections echoing request ids with
//!   an exact message; connection stays usable after
//!
//! Metric deltas are asserted through the daemon's own Metrics IPC response,
//! so a mutation that drops `inc_requests_failed()` is also caught.

pub mod common;

use anyhow::{Context, Result};
use common::*;
use omg_lib::daemon::handlers::DaemonState;
use omg_lib::daemon::index::PackageIndex;
use omg_lib::daemon::protocol::{Request, Response, ResponseResult, error_codes};
use omg_lib::daemon::server;
use omg_lib::package_managers::mock::MockPackageManager;
use serial_test::serial;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::time::timeout;

/// Mirror of the private `MAX_REQUEST_SIZE` in `src/daemon/server.rs`
/// (the `LengthDelimitedCodec` cap). If the product raises its real cap past
/// this value, the oversized-frame test below fails loudly instead of
/// passing vacuously against a moved threshold.
const REQUEST_WIRE_CAP: usize = 1024 * 1024;

/// Per-read timeout. Long enough for loaded CI machines, short enough that a
/// "server went silent" regression fails fast.
const READ_TIMEOUT: Duration = Duration::from_secs(10);

/// Burst size sent at the rate limiter (product burst budget is 100).
const RATE_BURST_N: usize = 150;
const RATE_LIMITED_MESSAGE: &str = "Rate limit exceeded. Please slow down.";

// ═══════════════════════════════════════════════════════════════════════════════
// Fixture: the REAL daemon server on a private Unix socket
// ═══════════════════════════════════════════════════════════════════════════════

struct RealServerFixture {
    _temp_dir: TempDir,
    socket_path: PathBuf,
}

impl RealServerFixture {
    async fn new() -> Result<Self> {
        init_test_env();
        let temp_dir = TempDir::new().context("failed to create fixture temp dir")?;
        let data_dir = temp_dir.path().join("data");
        std::fs::create_dir_all(&data_dir)?;

        // Scoped env: audit logger and persistent cache capture their data-dir
        // paths during construction (same isolation pattern as daemon_e2e_ipc).
        let state = Arc::new(temp_env::with_vars(
            [
                ("OMG_DAEMON_DATA_DIR", Some(data_dir.as_os_str())),
                ("OMG_DATA_DIR", Some(data_dir.as_os_str())),
            ],
            || -> anyhow::Result<_> {
                omg_lib::core::security::init_audit_logger()?;
                DaemonState::new_isolated(
                    &data_dir,
                    PackageIndex::empty(),
                    Arc::new(MockPackageManager::new_in("arch", &data_dir)),
                )
            },
        )?);

        let socket_path = temp_dir.path().join("cov18.sock");
        let listener = UnixListener::bind(&socket_path)?;
        tokio::spawn(server::run(
            listener,
            Arc::clone(&state),
            socket_path.clone(),
        ));

        // Wait until the accept loop is actually serving connections.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if UnixStream::connect(&socket_path).await.is_ok() {
                break;
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "real daemon never became connectable on {}",
                socket_path.display()
            );
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        Ok(Self {
            _temp_dir: temp_dir,
            socket_path,
        })
    }

    async fn connect(&self) -> Result<UnixStream> {
        UnixStream::connect(&self.socket_path)
            .await
            .with_context(|| format!("connect to {}", self.socket_path.display()))
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Raw-frame client helpers
// ═══════════════════════════════════════════════════════════════════════════════

/// Write one length-delimited frame (BE u32 length prefix + payload), exactly
/// what the daemon's `LengthDelimitedCodec` expects.
async fn send_raw_frame(stream: &mut UnixStream, payload: &[u8]) -> Result<()> {
    let len = u32::try_from(payload.len()).context("frame payload exceeds u32 length prefix")?;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(payload).await?;
    stream.flush().await?;
    Ok(())
}

/// Read exactly one frame. `Ok(None)` means clean EOF before any byte of a
/// new frame arrived (connection closed by the peer).
async fn try_read_raw_frame(stream: &mut UnixStream) -> std::io::Result<Option<Vec<u8>>> {
    let mut len_buf = [0u8; 4];
    match stream.read_exact(&mut len_buf).await {
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut buf = vec![0u8; len];
    stream.read_exact(&mut buf).await?;
    Ok(Some(buf))
}

fn decode_response(frame: &[u8]) -> Result<Response> {
    let (_, payload) = omg_lib::daemon::protocol::split_frame(frame)?;
    Ok(bitcode::deserialize(payload)?)
}

/// Read one framed response within [`READ_TIMEOUT`]. Errors if the connection
/// closes first or the peer goes silent.
async fn read_response(stream: &mut UnixStream) -> Result<Response> {
    let fut = try_read_raw_frame(stream);
    let frame = timeout(READ_TIMEOUT, fut)
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for a response frame"))?
        .map_err(|e| anyhow::anyhow!("I/O error reading response frame: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("connection closed before a response frame arrived"))?;
    decode_response(&frame)
}

/// Assert the daemon tears the connection down cleanly: clean EOF within
/// [`READ_TIMEOUT`], and no further frame bytes were pushed onto the wire.
async fn expect_eof(stream: &mut UnixStream, ctx: &str) {
    match timeout(READ_TIMEOUT, try_read_raw_frame(stream)).await {
        Err(_) => panic!("{ctx}: expected the server to close the connection, but it stayed open"),
        Ok(Err(e)) => panic!("{ctx}: expected clean EOF, got I/O error: {e}"),
        Ok(Ok(None)) => {}
        Ok(Ok(Some(bytes))) => panic!(
            "{ctx}: expected EOF, but the server sent another frame of {} bytes",
            bytes.len()
        ),
    }
}

/// Read the daemon's own `requests_failed` metric through the Metrics IPC
/// request, so metric-delta assertions exercise the real serving path.
async fn requests_failed_probe(fixture: &RealServerFixture) -> Result<u64> {
    let mut stream = fixture.connect().await?;
    let bytes = omg_lib::daemon::protocol::encode_frame(&Request::Metrics { id: 0xBEEF })?;
    send_raw_frame(&mut stream, &bytes).await?;
    match read_response(&mut stream).await? {
        Response::Success {
            result: ResponseResult::Metrics(snapshot),
            ..
        } => Ok(snapshot.requests_failed),
        other => Err(anyhow::anyhow!(
            "expected a Metrics snapshot response, got {other:?}"
        )),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Contract 1: protocol-version mismatch rejection
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial]
async fn version_mismatch_gets_exact_parse_error_then_connection_closes() -> Result<()> {
    let fixture = RealServerFixture::new().await?;
    let baseline = requests_failed_probe(&fixture).await?;

    let mut stream = fixture.connect().await?;
    // A well-formed Ping from a peer speaking a future protocol version.
    let ping = Request::Ping { id: 77 };
    let mut frame = Vec::new();
    frame.extend_from_slice(&999_001u32.to_le_bytes());
    frame.extend_from_slice(&bitcode::serialize(&ping)?);
    send_raw_frame(&mut stream, &frame).await?;

    match read_response(&mut stream).await? {
        Response::Error { id, code, message } => {
            assert_eq!(
                id, 0,
                "undecodable requests must be answered on reserved error id 0"
            );
            assert_eq!(
                code,
                error_codes::PARSE_ERROR,
                "version mismatch must surface as PARSE_ERROR"
            );
            assert_eq!(
                message,
                "unsupported peer protocol version 999001 (this daemon speaks 1); update omg",
                "rejection must tell the client WHY (peer version) and WHAT to do"
            );
        }
        Response::Success { .. } => {
            panic!("a peer speaking protocol version 999001 must not be served");
        }
    }
    // The stream is unusable afterwards: the daemon must close it, not hang.
    expect_eof(&mut stream, "after version-mismatch rejection").await;

    let after = requests_failed_probe(&fixture).await?;
    assert_eq!(
        after,
        baseline + 1,
        "each version-mismatch rejection must count exactly one failed request metric"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Contract 2: frame shorter than the version header
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial]
async fn frame_too_short_for_header_gets_parse_error_then_connection_closes() -> Result<()> {
    let fixture = RealServerFixture::new().await?;
    let baseline = requests_failed_probe(&fixture).await?;

    let mut stream = fixture.connect().await?;
    // Two junk bytes cannot contain the 4-byte version header.
    send_raw_frame(&mut stream, &[0xDE, 0xAD]).await?;

    match read_response(&mut stream).await? {
        Response::Error { id, code, message } => {
            assert_eq!(id, 0);
            assert_eq!(
                code,
                error_codes::PARSE_ERROR,
                "short frames must surface as PARSE_ERROR"
            );
            assert_eq!(
                message,
                "malformed frame header: frame too short to contain the protocol version header",
                "the daemon must name the framing failure verbatim"
            );
        }
        Response::Success { .. } => {
            panic!("a frame without a complete version header must not be served");
        }
    }
    expect_eof(&mut stream, "after malformed-header rejection").await;

    let after = requests_failed_probe(&fixture).await?;
    assert_eq!(
        after,
        baseline + 1,
        "malformed header must bump requests_failed"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Contract 3: medium frame reaches protocol parsing
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial]
async fn four_kib_frame_reaches_protocol_parser_before_rejection() -> Result<()> {
    let fixture = RealServerFixture::new().await?;
    let baseline = requests_failed_probe(&fixture).await?;
    let mut stream = fixture.connect().await?;

    // Above a mutated 2 KiB cap but comfortably below the documented 1 MiB
    // cap. The zeroed protocol header is malformed, so acceptance is observed
    // as one PARSE_ERROR response followed by connection close.
    send_raw_frame(&mut stream, &vec![0; 4 * 1024]).await?;
    match read_response(&mut stream).await? {
        Response::Error { id, code, message } => {
            assert_eq!(id, 0);
            assert_eq!(code, error_codes::PARSE_ERROR);
            assert!(message.contains("protocol version") || message.contains("frame header"));
        }
        other => anyhow::bail!("expected parse-error response, got {other:?}"),
    }
    expect_eof(&mut stream, "4 KiB malformed frame").await;

    let after = requests_failed_probe(&fixture).await?;
    assert_eq!(after, baseline + 1);
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Contract 4: correct version header, undecodable payload
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial]
async fn undecodable_payload_gets_parse_error_validation_failure_then_close() -> Result<()> {
    let fixture = RealServerFixture::new().await?;
    let baseline = requests_failed_probe(&fixture).await?;

    let mut stream = fixture.connect().await?;
    let mut frame = Vec::new();
    frame.extend_from_slice(&1u32.to_le_bytes()); // current PROTOCOL_VERSION
    frame.extend_from_slice(&[0xFFu8; 100]); // garbage payload
    send_raw_frame(&mut stream, &frame).await?;

    match read_response(&mut stream).await? {
        Response::Error { id, code, message } => {
            assert_eq!(id, 0, "requests that never decoded must answer on id 0");
            assert_eq!(code, error_codes::PARSE_ERROR);
            assert!(
                message.starts_with("Failed to deserialize request: "),
                "deserialization failure must name the cause, got: {message}"
            );
        }
        Response::Success { .. } => {
            panic!("an undecodable payload must never be dispatched to a handler");
        }
    }
    expect_eof(&mut stream, "after deserialization failure").await;

    let after = requests_failed_probe(&fixture).await?;
    assert_eq!(after, baseline + 1);

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Contract 5: frame exceeding MAX_REQUEST_SIZE tears down WITHOUT a response
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial]
async fn oversized_frame_tears_down_silently_without_any_response_frame() -> Result<()> {
    let fixture = RealServerFixture::new().await?;
    let baseline = requests_failed_probe(&fixture).await?;

    let mut stream = fixture.connect().await?;
    // Announce a frame larger than the codec's max, then dribble some body
    // bytes so a codec that ignores its cap would keep waiting (and the test
    // would time out -> fail) rather than see EOF.
    let announced = (REQUEST_WIRE_CAP + 1) as u32;
    stream.write_all(&announced.to_be_bytes()).await?;
    stream.write_all(&[0u8; 4096]).await?;
    stream.flush().await?;

    // Contract: NO response frame, then teardown. A hard reset also proves
    // teardown, so ConnectionReset/ConnectionAborted/BrokenPipe pass too.
    let outcome = timeout(READ_TIMEOUT, try_read_raw_frame(&mut stream)).await;
    match outcome {
        Err(_) => panic!(
            "oversized frame: server neither answered nor closed the connection \
             (codec cap missing?)"
        ),
        Ok(Err(e))
            if matches!(
                e.kind(),
                std::io::ErrorKind::ConnectionReset
                    | std::io::ErrorKind::ConnectionAborted
                    | std::io::ErrorKind::BrokenPipe
            ) => {}
        Ok(Err(e)) => panic!("oversized frame: unexpected I/O error: {e}"),
        Ok(Ok(None)) => {}
        Ok(Ok(Some(bytes))) => panic!(
            "oversized frame must be rejected silently, but the server sent a \
             {}-byte response frame",
            bytes.len()
        ),
    }

    let after = requests_failed_probe(&fixture).await?;
    assert_eq!(
        after,
        baseline + 1,
        "frame-decode failure must bump requests_failed even though nothing is answered"
    );
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════════
// Contract 6: rate limit rejects bursts with exact envelope, keeps connection
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
#[serial]
async fn rate_limited_burst_rejects_with_exact_envelope_and_keeps_connection_open() -> Result<()> {
    let fixture = RealServerFixture::new().await?;
    let mut stream = fixture.connect().await?;

    // Fire 150 pings back-to-back inside the ~100-request burst budget.
    let frames: Vec<Vec<u8>> = (0..RATE_BURST_N)
        .map(|i| {
            omg_lib::daemon::protocol::encode_frame(&Request::Ping { id: i as u64 })
                .map_err(anyhow::Error::from)
        })
        .collect::<Result<_>>()?;
    for frame in &frames {
        send_raw_frame(&mut stream, frame).await?;
    }

    let mut responses = Vec::with_capacity(RATE_BURST_N);
    for _ in 0..RATE_BURST_N {
        responses.push(read_response(&mut stream).await?);
    }

    let limited_idx: Vec<usize> = responses
        .iter()
        .enumerate()
        .filter(|(_, r)| {
            matches!(
                r,
                Response::Error { code, .. } if *code == error_codes::RATE_LIMITED
            )
        })
        .map(|(i, _)| i)
        .collect();

    assert!(
        !limited_idx.is_empty(),
        "a burst beyond the per-connection rate budget must produce at least one \
         RATE_LIMITED rejection (limiter removed or burst raised?)"
    );

    // The very first request of a fresh connection must never be rejected:
    // pins the burst allowance against a zero-budget regression.
    assert!(
        matches!(responses[0], Response::Success { .. }),
        "first request on a fresh connection must not be rate-limited"
    );

    for &i in &limited_idx {
        match &responses[i] {
            Response::Error { id, code, message } => {
                assert_eq!(*code, error_codes::RATE_LIMITED);
                assert_eq!(
                    *id, i as u64,
                    "rejection must echo the offending request's id"
                );
                assert_eq!(
                    message, RATE_LIMITED_MESSAGE,
                    "rate-limit rejection must carry the exact operator-facing message"
                );
            }
            Response::Success { .. } => unreachable!("filtered above"),
        }
    }
    // Everything outside the rejections must have been served successfully,
    // with matching ids: the limiter must not corrupt unrelated traffic.
    for (i, response) in responses.iter().enumerate() {
        if limited_idx.contains(&i) {
            continue;
        }
        match response {
            Response::Success { id, .. } => {
                assert_eq!(*id, i as u64, "served response must echo its request id");
            }
            other => panic!("request {i} was neither served nor rate-limited, got {other:?}"),
        }
    }

    // Rate limiting uses `continue`, not `break`: after tokens refill, the
    // SAME connection must serve again.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let follow_up = omg_lib::daemon::protocol::encode_frame(&Request::Ping { id: 4242 })?;
    send_raw_frame(&mut stream, &follow_up).await?;
    match read_response(&mut stream).await? {
        Response::Success { id, .. } => assert_eq!(id, 4242),
        other => panic!("connection must stay usable after a rate-limit rejection, got {other:?}"),
    }
    Ok(())
}
