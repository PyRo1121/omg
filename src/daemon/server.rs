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
    let socket_path_worker = socket_path;
    // Clone the parent token so the health check can trigger a full shutdown
    let shutdown_trigger = shutdown_token.clone();

    let worker_handle = tokio::spawn(async move {
        tracing::info!("Background status worker started");

        // OPTIMIZATION: Deduplicate status fetching logic into a helper function
        async fn refresh_status(state: &Arc<DaemonState>) {
            let pm_name = state.package_manager.name().to_string();
            // Offload heavy I/O and CPU work to a blocking thread
            let result = tokio::task::spawn_blocking(move || {
                use crate::cli::runtimes::{ensure_active_version, known_runtimes};

                // 1. Probe Runtimes (Fast but sync I/O)
                let mut versions = Vec::new();
                match known_runtimes() {
                    Ok(runtimes) => {
                        for runtime in runtimes {
                            match ensure_active_version(&runtime) {
                                Ok(Some(v)) => versions.push((runtime, v)),
                                Ok(None) => {}
                                Err(error) => {
                                    tracing::warn!(
                                        "Failed to resolve active {runtime} version: {error}"
                                    );
                                }
                            }
                        }
                    }
                    Err(error) => {
                        tracing::warn!("Failed to list known runtimes: {error}");
                    }
                }

                // 2. Refresh Package Status (Heavy sync I/O)
                let status_result = super::handlers::system_status_for_backend(&pm_name);

                (versions, status_result)
            })
            .await;

            if let Ok((versions, status)) = result {
                // Update runtime versions safely
                state
                    .runtime_versions
                    .write()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone_from(&versions);

                if let Ok((total, explicit, orphans, updates)) = status {
                    // Write fast status file for zero-IPC CLI reads
                    let fast_status = crate::core::fast_status::FastStatus::new(
                        total, explicit, orphans, updates,
                    );
                    if let Err(e) = fast_status.write_default() {
                        tracing::warn!("Failed to write fast status file: {e}");
                    }

                    // 3. Scan for Vulnerabilities (async, done in background)
                    // This is already async, so we run it here in the async context
                    let scanner = crate::core::security::VulnerabilityScanner::new();
                    let previous_vulns = state
                        .cache
                        .get_status()
                        .and_then(|status| status.scanned_vulnerability_count());
                    let scan = scanner.scan_system().await;
                    if let Err(error) = &scan {
                        tracing::warn!("Vulnerability scan failed during status refresh: {error}");
                    }
                    // Publication only: a failed scan with no prior count
                    // skips status publication; cache pre-warming below runs
                    // unconditionally either way.
                    if let Some(vuln_count) =
                        super::status_policy::vulnerability_count_from_scan(&scan, previous_vulns)
                    {
                        let res = super::protocol::StatusResult {
                            total_packages: total,
                            explicit_packages: explicit,
                            orphan_packages: orphans,
                            updates_available: updates,
                            security_vulnerabilities: vuln_count,
                            vulnerabilities_scanned: true,
                            runtime_versions: versions,
                        };
                        let res_arc = Arc::new(res);
                        // Persist for faster restarts; the in-memory cache is
                        // authoritative for this process, so a persistence failure
                        // is logged, not fatal.
                        if let Err(error) = state.persistent.set_status(&res_arc) {
                            tracing::warn!("Failed to persist status cache: {error}");
                        }
                        state.cache.update_status(res_arc);
                    } else {
                        tracing::warn!(
                            "No prior vulnerability count; not publishing a zero-vuln status"
                        );
                    }
                } else if let Err(error) = status {
                    tracing::warn!("Failed to refresh package status: {error}");
                }
            } else if let Err(e) = result {
                tracing::error!("Status refresh panic: {e}");
            }
        }

        /// Pre-compute caches for instant first queries.
        ///
        /// Deliberately unconditional: it must run even when status
        /// publication was skipped above (structural guarantee against the
        /// early-return regression this replaces).
        async fn prewarm_caches(state: &Arc<DaemonState>) {
            // Pre-compute explicit package list for instant first query
            let pm_name = state.package_manager.name().to_string();
            let state_explicit = Arc::clone(state);
            if let Err(error) = tokio::task::spawn_blocking(move || {
                match super::handlers::explicit_packages_for_backend(&pm_name) {
                    Ok(explicit_pkgs) => {
                        state_explicit.cache.update_explicit(explicit_pkgs);
                        tracing::debug!("Pre-warmed explicit package cache");
                    }
                    Err(error) => {
                        tracing::warn!("Failed to pre-warm explicit package cache: {error}");
                    }
                }
            })
            .await
            {
                tracing::warn!("Explicit package cache pre-warm task failed: {error}");
            }

            // Pre-warm search cache with common queries for instant first searches
            let state_search = Arc::clone(state);
            if let Err(error) = tokio::task::spawn_blocking(move || {
                let common_queries = ["", "linux", "python", "node", "firefox", "git"];
                for query in common_queries {
                    let results = state_search.index.search(query, 50);
                    let results_arc = Arc::new(results);
                    state_search
                        .cache
                        .insert_arc(query.to_string(), results_arc);
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
                        if !socket_path_worker.exists() {
                            tracing::error!(
                                "Socket file {} has been removed externally! Initiating shutdown.",
                                socket_path_worker.display()
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

/// Maximum request size to prevent `DoS` attacks (1MB should be sufficient)
const MAX_REQUEST_SIZE: usize = 1024 * 1024;

/// Maximum deserialized request size to prevent compression bomb attacks (10MB)
const MAX_DESERIALIZED_SIZE: usize = 10 * 1024 * 1024;

/// Upper bound on Batch nesting walked by [`validate_batch_depth`] (and thus
/// by `Request::heap_size`). `handle_batch` rejects any nested Batch outright,
/// so 1 (top-level batch, flat children only) is the honest bound; this guard
/// exists to bound recursion before attacker-controlled payloads are walked.
const MAX_BATCH_DEPTH: usize = 1;

/// Validate batch request depth to prevent recursion `DoS` attacks
fn validate_batch_depth(request: &Request, depth: usize) -> Result<()> {
    if depth > MAX_BATCH_DEPTH {
        return Err(anyhow::anyhow!(
            "Batch nesting depth {depth} exceeds maximum of {MAX_BATCH_DEPTH}"
        ));
    }

    if let Request::Batch { requests, .. } = request {
        for req in requests {
            validate_batch_depth(req, depth + 1)?;
        }
    }

    Ok(())
}

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
        let request: Request = match bitcode::deserialize(&bytes) {
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
                let response = Response::Error {
                    id: 0,
                    code: error_codes::PARSE_ERROR,
                    message: msg,
                };
                match bitcode::serialize(&response) {
                    Ok(response_bytes) => {
                        GLOBAL_METRICS.add_bytes_sent(response_bytes.len() as u64);
                        if let Err(send_error) = framed.send(response_bytes.into()).await {
                            tracing::warn!("Failed to deliver parse-error response: {send_error}");
                        }
                    }
                    Err(serialize_error) => {
                        tracing::error!(
                            "Failed to serialize parse-error response: {serialize_error}"
                        );
                    }
                }
                break;
            }
        };

        // SECURITY: Validate batch nesting depth before walking heap contents;
        // this bounds the recursion in both this check and `heap_size`.
        if let Err(e) = validate_batch_depth(&request, 0) {
            let msg = format!("Invalid batch structure: {e}");
            tracing::warn!("{}", msg);
            audit_log(
                AuditEventType::PolicyViolation,
                AuditSeverity::Warning,
                "daemon_server",
                &msg,
            );
            GLOBAL_METRICS.inc_requests_failed();
            continue;
        }

        // SECURITY: Validate deserialized size to prevent compression bomb attacks.
        // `heap_size` walks `String`/`Vec` payloads; `std::mem::size_of_val`
        // would only measure the enum's stack size and could never fire.
        let estimated_size = request.heap_size();
        if estimated_size > MAX_DESERIALIZED_SIZE {
            let msg = format!(
                "Deserialized request too large: {estimated_size} bytes (max {MAX_DESERIALIZED_SIZE})"
            );
            tracing::warn!("{}", msg);
            audit_log(
                AuditEventType::PolicyViolation,
                AuditSeverity::Warning,
                "daemon_server",
                &msg,
            );
            GLOBAL_METRICS.inc_requests_failed();
            continue;
        }

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

            let response = Response::Error {
                id: request_id,
                code: error_codes::RATE_LIMITED,
                message: "Rate limit exceeded. Please slow down.".to_string(),
            };

            let response_bytes = bitcode::serialize(&response)?;
            GLOBAL_METRICS.add_bytes_sent(response_bytes.len() as u64);
            framed.send(response_bytes.into()).await?;
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
        let response_bytes = bitcode::serialize(&response)?;
        GLOBAL_METRICS.add_bytes_sent(response_bytes.len() as u64);
        framed.send(response_bytes.into()).await?;
    }

    tracing::debug!("Client disconnected");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{MAX_BATCH_DEPTH, validate_batch_depth};
    use crate::daemon::protocol::Request;

    fn flat_batch(id: u64) -> Request {
        Request::Batch {
            id,
            requests: vec![Request::Ping { id }, Request::Status { id }],
        }
    }

    #[test]
    fn batch_depth_guard_accepts_top_level_and_flat_children() {
        assert!(validate_batch_depth(&flat_batch(1), 0).is_ok());
        assert!(validate_batch_depth(&Request::Ping { id: 2 }, 0).is_ok());
    }

    #[test]
    fn batch_depth_guard_rejects_nesting_beyond_the_honest_bound() {
        let nested = Request::Batch {
            id: 1,
            requests: vec![flat_batch(2)],
        };
        assert!(validate_batch_depth(&nested, 0).is_err());
        // Directly past the bound at any depth.
        assert!(validate_batch_depth(&Request::Ping { id: 3 }, MAX_BATCH_DEPTH + 1).is_err());
    }
}
