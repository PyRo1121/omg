//! Daemon server implementation with Unix socket IPC
//!
//! Uses `LengthDelimitedCodec` and bitcode for maximum IPC performance.

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use governor::{Quota, RateLimiter};
use tokio::net::UnixListener;
use tokio_util::codec::{FramedWrite, LengthDelimitedCodec};
use tokio_util::sync::CancellationToken;

use super::handlers::{DaemonState, handle_request};
use super::protocol::{
    ExplicitResult, Request, Response, ResponseResult, SearchResult, SecurityAuditResult,
    error_codes,
};
use crate::core::metrics::GLOBAL_METRICS;
use crate::core::security::{
    AuditEventType, AuditSeverity, audit_log_nonblocking, init_audit_logger,
};

/// Request handling timeout (30 seconds should be sufficient for most operations)
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Status refresh interval (5 minutes)
const STATUS_REFRESH_INTERVAL: Duration = Duration::from_mins(5);

/// Memory cleanup interval (30 minutes) - matches mmap TTL
const MEMORY_CLEANUP_INTERVAL: Duration = Duration::from_mins(30);

/// Socket health check interval (60 seconds) - detect deleted socket files
const SOCKET_HEALTH_CHECK_INTERVAL: Duration = Duration::from_mins(1);

/// Close clients that hold a connection without sending a complete frame.
const CLIENT_IDLE_TIMEOUT: Duration = Duration::from_mins(1);

/// Bound response writes so a local client that stops reading cannot retain a
/// connection permit indefinitely.
const CLIENT_WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Per-connection rate limit (requests per second)
const CLIENT_RATE_LIMIT_HZ: u32 = 50;
/// Per-connection burst size
const CLIENT_BURST_SIZE: u32 = 100;

/// Upper bound on concurrently served client connections. Each connection
/// holds framed buffers and a rate limiter; without a cap a local client can
/// exhaust memory/file descriptors before the accept-loop EMFILE backoff
/// ever triggers.
const MAX_CONCURRENT_CONNECTIONS: usize = 128;

async fn wait_for_termination_signal() -> Result<()> {
    #[cfg(unix)]
    {
        let mut signal = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
        signal
            .recv()
            .await
            .context("SIGTERM listener closed unexpectedly")?;
        Ok(())
    }

    #[cfg(not(unix))]
    std::future::pending::<Result<()>>().await
}

/// Run the daemon server
pub async fn run(
    listener: UnixListener,
    state: Arc<DaemonState>,
    socket_path: PathBuf,
) -> Result<()> {
    init_audit_logger()?;
    run_with_status_path(
        listener,
        state,
        socket_path,
        crate::core::paths::fast_status_path(),
    )
    .await
}

async fn write_fast_status_async(
    status: crate::core::fast_status::FastStatus,
    path: PathBuf,
) -> Result<()> {
    tokio::task::spawn_blocking(move || status.write_to_file(&path))
        .await
        .context("Fast-status writer panicked")??;
    Ok(())
}

fn validate_signal_result(result: std::io::Result<()>, signal: &str) -> Result<()> {
    result.with_context(|| format!("Failed to listen for {signal}"))
}

fn daemon_shutdown_result(internal_failure: Option<String>) -> Result<()> {
    match internal_failure {
        Some(failure) => anyhow::bail!(failure),
        None => Ok(()),
    }
}

