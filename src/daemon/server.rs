//! Daemon server implementation with Unix socket IPC
//!
//! Uses `LengthDelimitedCodec` and bitcode for maximum IPC performance.

use anyhow::Result;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use governor::{Quota, RateLimiter};

use std::sync::Arc;
use std::time::Duration;
use tokio::net::UnixListener;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
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
const STATUS_REFRESH_INTERVAL: Duration = Duration::from_secs(300);

/// Per-connection rate limit (requests per second)
const CLIENT_RATE_LIMIT_HZ: u32 = 50;
/// Per-connection burst size
const CLIENT_BURST_SIZE: u32 = 100;

/// Run the daemon server
pub async fn run(listener: UnixListener) -> Result<()> {
    let state = Arc::new(DaemonState::new()?);
    let shutdown_token = CancellationToken::new();

    // START BACKGROUND WORKER
    let state_worker = Arc::clone(&state);
    let worker_token = shutdown_token.child_token();

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
                (*state.runtime_versions.write()).clone_from(&versions);

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
                    let _ = state.persistent.set_status(&res);
                    state.cache.update_status(res);
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
        }

        // Initial refresh
        refresh_status(&state_worker).await;

        loop {
            tokio::select! {
                () = tokio::time::sleep(STATUS_REFRESH_INTERVAL) => {
                    tracing::debug!("Refreshing system status cache...");
                    refresh_status(&state_worker).await;
                    tracing::debug!("Status cache refreshed");
                }
                () = worker_token.cancelled() => {
                    tracing::info!("Background worker shutting down");
                    break;
                }
            }
        }
    });

    tracing::info!("Daemon ready, binary IPC enabled");

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, _addr) = result?;
                let state = Arc::clone(&state);
                let client_token = shutdown_token.child_token();

                tokio::spawn(async move {
                    tokio::select! {
                        result = handle_client(stream, state) => {
                            if let Err(e) = result {
                                tracing::error!("Client error: {}", e);
                            }
                        }
                        () = client_token.cancelled() => {
                            tracing::debug!("Client connection closed due to shutdown");
                        }
                    }
                });
            }

            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutdown signal received, cleaning up...");
                shutdown_token.cancel();
                break;
            }
        }
    }

    Ok(())
}

/// Maximum request size to prevent `DoS` attacks (1MB should be sufficient)
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
async fn handle_client(stream: tokio::net::UnixStream, state: Arc<DaemonState>) -> Result<()> {
    // METRICS: Track active connections using RAII guard
    let _guard = ConnectionGuard::new();

    // Use length-delimited framing for binary messages with max frame length
    let mut codec = LengthDelimitedCodec::new();
    codec.set_max_frame_length(MAX_REQUEST_SIZE);
    let mut framed = Framed::new(stream, codec);

    // Rate limit per connection to ensure fairness
    let quota = Quota::per_second(crate::core::safe_ops::nonzero_u32_or_default(
        CLIENT_RATE_LIMIT_HZ,
        1,
    ))
    .allow_burst(crate::core::safe_ops::nonzero_u32_or_default(
        CLIENT_BURST_SIZE,
        1,
    ));
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
