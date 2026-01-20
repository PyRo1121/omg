//! Daemon server implementation with Unix socket IPC
//!
//! Uses `LengthDelimitedCodec` and bitcode for maximum IPC performance.

use anyhow::Result;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UnixListener;
use tokio_util::codec::{Framed, LengthDelimitedCodec};
use tokio_util::sync::CancellationToken;

use super::handlers::{DaemonState, handle_request};
use super::protocol::{Request, Response, error_codes};
use crate::core::env::distro::use_debian_backend;

#[cfg(feature = "debian")]
use crate::package_managers::apt_get_system_status;

/// Request handling timeout (30 seconds should be sufficient for most operations)
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Run the daemon server
pub async fn run(listener: UnixListener) -> Result<()> {
    let state = Arc::new(DaemonState::new());
    let shutdown_token = CancellationToken::new();

    // START BACKGROUND WORKER
    let state_worker = Arc::clone(&state);
    let worker_token = shutdown_token.child_token();

    tokio::spawn(async move {
        tracing::info!("Background status worker started");

        // Initial refresh
        {
            use crate::cli::runtimes::{ensure_active_version, known_runtimes};
            let mut versions = Vec::new();
            for runtime in known_runtimes() {
                if let Some(v) = ensure_active_version(&runtime) {
                    versions.push((runtime, v));
                }
            }
            state_worker.runtime_versions.write().clone_from(&versions);

            #[cfg(feature = "arch")]
            let status = if use_debian_backend() {
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
            let status = if use_debian_backend() {
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

            if let Ok((total, explicit, orphans, updates)) = status {
                // Write fast status file for zero-IPC CLI reads (no vuln scan needed)
                let fast_status =
                    crate::core::fast_status::FastStatus::new(total, explicit, orphans, updates);
                if let Err(e) = fast_status.write_default() {
                    tracing::warn!("Failed to write fast status file: {e}");
                }

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
                let _ = state_worker.persistent.set_status(&res);
                state_worker.cache.update_status(res);
            }

            // Pre-compute explicit package list for instant first query
            #[cfg(feature = "arch")]
            if !use_debian_backend()
                && let Ok(explicit_pkgs) = crate::package_managers::list_explicit_fast()
            {
                state_worker.cache.update_explicit(explicit_pkgs);
                tracing::debug!("Pre-warmed explicit package cache");
            }
        }

        loop {
            tokio::select! {
                () = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                    tracing::debug!("Refreshing system status cache...");
                    use crate::cli::runtimes::{ensure_active_version, known_runtimes};

                    // 1. Probe Runtimes (Fast)
                    let mut versions = Vec::new();
                    for runtime in known_runtimes() {
                        if let Some(v) = ensure_active_version(&runtime) {
                            versions.push((runtime, v));
                        }
                    }
                    state_worker
                        .runtime_versions
                        .write()
                        .clone_from(&versions);

                    // 2. Refresh Package Status
                    #[cfg(feature = "arch")]
                    let status = if use_debian_backend() {
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
                    let status = if use_debian_backend() {
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

                    // 3. Scan for Vulnerabilities (New!)
                    let scanner = crate::core::security::VulnerabilityScanner::new();
                    let vuln_count = scanner.scan_system().await.unwrap_or(0);

                    if let Ok((total, explicit, orphans, updates)) = status {
                        let fast_status = crate::core::fast_status::FastStatus::new(
                            total, explicit, orphans, updates,
                        );
                        if let Err(e) = fast_status.write_default() {
                            tracing::warn!("Failed to write fast status file: {e}");
                        }

                        let res = super::protocol::StatusResult {
                            total_packages: total,
                            explicit_packages: explicit,
                            orphan_packages: orphans,
                            updates_available: updates,
                            security_vulnerabilities: vuln_count,
                            runtime_versions: versions,
                        };
                        let _ = state_worker.persistent.set_status(&res);
                        state_worker.cache.update_status(res);
                        tracing::debug!("Status cache refreshed (CVEs: {})", vuln_count);
                    }
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

/// Handle a single client connection
async fn handle_client(stream: tokio::net::UnixStream, state: Arc<DaemonState>) -> Result<()> {
    // Use length-delimited framing for binary messages
    let mut framed = Framed::new(stream, LengthDelimitedCodec::new());

    tracing::debug!("New binary client connected");

    while let Some(request_bytes) = framed.next().await {
        let bytes = request_bytes?;

        // Decode request
        let request: Request = bitcode::deserialize(&bytes)?;
        let request_id = request.id();

        // Handle request with timeout to prevent hung clients
        let response = if let Ok(response) = tokio::time::timeout(
            REQUEST_TIMEOUT,
            handle_request(Arc::clone(&state), request),
        )
        .await
        {
            response
        } else {
            tracing::warn!("Request {} timed out after {:?}", request_id, REQUEST_TIMEOUT);
            Response::Error {
                id: request_id,
                code: error_codes::INTERNAL_ERROR,
                message: format!("Request timed out after {} seconds", REQUEST_TIMEOUT.as_secs()),
            }
        };

        // Encode and send response
        let response_bytes = bitcode::serialize(&response)?;
        framed.send(response_bytes.into()).await?;
    }

    tracing::debug!("Client disconnected");
    Ok(())
}