async fn run_with_status_path(
    listener: UnixListener,
    state: Arc<DaemonState>,
    socket_path: PathBuf,
    fast_status_path: PathBuf,
) -> Result<()> {
    let shutdown_token = CancellationToken::new();
    let (internal_failure_tx, mut internal_failure_rx) =
        tokio::sync::mpsc::unbounded_channel::<String>();

    // Budget for concurrent client connections; permits are released when a
    // connection task ends.
    let connection_permits = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CONNECTIONS));

    // START BACKGROUND WORKER
    let state_worker = Arc::clone(&state);
    let worker_token = shutdown_token.child_token();
    // Clone the parent token so the health check can trigger a full shutdown
    let shutdown_trigger = shutdown_token.clone();
    let health_failure_tx = internal_failure_tx.clone();

    let worker_handle = tokio::spawn(async move {
        tracing::info!("Background status worker started");

        async fn refresh_status(state: &Arc<DaemonState>, fast_status_path: &std::path::Path) {
            let versions = match tokio::task::spawn_blocking(|| {
                use crate::cli::runtimes::{ensure_active_version, known_runtimes};

                let mut versions = Vec::new();
                match known_runtimes() {
                    Ok(runtimes) => {
                        for runtime in runtimes {
                            match ensure_active_version(&runtime) {
                                Ok(Some(v)) => versions.push((runtime, v)),
                                Ok(None) => {}
                                Err(error) => tracing::warn!(
                                    "Failed to resolve active {runtime} version: {error}"
                                ),
                            }
                        }
                    }
                    Err(error) => tracing::warn!("Failed to list known runtimes: {error}"),
                }
                versions
            })
            .await
            {
                Ok(versions) => versions,
                Err(error) => {
                    tracing::error!("Runtime status task panicked: {error}");
                    return;
                }
            };
            let status = state.status_counts().await;
            state
                .runtime_versions
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone_from(&versions);

            let (total, explicit, orphans, updates) = match status {
                Ok(status) => status,
                Err(error) => {
                    tracing::warn!("Failed to refresh package status: {error}");
                    return;
                }
            };
            let fast_status =
                crate::core::fast_status::FastStatus::new(total, explicit, orphans, updates);
            if let Err(error) =
                write_fast_status_async(fast_status, fast_status_path.to_path_buf()).await
            {
                tracing::warn!("Failed to write fast status file: {error}");
            }

            let scanner = crate::core::security::VulnerabilityScanner::new();
            let previous_vulns = state
                .cache
                .get_status()
                .and_then(|status| status.scanned_vulnerability_count());
            let scan = scanner.scan_system().await;
            if let Err(error) = &scan {
                tracing::warn!("Vulnerability scan failed during status refresh: {error}");
            }
            let Some(vuln_count) =
                super::status_policy::vulnerability_count_from_scan(&scan, previous_vulns)
            else {
                tracing::warn!("No prior vulnerability count; not publishing a zero-vuln status");
                return;
            };
            let res = super::status_policy::status_snapshot(
                total,
                explicit,
                orphans,
                updates,
                versions,
                Some(vuln_count),
            )
            .0;
            let res_arc = Arc::new(res);
            // The in-memory cache is authoritative, so persistence is best-effort.
            if let Err(error) = state.persistent.set_status(&res_arc) {
                tracing::warn!("Failed to persist status cache: {error}");
            }
            state.cache.update_status(res_arc);
        }

        /// Pre-compute caches for instant first queries.
        ///
        /// Deliberately unconditional: it must run even when status
        /// publication was skipped above (structural guarantee against the
        /// early-return regression this replaces).
        async fn prewarm_caches(state: &Arc<DaemonState>) {
            // Pre-compute explicit package list for instant first query.
            // The state owns the backend choice so isolated daemons never
            // fall through to a host package database.
            match state.explicit_packages().await {
                Ok(packages) => {
                    state.cache.update_explicit(packages);
                    tracing::debug!("Pre-warmed explicit package cache");
                }
                Err(error) => {
                    tracing::warn!("Failed to pre-warm explicit package cache: {error}");
                }
            }

            // Pre-warm search cache with common queries for instant first searches
            let state_search = Arc::clone(state);
            if let Err(error) = tokio::task::spawn_blocking(move || {
                let common_queries = ["", "linux", "python", "node", "firefox", "git"];
                let index = state_search.index_snapshot();
                for query in common_queries {
                    let results = Arc::new(index.search(query, 50));
                    if !state_search.with_current_index(&index, || {
                        state_search
                            .cache
                            .insert_arc(query.to_string(), Arc::clone(&results));
                    }) {
                        break;
                    }
                }
                tracing::debug!(
                    "Pre-warmed search cache with {} common queries",
                    common_queries.len()
                );
            })
            .await
            {
                tracing::warn!("Search cache pre-warm task failed: {error}");
            }
        }

        // Initial refresh
        refresh_status(&state_worker, &fast_status_path).await;
        prewarm_caches(&state_worker).await;

        // Track last cleanup time for periodic mmap cleanup
        let mut last_cleanup = std::time::Instant::now();
        let mut socket_health = tokio::time::interval(SOCKET_HEALTH_CHECK_INTERVAL);
        socket_health.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        // Consume the immediate first tick; the listener was just bound.
        socket_health.tick().await;

        loop {
            tokio::select! {
                // biased: always check cancellation first to ensure prompt shutdown
                biased;

                () = worker_token.cancelled() => {
                    tracing::info!("Background worker shutting down");
                    break;
                }
                () = tokio::time::sleep(STATUS_REFRESH_INTERVAL) => {
                    tracing::debug!("Refreshing system status cache...");
                    refresh_status(&state_worker, &fast_status_path).await;
                    // Independent of status publication: a failed scan must
                    // not degrade first-query latency for unrelated paths.
                    prewarm_caches(&state_worker).await;
                    tracing::debug!("Status cache refreshed");

                    // Periodic mmap cleanup (every 30 min) to prevent 500MB+ memory leaks
                    if last_cleanup.elapsed() >= MEMORY_CLEANUP_INTERVAL {
                        #[cfg(any(feature = "debian", feature = "debian-pure"))]
                        {
                            crate::package_managers::debian_db::cleanup_expired_mmaps();
                        }
                        last_cleanup = std::time::Instant::now();
                    }
                }
                _ = socket_health.tick() => {
                    if !socket_path.exists() {
                        let failure = format!(
                            "Daemon socket {} was removed externally",
                            socket_path.display()
                        );
                        tracing::error!("{failure}; initiating shutdown");
                        let _ = health_failure_tx.send(failure);
                        // Cancel the parent shutdown token to stop the accept loop.
                        shutdown_trigger.cancel();
                        break;
                    }
                }
            }
        }
    });

    // Observe the singleton worker: an unobserved panic would silently freeze
    // every status-refresh and cache-pre-warm path, so count the failure and
    // shut down cleanly rather than serving stale data forever.
    {
        let state_monitor = Arc::clone(&state);
        let shutdown_monitor = shutdown_token.clone();
        let worker_failure_tx = internal_failure_tx;
        tokio::spawn(async move {
            if let Err(error) = worker_handle.await {
                state_monitor.inc_background_worker_failures();
                let failure = format!("Background status worker terminated unexpectedly: {error}");
                tracing::error!("{failure}; initiating shutdown");
                let _ = worker_failure_tx.send(failure);
                shutdown_monitor.cancel();
            }
        });
    }

    tracing::info!("Daemon ready, binary IPC enabled");

    // Register signal listeners once. Recreating them after every accepted
    // connection leaves gaps where process signals can be lost.
    let ctrl_c = tokio::signal::ctrl_c();
    tokio::pin!(ctrl_c);
    let termination_signal = wait_for_termination_signal();
    tokio::pin!(termination_signal);

    let mut internal_failure = None;
    loop {
        tokio::select! {
            // biased: always check shutdown signal first to avoid accepting
            // new connections after shutdown was requested
            biased;

            Some(failure) = internal_failure_rx.recv() => {
                internal_failure = Some(failure);
                shutdown_token.cancel();
                break;
            }

            result = &mut ctrl_c => {
                validate_signal_result(result, "SIGINT")?;
                tracing::info!("Interrupt signal received, cleaning up...");
                shutdown_token.cancel();
                break;
            }

            result = &mut termination_signal => {
                result.context("Failed to listen for SIGTERM")?;
                tracing::info!("Termination signal received, cleaning up...");
                shutdown_token.cancel();
                break;
            }

            () = shutdown_token.cancelled() => {
                tracing::info!("Shutdown triggered by health monitor, cleaning up...");
                break;
            }

            result = listener.accept() => {
                let (stream, _addr) = match result {
                    Ok(conn) => conn,
                    Err(e) => {
                        // Classify the error: transient errors should not kill the server
                        let raw_os_error = e.raw_os_error();
                        match e.kind() {
                            // Transient: client disconnected before accept completed, or signal
                            std::io::ErrorKind::ConnectionAborted
                            | std::io::ErrorKind::Interrupted => {
                                tracing::warn!("Transient accept error (continuing): {e}");
                                continue;
                            }
                            _ if raw_os_error == Some(24) || raw_os_error == Some(23) => {
                                // EMFILE (24) = per-process fd limit, ENFILE (23) = system-wide
                                tracing::error!(
                                    "File descriptor limit reached (errno {}), backing off: {e}",
                                    raw_os_error.unwrap_or(0)
                                );
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                continue;
                            }
                            _ => {
                                // Truly fatal: propagate to shut down the server
                                return Err(e.into());
                            }
                        }
                    }
                };
                let state = Arc::clone(&state);
                let client_token = shutdown_token.child_token();

                // Acquire before spawning so the number of live connection
                // tasks stays bounded; on saturation, refuse the connection
                // (dropping the stream closes the socket).
                let Ok(permit) = Arc::clone(&connection_permits).try_acquire_owned() else {
                    tracing::warn!(
                        "Connection limit of {MAX_CONCURRENT_CONNECTIONS} reached; refusing new client"
                    );
                    continue;
                };

                tokio::spawn(async move {
                    // Held until the task completes; Drop releases the permit.
                    let _permit = permit;
                    tokio::select! {
                        // biased: check cancellation first for prompt client shutdown
                        biased;

                        () = client_token.cancelled() => {
                            tracing::debug!("Client connection closed due to shutdown");
                        }
                        result = handle_client(stream, state) => {
                            if let Err(e) = result {
                                tracing::error!("Client error: {}", e);
                            }
                        }
                    }
                });
            }
        }
    }

    daemon_shutdown_result(internal_failure)
}

