//! Daemon server implementation with Unix socket IPC
//!
//! Uses `LengthDelimitedCodec` and bitcode for maximum IPC performance.

use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use governor::{Quota, RateLimiter};
use tokio::net::UnixListener;
use tokio_util::codec::LengthDelimitedCodec;
use tokio_util::sync::CancellationToken;

use super::handlers::{DaemonState, handle_request};
use super::protocol::{Request, Response, error_codes};
use crate::core::metrics::GLOBAL_METRICS;
use crate::core::security::{AuditEventType, AuditSeverity, audit_log};

/// Request handling timeout (30 seconds should be sufficient for most operations)
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Status refresh interval (5 minutes)
const STATUS_REFRESH_INTERVAL: Duration = Duration::from_mins(5);

/// Memory cleanup interval (30 minutes) - matches mmap TTL
const MEMORY_CLEANUP_INTERVAL: Duration = Duration::from_mins(30);

/// Socket health check interval (60 seconds) - detect deleted socket files
const SOCKET_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(60);

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
        signal.recv().await;
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
    let shutdown_token = CancellationToken::new();

    // Budget for concurrent client connections; permits are released when a
    // connection task ends.
    let connection_permits = Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CONNECTIONS));

    // START BACKGROUND WORKER
    let state_worker = Arc::clone(&state);
    let worker_token = shutdown_token.child_token();
    // Clone the parent token so the health check can trigger a full shutdown
    let shutdown_trigger = shutdown_token.clone();

    let worker_handle = tokio::spawn(async move {
        tracing::info!("Background status worker started");

        async fn refresh_status(state: &Arc<DaemonState>) {
            let pm_name = state.package_manager.name().to_string();
            let result = tokio::task::spawn_blocking(move || {
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

                (
                    versions,
                    super::handlers::system_status_for_backend(&pm_name),
                )
            })
            .await;

            let (versions, status) = match result {
                Ok(result) => result,
                Err(error) => {
                    tracing::error!("Status refresh panic: {error}");
                    return;
                }
            };
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
            if let Err(error) = fast_status.write_default() {
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
            // Pre-compute explicit package list for instant first query
            let pm_name = state.package_manager.name().to_string();
            let explicit = tokio::task::spawn_blocking(move || {
                super::handlers::explicit_packages_for_backend(&pm_name)
            })
            .await;
            match explicit {
                Ok(Ok(packages)) => {
                    state.cache.update_explicit(packages);
                    tracing::debug!("Pre-warmed explicit package cache");
                }
                Ok(Err(error)) => {
                    tracing::warn!("Failed to pre-warm explicit package cache: {error}");
                }
                Err(error) => {
                    tracing::warn!("Explicit package cache pre-warm task failed: {error}");
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
        refresh_status(&state_worker).await;
        prewarm_caches(&state_worker).await;

        // Track last cleanup time for periodic mmap cleanup
        let mut last_cleanup = std::time::Instant::now();
        let mut last_socket_check = std::time::Instant::now();

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
                    refresh_status(&state_worker).await;
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

                    // Socket health check: verify socket file still exists
                    if last_socket_check.elapsed() >= SOCKET_HEALTH_CHECK_INTERVAL {
                        if !socket_path.exists() {
                            tracing::error!(
                                "Socket file {} has been removed externally! Initiating shutdown.",
                                socket_path.display()
                            );
                            // Cancel the parent shutdown token to stop the accept loop
                            shutdown_trigger.cancel();
                            break;
                        }
                        last_socket_check = std::time::Instant::now();
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
        tokio::spawn(async move {
            if worker_handle.await.is_err() {
                state_monitor.inc_background_worker_failures();
                tracing::error!(
                    "Background status worker terminated unexpectedly; initiating shutdown"
                );
                shutdown_monitor.cancel();
            }
        });
    }

    tracing::info!("Daemon ready, binary IPC enabled");

    loop {
        tokio::select! {
            // biased: always check shutdown signal first to avoid accepting
            // new connections after shutdown was requested
            biased;

            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Interrupt signal received, cleaning up...");
                shutdown_token.cancel();
                break;
            }

            result = wait_for_termination_signal() => {
                result?;
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

    Ok(())
}

/// Maximum request size to prevent `DoS` attacks. This also bounds the sole
/// `String` in every `Request` variant: bitcode consumes its bytes directly
/// from the frame before copying them into the decoded value.
/// <https://github.com/SoftbearStudios/bitcode/blob/f41da053c08178189aaee8c62f4c6e738add6eda/src/str.rs>
const MAX_REQUEST_SIZE: usize = 1024 * 1024;

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

/// Handle a single client connection
/// Encode and send one error frame, accounting bytes sent. The shared tail
/// of every early-reject path in `handle_client` (audit typ01 C2): without
/// it, drift between copies is how a future edit forgets the metric.
async fn send_error_response(
    framed: &mut tokio_util::codec::Framed<tokio::net::UnixStream, LengthDelimitedCodec>,
    id: u64,
    code: i32,
    message: String,
) {
    let response = Response::Error { id, code, message };
    if let Ok(response_bytes) = crate::daemon::protocol::encode_frame(&response) {
        GLOBAL_METRICS.add_bytes_sent(response_bytes.len() as u64);
        let _ = framed.send(response_bytes.into()).await;
    }
}

async fn handle_client(stream: tokio::net::UnixStream, state: Arc<DaemonState>) -> Result<()> {
    // METRICS: Track active connections using RAII guard
    let _guard = ConnectionGuard::new();

    // Use length-delimited framing for binary messages with max frame length
    let mut framed = LengthDelimitedCodec::builder()
        .max_frame_length(MAX_REQUEST_SIZE)
        .new_framed(stream);

    // Rate limit per connection to ensure fairness
    // Const-eval guarantee: the literals above are non-zero; a violation is
    // rejected at compile time, not at runtime.
    const RATE_LIMIT_NZ: NonZeroU32 = NonZeroU32::new(CLIENT_RATE_LIMIT_HZ).unwrap();
    const BURST_SIZE_NZ: NonZeroU32 = NonZeroU32::new(CLIENT_BURST_SIZE).unwrap();
    let quota = Quota::per_second(RATE_LIMIT_NZ).allow_burst(BURST_SIZE_NZ);
    let rate_limiter = RateLimiter::direct(quota);

    tracing::debug!("New binary client connected");

    while let Some(request_bytes) = framed.next().await {
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
                    &mut framed,
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
                    &mut framed,
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
                audit_log(
                    AuditEventType::PolicyViolation,
                    AuditSeverity::Warning,
                    "daemon_server",
                    &msg,
                );
                GLOBAL_METRICS.inc_validation_failures();
                GLOBAL_METRICS.inc_requests_failed();
                send_error_response(&mut framed, 0, error_codes::PARSE_ERROR, msg).await;
                break;
            }
        };

        let request_id = request.id();

        // SECURITY: Enforce per-connection rate limiting
        if rate_limiter.check().is_err() {
            tracing::warn!("Client rate limit exceeded for request {}", request_id);
            audit_log(
                AuditEventType::PolicyViolation,
                AuditSeverity::Warning,
                "daemon_server",
                "Client rate limit exceeded",
            );
            GLOBAL_METRICS.inc_rate_limit_hits();
            GLOBAL_METRICS.inc_requests_failed();

            send_error_response(
                &mut framed,
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
        let response_bytes = crate::daemon::protocol::encode_frame(&response)?;
        GLOBAL_METRICS.add_bytes_sent(response_bytes.len() as u64);
        framed.send(response_bytes.into()).await?;
    }

    tracing::debug!("Client disconnected");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::STATUS_REFRESH_INTERVAL;

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
}
