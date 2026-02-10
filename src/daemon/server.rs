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

#[cfg(feature = "debian")]
use crate::package_managers::apt_get_system_status;

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

/// Run the daemon server
pub async fn run(
    listener: UnixListener,
    state: Arc<DaemonState>,
    socket_path: PathBuf,
) -> Result<()> {
    let shutdown_token = CancellationToken::new();

    // START BACKGROUND WORKER
    let state_worker = Arc::clone(&state);
    let worker_token = shutdown_token.child_token();
    let socket_path_worker = socket_path;
    // Clone the parent token so the health check can trigger a full shutdown
    let shutdown_trigger = shutdown_token.clone();

    tokio::spawn(async move {
        tracing::info!("Background status worker started");

        // OPTIMIZATION: Deduplicate status fetching logic into a helper function
        async fn refresh_status(state: &Arc<DaemonState>) {
            // Offload heavy I/O and CPU work to a blocking thread
            let result = tokio::task::spawn_blocking(move || {
                use crate::cli::runtimes::{ensure_active_version, known_runtimes};
                use crate::core::env::distro::use_debian_backend;

                // 1. Probe Runtimes (Fast but sync I/O)
                let mut versions = Vec::new();
                for runtime in known_runtimes() {
                    if let Some(v) = ensure_active_version(&runtime) {
                        versions.push((runtime, v));
                    }
                }

                // 2. Refresh Package Status (Heavy sync I/O)
                #[cfg(feature = "arch")]
                let status_result = if use_debian_backend() {
                    #[cfg(feature = "debian")]
                    {
                        apt_get_system_status()
                    }
                    #[cfg(not(feature = "debian"))]
                    {
                        Err(anyhow::anyhow!("Debian backend disabled"))
                    }
                } else {
                    use crate::package_managers::get_system_status;
                    get_system_status()
                };

                #[cfg(not(feature = "arch"))]
                let status_result = if use_debian_backend() {
                    #[cfg(feature = "debian")]
                    {
                        apt_get_system_status()
                    }
                    #[cfg(not(feature = "debian"))]
                    {
                        Err(anyhow::anyhow!("No package manager backend available"))
                    }
                } else {
                    Err(anyhow::anyhow!("Arch backend disabled"))
                };

                (versions, status_result)
            })
            .await;

            if let Ok((versions, status)) = result {
                // Update runtime versions safely
                state
                    .runtime_versions
                    .write()
                    .expect("lock poisoned")
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
                    let vuln_count = scanner.scan_system().await.unwrap_or(0);

                    let res = super::protocol::StatusResult {
                        total_packages: total,
                        explicit_packages: explicit,
                        orphan_packages: orphans,
                        updates_available: updates,
                        security_vulnerabilities: vuln_count,
                        runtime_versions: versions,
                    };
                    let res_arc = Arc::new(res);
                    let _ = state.persistent.set_status(&res_arc);
                    state.cache.update_status(res_arc);
                }
            } else if let Err(e) = result {
                tracing::error!("Status refresh panic: {e}");
            }

            // Pre-compute explicit package list for instant first query
            // This is also heavy, so we spawn another blocking task or check if it's fast enough
            #[cfg(feature = "arch")]
            {
                let state_explicit = Arc::clone(state);
                let _ = tokio::task::spawn_blocking(move || {
                    use crate::core::env::distro::use_debian_backend;
                    if !use_debian_backend()
                        && let Ok(explicit_pkgs) = crate::package_managers::list_explicit_fast()
                    {
                        state_explicit.cache.update_explicit(explicit_pkgs);
                        tracing::debug!("Pre-warmed explicit package cache");
                    }
                })
                .await;
            }

            // Pre-warm search cache with common queries for instant first searches
            {
                let state_search = Arc::clone(state);
                tokio::task::spawn_blocking(move || {
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
                .ok();
            }
        }

        // Initial refresh
        refresh_status(&state_worker).await;

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

    tracing::info!("Daemon ready, binary IPC enabled");

    loop {
        tokio::select! {
            // biased: always check shutdown signal first to avoid accepting
            // new connections after shutdown was requested
            biased;

            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutdown signal received, cleaning up...");
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

                tokio::spawn(async move {
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

/// Maximum batch nesting depth to prevent recursion `DoS`
const MAX_BATCH_DEPTH: usize = 3;

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
    // SAFETY: Both constants are known non-zero at compile time
    const RATE_LIMIT_NZ: NonZeroU32 = NonZeroU32::new(CLIENT_RATE_LIMIT_HZ).unwrap();
    const BURST_SIZE_NZ: NonZeroU32 = NonZeroU32::new(CLIENT_BURST_SIZE).unwrap();
    let quota = Quota::per_second(RATE_LIMIT_NZ).allow_burst(BURST_SIZE_NZ);
    let rate_limiter = RateLimiter::direct(quota);

    tracing::debug!("New binary client connected");

    while let Some(request_bytes) = framed.next().await {
        let bytes = request_bytes?;

        // METRICS: Track bytes received
        GLOBAL_METRICS.add_bytes_received(bytes.len() as u64);

        // SECURITY: Validate size before deserialization to prevent memory exhaustion
        if bytes.len() > MAX_REQUEST_SIZE {
            let msg = format!("Request exceeds maximum size: {} bytes", bytes.len());
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

        // Decode request
        let request: Request = bitcode::deserialize(&bytes)?;

        // SECURITY: Validate deserialized size to prevent compression bomb attacks
        let estimated_size = std::mem::size_of_val(&request);
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

        // SECURITY: Validate batch nesting depth to prevent recursion DoS
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