/// Maximum request size to prevent `DoS` attacks. This also bounds the sole
/// `String` in every `Request` variant: bitcode consumes its bytes directly
/// from the frame before copying them into the decoded value.
/// <https://github.com/SoftbearStudios/bitcode/blob/f41da053c08178189aaee8c62f4c6e738add6eda/src/str.rs>
const MAX_REQUEST_SIZE: usize = 1024 * 1024;

/// Maximum encoded response size (8 MiB). Deliberately distinct from
/// [`MAX_REQUEST_SIZE`] (1 MiB): the two directions carry very different
/// payloads. Requests are small queries, so 1 MiB is a generous cap that
/// also bounds the DoS surface of request parsing; responses may carry up
/// to the daemon's 1000-entry search/audit limits with full descriptions
/// (broad `DebianSearch` or `SecurityAudit` results), which realistically
/// exceeds 1 MiB on large installs.
///
/// The budget sits strictly below the transport ceiling
/// `protocol::MAX_FRAME_SIZE` (10 MiB) that every client-side reader accepts
/// (`read_frame` and the client `Framed` codec). The daemon write codec uses
/// this same budget so an encoded frame can actually leave the socket;
/// inbound frames stay capped at [`MAX_REQUEST_SIZE`]. Responses that overflow
/// it are degraded gracefully by [`encode_bounded_response`] (semantic
/// truncation of list results), never mapped to `INTERNAL_ERROR`.
const MAX_RESPONSE_SIZE: usize = 8 * 1024 * 1024;

