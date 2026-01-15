//! Daemon server implementation with Unix socket IPC
//!
//! Uses `LengthDelimitedCodec` and Bincode for maximum IPC performance.

use anyhow::Result;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std::sync::Arc;
use tokio::net::UnixListener;
use tokio::sync::broadcast;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use super::handlers::{handle_request, DaemonState};
use super::protocol::Request;

/// Run the daemon server
pub async fn run(listener: UnixListener) -> Result<()> {
    let state = Arc::new(DaemonState::new());
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // START BACKGROUND WORKER
    let state_worker = Arc::clone(&state);
    let mut shutdown_worker = shutdown_tx.subscribe();

    tokio::spawn(async move {
        tracing::info!("Background status worker started");

        // Initial refresh
        {
            use crate::package_managers::get_system_status;
            use crate::runtimes::{probe_version, SUPPORTED_RUNTIMES};
            let mut versions = Vec::new();
            for runtime in SUPPORTED_RUNTIMES {
                if let Some(v) = probe_version(runtime) {
                    versions.push(((*runtime).to_string(), v));
                }
            }
            state_worker
                .runtime_versions
                .write()
                .clone_from(&versions);
            if let Ok((total, explicit, orphans, updates)) = get_system_status() {
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
        }

        loop {
            tokio::select! {
                () = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                    tracing::debug!("Refreshing system status cache...");
                    use crate::package_managers::get_system_status;
                    use crate::runtimes::{SUPPORTED_RUNTIMES, probe_version};

                    // 1. Probe Runtimes (Fast)
                    let mut versions = Vec::new();
                    for runtime in SUPPORTED_RUNTIMES {
                        if let Some(v) = probe_version(runtime) {
                            versions.push(((*runtime).to_string(), v));
                        }
                    }
                    state_worker
                        .runtime_versions
                        .write()
                        .clone_from(&versions);

                    // 2. Refresh Package Status (ALPM)
                    let status = get_system_status();

                    // 3. Scan for Vulnerabilities (New!)
                    let scanner = crate::core::security::VulnerabilityScanner::new();
                    let vuln_count = scanner.scan_system().await.unwrap_or(0);

                    if let Ok((total, explicit, orphans, updates)) = status {
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
                _ = shutdown_worker.recv() => {
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
                let mut shutdown_rx = shutdown_tx.subscribe();

                tokio::spawn(async move {
                    tokio::select! {
                        result = handle_client(stream, state) => {
                            if let Err(e) = result {
                                tracing::error!("Client error: {}", e);
                            }
                        }
                        _ = shutdown_rx.recv() => {
                            tracing::debug!("Client connection closed due to shutdown");
                        }
                    }
                });
            }

            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Shutdown signal received, cleaning up...");
                let _ = shutdown_tx.send(());
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
        let request: Request = bincode::deserialize(&bytes)?;

        // Handle request
        let response = handle_request(Arc::clone(&state), request).await;

        // Encode and send response
        let response_bytes = bincode::serialize(&response)?;
        framed.send(response_bytes.into()).await?;
    }

    tracing::debug!("Client disconnected");
    Ok(())
}
