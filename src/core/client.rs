//! IPC Client for communicating with the daemon
//!
//! Uses LengthDelimitedCodec and Bincode for maximum IPC performance.

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::UnixStream;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::daemon::protocol::*;

/// Get the default socket path
pub fn default_socket_path() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(|d| PathBuf::from(d).join("omg.sock"))
        .unwrap_or_else(|_| PathBuf::from("/tmp/omg.sock"))
}

/// IPC Client for daemon communication
pub struct DaemonClient {
    framed: Framed<UnixStream, LengthDelimitedCodec>,
    request_id: AtomicU64,
}

impl DaemonClient {
    /// Connect to the daemon
    pub async fn connect() -> Result<Self> {
        Self::connect_to(default_socket_path()).await
    }

    /// Connect to daemon at specific socket path
    pub async fn connect_to(socket_path: PathBuf) -> Result<Self> {
        tracing::debug!("Connecting to daemon at {:?}", socket_path);
        let stream = UnixStream::connect(&socket_path)
            .await
            .with_context(|| format!("Failed to connect to daemon at {:?}", socket_path))?;

        tracing::debug!("Connected to daemon");
        let framed = Framed::new(stream, LengthDelimitedCodec::new());

        Ok(DaemonClient {
            framed,
            request_id: AtomicU64::new(1),
        })
    }

    /// Check if daemon is running
    pub async fn is_running() -> bool {
        Self::connect().await.is_ok()
    }

    /// Send a request and get response
    pub async fn call(&mut self, request: Request) -> Result<ResponseResult> {
        let id = request.id();
        
        // Encode and send
        let request_bytes = bincode::serialize(&request)?;
        self.framed.send(request_bytes.into()).await?;

        // Read and decode response
        let response_bytes = self.framed.next().await
            .ok_or_else(|| anyhow::anyhow!("Daemon disconnected"))??;
            
        let response: Response = bincode::deserialize(&response_bytes)?;

        match response {
            Response::Success { id: resp_id, result } => {
                if resp_id != id {
                    anyhow::bail!("Request ID mismatch: sent {}, got {}", id, resp_id);
                }
                Ok(result)
            }
            Response::Error { id: _, code, message } => {
                anyhow::bail!("Daemon error ({}): {}", code, message);
            }
        }
    }

    /// Ping the daemon
    pub async fn ping(&mut self) -> Result<String> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(Request::Ping { id }).await? {
            ResponseResult::Ping(s) => Ok(s),
            _ => anyhow::bail!("Invalid response type"),
        }
    }

    /// Search for packages
    pub async fn search(&mut self, query: &str, limit: Option<usize>) -> Result<SearchResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(Request::Search { id, query: query.to_string(), limit }).await? {
            ResponseResult::Search(res) => Ok(res),
            _ => anyhow::bail!("Invalid response type"),
        }
    }

    /// Get package info
    pub async fn info(&mut self, package: &str) -> Result<DetailedPackageInfo> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(Request::Info { id, package: package.to_string() }).await? {
            ResponseResult::Info(res) => Ok(res),
            _ => anyhow::bail!("Invalid response type"),
        }
    }

    /// Get system status
    pub async fn status(&mut self) -> Result<StatusResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(Request::Status { id }).await? {
            ResponseResult::Status(res) => Ok(res),
            _ => anyhow::bail!("Invalid response type"),
        }
    }

    /// Trigger a security audit
    pub async fn security_audit(&mut self) -> Result<SecurityAuditResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(Request::SecurityAudit { id }).await? {
            ResponseResult::SecurityAudit(res) => Ok(res),
            _ => anyhow::bail!("Invalid response type"),
        }
    }

    /// List explicitly installed packages
    pub async fn list_explicit(&mut self) -> Result<Vec<String>> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(Request::Explicit { id }).await? {
            ResponseResult::Explicit(res) => Ok(res.packages),
            _ => anyhow::bail!("Invalid response type"),
        }
    }
}