// Guard against a future edit raising the response budget past what the
// transport framing and every client-side reader will accept.
const _: () = assert!(MAX_RESPONSE_SIZE < crate::daemon::protocol::MAX_FRAME_SIZE);

/// RAII guard for tracking active connections
struct ConnectionGuard;

impl ConnectionGuard {
    fn new() -> Self {
        GLOBAL_METRICS.inc_active_connections();
        Self
    }
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        GLOBAL_METRICS.dec_active_connections();
    }
}

async fn await_client_write(
    write: impl std::future::Future<Output = std::io::Result<()>>,
    timeout: Duration,
) -> Result<()> {
    tokio::time::timeout(timeout, write)
        .await
        .map_err(|_| anyhow::anyhow!("Daemon client write timed out after {timeout:?}"))??;
    Ok(())
}

/// Halve a list-bearing response result so an oversized response can be
/// degraded gracefully instead of failing.
///
/// Bitcode frames cannot be byte-truncated (the client's decode would fail),
/// so truncation happens at the semantic level: list payloads are cut to a
/// prefix and re-encoded into a still-valid [`Response`] the client decodes
/// normally. Count fields are kept consistent with the delivered entries.
/// Returns `None` when nothing can shrink further (single-entry lists, or
/// results with no list at all).
fn shrink_result(result: &ResponseResult) -> Option<(ResponseResult, usize)> {
    let halve = |len: usize| len / 2;
    let truncatable = |len: usize| len > 1;
    match result {
        ResponseResult::Search(r) if truncatable(r.packages.len()) => {
            let keep = halve(r.packages.len());
            let packages = r.packages[..keep].to_vec();
            let dropped = r.packages.len() - keep;
            // Keep `total` consistent with the delivered prefix so the client
            // sees a complete (smaller) result set instead of one that hints
            // at more matches it can never receive.
            Some((
                ResponseResult::Search(SearchResult {
                    packages,
                    total: keep,
                }),
                dropped,
            ))
        }
        ResponseResult::DebianSearch(v) if truncatable(v.len()) => {
            let keep = halve(v.len());
            let dropped = v.len() - keep;
            Some((ResponseResult::DebianSearch(v[..keep].to_vec()), dropped))
        }
        ResponseResult::Explicit(r) if truncatable(r.packages.len()) => {
            let keep = halve(r.packages.len());
            let dropped = r.packages.len() - keep;
            Some((
                ResponseResult::Explicit(ExplicitResult {
                    packages: r.packages[..keep].to_vec(),
                }),
                dropped,
            ))
        }
        ResponseResult::ListUpdates(v) if truncatable(v.len()) => {
            let keep = halve(v.len());
            let dropped = v.len() - keep;
            Some((ResponseResult::ListUpdates(v[..keep].to_vec()), dropped))
        }
        ResponseResult::Suggest(v) if truncatable(v.len()) => {
            let keep = halve(v.len());
            let dropped = v.len() - keep;
            Some((ResponseResult::Suggest(v[..keep].to_vec()), dropped))
        }
        ResponseResult::SecurityAudit(r) if truncatable(r.vulnerabilities.len()) => {
            let keep = halve(r.vulnerabilities.len());
            let dropped = r.vulnerabilities.len() - keep;
            let kept = &r.vulnerabilities[..keep];
            let total_vulnerabilities: usize = kept.iter().map(|(_, v)| v.len()).sum();
            let high_severity = kept
                .iter()
                .flat_map(|(_, vulns)| vulns)
                .filter(|v| {
                    v.score
                        .as_deref()
                        .and_then(super::handlers::vulnerability_score)
                        .is_some_and(|score| score >= 7.0)
                })
                .count();
            Some((
                ResponseResult::SecurityAudit(SecurityAuditResult {
                    total_vulnerabilities,
                    high_severity,
                    vulnerabilities: kept.to_vec(),
                }),
                dropped,
            ))
        }
        _ => None,
    }
}

