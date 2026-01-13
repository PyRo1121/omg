//! Daemon server implementation with Unix socket IPC

use anyhow::Result;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::broadcast;

use super::handlers::{handle_request, DaemonState};
use super::protocol::{Request, Response};

/// Run the daemon server
pub async fn run(listener: UnixListener) -> Result<()> {
    let state = Arc::new(DaemonState::new());
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // START BACKGROUND WORKER
    let state_worker = Arc::clone(&state);
    let mut shutdown_worker = shutdown_tx.subscribe();

    tokio::spawn(async move {
        tracing::info!("Background status worker started");
        loop {
            tokio::select! {
                _ = tokio::time::sleep(std::time::Duration::from_secs(300)) => {
                    tracing::debug!("Refreshing system status cache...");
                    use crate::package_managers::get_system_status;
                    if let Ok((total, explicit, orphans, updates)) = get_system_status() {
                        let res = super::protocol::StatusResult {
                            total_packages: total,
                            explicit_packages: explicit,
                            orphan_packages: orphans,
                            updates_available: updates,
                            security_vulnerabilities: 0, // Updated by security worker
                        };
                        // Update both caches
                        let _ = state_worker.persistent.set_status(res.clone());
                        state_worker.cache.update_status(res);
                        tracing::debug!("Status cache refreshed");
                    }
                }
                _ = shutdown_worker.recv() => {
                    tracing::info!("Background worker shutting down");
                    break;
                }
            }
        }
    });

    tracing::info!("Daemon ready, waiting for connections...");
    tracing::info!(
        "Cache initialized with {} max entries",
        state.cache.stats().max_size
    );

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

    tracing::info!("Daemon shutdown complete");
    Ok(())
}

/// Handle a single client connection
async fn handle_client(stream: tokio::net::UnixStream, state: Arc<DaemonState>) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    tracing::debug!("New client connected");

    while reader.read_line(&mut line).await? > 0 {
        let trimmed = line.trim();

        // Skip empty lines
        if trimmed.is_empty() {
            line.clear();
            continue;
        }

        // Parse request
        let response = match serde_json::from_str::<Request>(trimmed) {
            Ok(request) => {
                let _id = request.id;
                handle_request(Arc::clone(&state), request).await
            }
            Err(e) => {
                tracing::warn!("Failed to parse request: {}", e);
                Response::error(
                    0,
                    super::protocol::error_codes::PARSE_ERROR,
                    format!("Parse error: {}", e),
                )
            }
        };

        // Send response
        let response_json = serde_json::to_string(&response)?;
        writer.write_all(response_json.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        line.clear();
    }

    tracing::debug!("Client disconnected");
    Ok(())
}