fn encode_bounded_response(response: &Response, request_id: u64) -> Result<Vec<u8>> {
    let response_bytes = crate::daemon::protocol::encode_frame(response)?;
    if response_bytes.len() <= MAX_RESPONSE_SIZE {
        return Ok(response_bytes);
    }

    // Graceful degradation: shrink list-bearing results until the encoded
    // frame fits the budget. Halving keeps the loop logarithmic; the frame
    // stays valid `Response` wire data the client decodes normally.
    if let Response::Success { result, .. } = response {
        let mut result = result.clone();
        let mut dropped = 0usize;
        while let Some((shrunk, step)) = shrink_result(&result) {
            result = shrunk;
            dropped += step;
            let response_bytes = crate::daemon::protocol::encode_frame(&Response::Success {
                id: request_id,
                result: result.clone(),
            })?;
            if response_bytes.len() <= MAX_RESPONSE_SIZE {
                tracing::warn!(
                    request_id,
                    dropped,
                    max_response_bytes = MAX_RESPONSE_SIZE,
                    "Daemon response exceeded the response budget; delivered truncated results"
                );
                return Ok(response_bytes);
            }
        }
    }

    // Nothing left to truncate (or an untruncatable result type): a single
    // entry alone exceeds the budget. Degrade to a dedicated limit error —
    // still valid `Response` semantics the client understands — never
    // INTERNAL_ERROR, which would misreport a size limit as a daemon bug.
    tracing::error!(
        request_id,
        response_bytes = response_bytes.len(),
        max_response_bytes = MAX_RESPONSE_SIZE,
        "Daemon response exceeded the response budget and could not be truncated"
    );
    GLOBAL_METRICS.inc_requests_failed();
    Ok(crate::daemon::protocol::encode_frame(&Response::Error {
        id: request_id,
        code: error_codes::RESPONSE_TOO_LARGE,
        message: format!(
            "Daemon response of {} bytes exceeds the {MAX_RESPONSE_SIZE}-byte response budget",
            response_bytes.len()
        ),
    })?)
}

async fn send_response_frame<W>(
    framed: &mut FramedWrite<W, LengthDelimitedCodec>,
    response_bytes: Vec<u8>,
) -> Result<()>
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let response_len = response_bytes.len();
    if let Err(error) =
        await_client_write(framed.send(response_bytes.into()), CLIENT_WRITE_TIMEOUT).await
    {
        GLOBAL_METRICS.inc_requests_failed();
        return Err(error);
    }
    GLOBAL_METRICS.add_bytes_sent(response_len as u64);
    Ok(())
}

/// Handle a single client connection
/// Encode and send one error frame, accounting bytes sent. The shared tail
/// copies is how a future edit forgets the metric.
async fn send_error_response<W>(
    framed: &mut FramedWrite<W, LengthDelimitedCodec>,
    id: u64,
    code: i32,
    message: String,
) where
    W: tokio::io::AsyncWrite + Unpin,
{
    let response = Response::Error { id, code, message };
    if let Ok(response_bytes) = crate::daemon::protocol::encode_frame(&response)
        && let Err(error) = send_response_frame(framed, response_bytes).await
    {
        tracing::warn!("Failed to send daemon error response: {error}");
    }
}

async fn handle_client(stream: tokio::net::UnixStream, state: Arc<DaemonState>) -> Result<()> {
    handle_client_with_idle_timeout(stream, state, CLIENT_IDLE_TIMEOUT).await
}

async fn handle_client_with_idle_timeout(
    stream: tokio::net::UnixStream,
    state: Arc<DaemonState>,
    idle_timeout: Duration,
) -> Result<()> {
    // METRICS: Track active connections using RAII guard
    let _guard = ConnectionGuard::new();

    // LengthDelimitedCodec applies max_frame_length to encode and decode.
    // Keep inbound frames at the request DoS cap; give the write half the
    // response budget so an 8 MiB success frame is not rejected after encode.
    let (read_half, write_half) = tokio::io::split(stream);
    let mut framed_read = LengthDelimitedCodec::builder()
        .max_frame_length(MAX_REQUEST_SIZE)
        .new_read(read_half);
    let mut framed_write = LengthDelimitedCodec::builder()
        .max_frame_length(MAX_RESPONSE_SIZE)
        .new_write(write_half);

    // Rate limit per connection to ensure fairness
    // Const-eval guarantee: the literals above are non-zero; a violation is
    // rejected at compile time, not at runtime.
    const RATE_LIMIT_NZ: NonZeroU32 = NonZeroU32::new(CLIENT_RATE_LIMIT_HZ).unwrap();
    const BURST_SIZE_NZ: NonZeroU32 = NonZeroU32::new(CLIENT_BURST_SIZE).unwrap();
    let quota = Quota::per_second(RATE_LIMIT_NZ).allow_burst(BURST_SIZE_NZ);
    let rate_limiter = RateLimiter::direct(quota);

    tracing::debug!("New binary client connected");

    loop {
        let request_bytes = match tokio::time::timeout(idle_timeout, framed_read.next()).await {
            Ok(Some(request_bytes)) => request_bytes,
            Ok(None) => break,
            Err(_) => {
                tracing::debug!(?idle_timeout, "Closing idle daemon client");
                break;
            }
        };
        // Transport/frame errors (oversize frame, I/O) tear down the
        // connection: the length-prefix stream may be desynchronized, so
        // answering on it would be unsafe.
        let bytes = match request_bytes {
            Ok(bytes) => bytes,
            Err(error) => {
                tracing::warn!("Frame decode failed; closing client connection: {error}");
                GLOBAL_METRICS.inc_requests_failed();
                break;
            }
        };

        // METRICS: Track bytes received
        GLOBAL_METRICS.add_bytes_received(bytes.len() as u64);

        // SECURITY: A malformed payload means we cannot trust anything further
        // from this client. Answer once with the protocol's parse-error code,
        // then close cleanly instead of tearing down silently. Request id 0 is
        // the reserved error-envelope id for requests that never decoded.
        // Reject peers speaking a different protocol version before any
        // decode attempt could silently mis-map same-shaped variants.
        let payload = match crate::daemon::protocol::split_frame(&bytes) {
            Ok((_, payload)) => payload,
            Err(crate::daemon::protocol::FrameError::VersionMismatch { peer, ours }) => {
                tracing::warn!(
                    peer,
                    ours,
                    "rejecting client with mismatched protocol version"
                );
                // Answer once so the client learns WHY instead of hanging for
                // its full timeout, then close — the stream is unusable.
                send_error_response(
                    &mut framed_write,
                    0,
                    error_codes::PARSE_ERROR,
                    format!(
                        "unsupported peer protocol version {peer} (this daemon speaks {ours}); update omg"
                    ),
                )
                .await;
                GLOBAL_METRICS.inc_requests_failed();
                break;
            }
            Err(e) => {
                tracing::warn!("malformed frame header: {e}");
                send_error_response(
                    &mut framed_write,
                    0,
                    error_codes::PARSE_ERROR,
                    format!("malformed frame header: {e}"),
                )
                .await;
                GLOBAL_METRICS.inc_requests_failed();
                break;
            }
        };
        let request: Request = match bitcode::deserialize(payload) {
            Ok(request) => request,
            Err(error) => {
                let msg = format!("Failed to deserialize request: {error}");
                tracing::warn!("{msg}");
                audit_log_nonblocking(
                    AuditEventType::PolicyViolation,
                    AuditSeverity::Warning,
                    "daemon_server",
                    &msg,
                );
                GLOBAL_METRICS.inc_validation_failures();
                GLOBAL_METRICS.inc_requests_failed();
                send_error_response(&mut framed_write, 0, error_codes::PARSE_ERROR, msg).await;
                break;
            }
        };

        let request_id = request.id();

        // SECURITY: Enforce per-connection rate limiting
        if rate_limiter.check().is_err() {
            tracing::warn!("Client rate limit exceeded for request {}", request_id);
            audit_log_nonblocking(
                AuditEventType::PolicyViolation,
                AuditSeverity::Warning,
                "daemon_server",
                "Client rate limit exceeded",
            );
            GLOBAL_METRICS.inc_rate_limit_hits();
            GLOBAL_METRICS.inc_requests_failed();

            send_error_response(
                &mut framed_write,
                request_id,
                error_codes::RATE_LIMITED,
                "Rate limit exceeded. Please slow down.".to_string(),
            )
            .await;
            continue;
        }

        // Handle request with timeout to prevent hung clients
        let response =
            tokio::time::timeout(REQUEST_TIMEOUT, handle_request(Arc::clone(&state), request))
                .await
                .unwrap_or_else(|_| {
                    tracing::warn!(
                        "Request {} timed out after {:?}",
                        request_id,
                        REQUEST_TIMEOUT
                    );
                    GLOBAL_METRICS.inc_requests_failed();
                    Response::Error {
                        id: request_id,
                        code: error_codes::INTERNAL_ERROR,
                        message: format!(
                            "Request timed out after {} seconds",
                            REQUEST_TIMEOUT.as_secs()
                        ),
                    }
                });

        // Encode and send response
        let response_bytes = encode_bounded_response(&response, request_id)?;
        send_response_frame(&mut framed_write, response_bytes).await?;
    }

    tracing::debug!("Client disconnected");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::protocol::{PackageInfo, WirePackageSource};
    use super::*;

    #[test]
    fn untruncatable_oversized_responses_use_a_dedicated_limit_error() {
        // Message results carry no truncatable list: a single payload alone
        // exceeds the budget, so the response degrades to the dedicated
        // limit error — never INTERNAL_ERROR.
        let oversized = Response::Success {
            id: 77,
            result: ResponseResult::Message("x".repeat(MAX_RESPONSE_SIZE + 1024)),
        };

        let encoded = encode_bounded_response(&oversized, 77).expect("bounded response");
        assert!(encoded.len() <= MAX_RESPONSE_SIZE);
        let (_, payload) = crate::daemon::protocol::split_frame(&encoded).expect("frame header");
        let decoded: Response = bitcode::deserialize(payload).expect("response payload");
        match decoded {
            Response::Error { id, code, message } => {
                assert_eq!(id, 77);
                assert_eq!(code, error_codes::RESPONSE_TOO_LARGE);
                assert!(message.contains("response budget"));
            }
            Response::Success { .. } => {
                panic!("untruncatable oversized response must become an error")
            }
        }
    }

    #[test]
    fn oversized_debian_search_results_are_truncated_not_errored() {
        let entry = PackageInfo {
            name: "pkg".to_string(),
            version: "1.0".to_string(),
            description: "d".repeat(16_000),
            source: WirePackageSource::Official,
        };
        // ~600 entries x ~16 KiB ≈ 9.6 MB encoded: over the 8 MiB budget.
        let count = 600;
        let oversized = Response::Success {
            id: 42,
            result: ResponseResult::DebianSearch(vec![entry; count]),
        };
        let raw = crate::daemon::protocol::encode_frame(&oversized).expect("encode oversized");
        assert!(
            raw.len() > MAX_RESPONSE_SIZE,
            "test setup: payload must exceed the budget"
        );

        let encoded = encode_bounded_response(&oversized, 42).expect("bounded response");
        assert!(encoded.len() <= MAX_RESPONSE_SIZE);
        let (_, payload) = crate::daemon::protocol::split_frame(&encoded).expect("frame header");
        let decoded: Response = bitcode::deserialize(payload).expect("truncated frame decodes");
        match decoded {
            Response::Success {
                id,
                result: ResponseResult::DebianSearch(list),
            } => {
                assert_eq!(id, 42);
                assert!(list.len() < count, "list must have been truncated");
                assert!(!list.is_empty(), "truncation must keep at least one entry");
            }
            other => panic!("oversized list response must stay a success, got {other:?}"),
        }
    }

    #[test]
    fn oversized_search_results_are_truncated_with_consistent_total() {
        let entry = PackageInfo {
            name: "pkg".to_string(),
            version: "1.0".to_string(),
            description: "d".repeat(16_000),
            source: WirePackageSource::Official,
        };
        let count = 600;
        let oversized = Response::Success {
            id: 43,
            result: ResponseResult::Search(SearchResult {
                packages: vec![entry; count],
                total: count,
            }),
        };

        let encoded = encode_bounded_response(&oversized, 43).expect("bounded response");
        assert!(encoded.len() <= MAX_RESPONSE_SIZE);
        let (_, payload) = crate::daemon::protocol::split_frame(&encoded).expect("frame header");
        let decoded: Response = bitcode::deserialize(payload).expect("truncated frame decodes");
        match decoded {
            Response::Success {
                id,
                result: ResponseResult::Search(results),
            } => {
                assert_eq!(id, 43);
                assert!(results.packages.len() < count);
                assert!(!results.packages.is_empty());
                // The client must see a complete, internally consistent
                // result set, not one hinting at more matches it can never
                // receive.
                assert_eq!(results.total, results.packages.len());
            }
            other => panic!("oversized search response must stay a success, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn response_write_codec_accepts_frames_above_the_request_budget() {
        let (mut client, server) = tokio::net::UnixStream::pair().expect("socket pair");
        let payload_len = MAX_REQUEST_SIZE + 1;
        let writer = tokio::spawn(async move {
            let mut framed_write = LengthDelimitedCodec::builder()
                .max_frame_length(MAX_RESPONSE_SIZE)
                .new_write(server);
            framed_write.send(vec![0u8; payload_len].into()).await
        });
        // Drain so the write cannot stall on a full socket buffer.
        let mut remaining = 4 + payload_len;
        let mut buf = vec![0u8; 64 * 1024];
        while remaining > 0 {
            let n = tokio::io::AsyncReadExt::read(&mut client, &mut buf)
                .await
                .expect("drain response frame");
            assert!(n > 0, "writer closed before the frame was fully sent");
            remaining = remaining.saturating_sub(n);
        }
        writer
            .await
            .expect("write task")
            .expect("write codec must accept a frame larger than the request budget");
    }

    #[tokio::test]
    async fn request_sized_codec_cannot_send_the_response_budget() {
        let (_client, server) = tokio::net::UnixStream::pair().expect("socket pair");
        let mut framed_write = LengthDelimitedCodec::builder()
            .max_frame_length(MAX_REQUEST_SIZE)
            .new_write(server);
        framed_write
            .send(vec![0u8; MAX_REQUEST_SIZE + 1].into())
            .await
            .expect_err("a 1 MiB write cap would reject the new response budget");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn fast_status_writes_run_through_the_blocking_adapter() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("status.bin");
        let status = crate::core::fast_status::FastStatus::new(10, 4, 1, 2);

        write_fast_status_async(status, path.clone())
            .await
            .expect("write fast status");

        let persisted =
            crate::core::fast_status::FastStatus::read_from_file(&path).expect("read fast status");
        assert_eq!(persisted.total_packages, 10);
        assert_eq!(persisted.updates_available, 2);
    }

    #[test]
    fn signal_listener_failures_are_not_treated_as_shutdown_signals() {
        validate_signal_result(Ok(()), "SIGINT").expect("received signal");
        let error = validate_signal_result(
            Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "signals unavailable",
            )),
            "SIGINT",
        )
        .expect_err("listener registration failure must propagate");
        assert!(error.to_string().contains("Failed to listen for SIGINT"));
    }

    #[test]
    fn internal_daemon_failures_produce_unsuccessful_exit_results() {
        assert!(daemon_shutdown_result(None).is_ok());
        let error = daemon_shutdown_result(Some("worker crashed".to_string()))
            .expect_err("internal failure must make daemon exit unsuccessfully");
        assert_eq!(error.to_string(), "worker crashed");
    }

    #[tokio::test]
    async fn daemon_client_writes_have_a_deadline() {
        let stalled = std::future::pending::<std::io::Result<()>>();
        let error = await_client_write(stalled, Duration::from_millis(1))
            .await
            .expect_err("stalled client write must time out");
        assert!(error.to_string().contains("write timed out"));

        await_client_write(std::future::ready(Ok(())), Duration::from_secs(1))
            .await
            .expect("completed write must succeed");
    }

    #[test]
    fn fast_status_reader_ttl_matches_daemon_writer_cadence() {
        // If the reader TTL is shorter than the writer interval, the
        // zero-IPC fast path rejects every file between daemon refreshes.
        assert_eq!(
            STATUS_REFRESH_INTERVAL.as_secs(),
            crate::core::fast_status::FAST_STATUS_FRESHNESS_SECS,
            "FastStatus TTL must equal the daemon writer interval"
        );
    }

    #[tokio::test]
    async fn idle_client_is_closed_and_releases_its_connection_metric() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let data_dir = directory.path().join("data");
        std::fs::create_dir_all(&data_dir)?;
        let state = Arc::new(super::super::handlers::DaemonState::new_isolated(
            &data_dir,
            super::super::index::PackageIndex::empty(),
            Arc::new(crate::package_managers::mock::MockPackageManager::new_in(
                "arch", &data_dir,
            )),
        )?);
        let baseline = GLOBAL_METRICS.snapshot().active_connections;
        let (server, _idle_client) = tokio::net::UnixStream::pair()?;

        let task = tokio::spawn(handle_client_with_idle_timeout(
            server,
            state,
            Duration::from_millis(20),
        ));
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .context("idle client did not time out")???;

        assert_eq!(
            GLOBAL_METRICS.snapshot().active_connections,
            baseline,
            "idle timeout must release the active connection guard"
        );
        Ok(())
    }

    #[tokio::test]
    async fn startup_prewarms_every_common_search_query() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let data_dir = directory.path().join("data");
        std::fs::create_dir_all(&data_dir)?;
        let state = Arc::new(super::super::handlers::DaemonState::new_isolated(
            &data_dir,
            super::super::index::PackageIndex::empty(),
            Arc::new(crate::package_managers::mock::MockPackageManager::new_in(
                "arch", &data_dir,
            )),
        )?);
        let socket_path = directory.path().join("prewarm.sock");
        let fast_status_path = directory.path().join("omg.status");
        let listener = UnixListener::bind(&socket_path)?;
        let server = tokio::spawn(run_with_status_path(
            listener,
            Arc::clone(&state),
            socket_path,
            fast_status_path.clone(),
        ));

        let queries = ["", "linux", "python", "node", "firefox", "git"];
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            if fast_status_path.is_file()
                && queries.iter().all(|query| state.cache.get(query).is_some())
            {
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                server.abort();
                anyhow::bail!("startup did not publish fast status and prewarm all common queries");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }

        server.abort();
        Ok(())
    }
}
