//! IPC Client for communicating with the daemon
//!
//! Uses `LengthDelimitedCodec` and Bincode for maximum IPC performance.

use anyhow::{Context, Result};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::daemon::protocol::{
    DetailedPackageInfo, Request, Response, ResponseResult, SearchResult, SecurityAuditResult,
    StatusResult,
};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream as SyncUnixStream;

/// Get the default socket path
#[must_use]
pub fn default_socket_path() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR").map_or_else(
        |_| PathBuf::from("/tmp/omg.sock"),
        |d| PathBuf::from(d).join("omg.sock"),
    )
}

/// IPC Client for daemon communication
pub struct DaemonClient {
    framed: Option<Framed<UnixStream, LengthDelimitedCodec>>,
    sync_stream: Option<SyncUnixStream>,
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
            .with_context(|| format!("Failed to connect to daemon at {}", socket_path.display()))?;

        tracing::debug!("Connected to daemon");
        let framed = Framed::new(stream, LengthDelimitedCodec::new());

        Ok(Self {
            framed: Some(framed),
            sync_stream: None,
            request_id: AtomicU64::new(1),
        })
    }

    /// Connect to the daemon synchronously (sub-millisecond)
    pub fn connect_sync() -> Result<Self> {
        let socket_path = default_socket_path();
        let stream = SyncUnixStream::connect(&socket_path)
            .with_context(|| format!("Failed to connect to daemon at {}", socket_path.display()))?;

        Ok(Self {
            framed: None,
            sync_stream: Some(stream),
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
        let framed = self.framed.as_mut().context("Client is in sync mode")?;

        // Encode and send
        let request_bytes = bincode::serialize(&request)?;
        framed.send(request_bytes.into()).await?;

        // Read and decode response
        let response_bytes = framed
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("Daemon disconnected"))??;

        let response: Response = bincode::deserialize(&response_bytes)?;

        match response {
            Response::Success {
                id: resp_id,
                result,
            } => {
                if resp_id != id {
                    anyhow::bail!("Request ID mismatch: sent {id}, got {resp_id}");
                }
                Ok(result)
            }
            Response::Error {
                id: _,
                code,
                message,
            } => {
                anyhow::bail!("Daemon error ({code}): {message}");
            }
        }
    }

    /// Send a request and get response synchronously (ultra fast)
    pub fn call_sync(&mut self, request: Request) -> Result<ResponseResult> {
        let id = request.id();
        let stream = self
            .sync_stream
            .as_mut()
            .context("Client is in async mode")?;

        // 1. Encode
        let request_bytes = bincode::serialize(&request)?;
        let len = request_bytes.len() as u32;

        // 2. Send length-delimited (Big Endian) combined to save a syscall
        let mut send_buf = Vec::with_capacity(4 + request_bytes.len());
        send_buf.extend_from_slice(&len.to_be_bytes());
        send_buf.extend_from_slice(&request_bytes);
        stream.write_all(&send_buf)?;

        // 3. Read length
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;

        // 4. Read body
        let mut resp_bytes = vec![0u8; resp_len];
        stream.read_exact(&mut resp_bytes)?;

        // 5. Decode
        let response: Response = bincode::deserialize(&resp_bytes)?;

        match response {
            Response::Success {
                id: resp_id,
                result,
            } => {
                if resp_id != id {
                    anyhow::bail!("Request ID mismatch: sent {id}, got {resp_id}");
                }
                Ok(result)
            }
            Response::Error {
                id: _,
                code,
                message,
            } => {
                anyhow::bail!("Daemon error ({code}): {message}");
            }
        }
    }

    /// Get package info synchronously
    pub fn info_sync(&mut self, package: &str) -> Result<DetailedPackageInfo> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call_sync(Request::Info {
            id,
            package: package.to_string(),
        })? {
            ResponseResult::Info(res) => Ok(res),
            _ => anyhow::bail!("Invalid response type"),
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
        match self
            .call(Request::Search {
                id,
                query: query.to_string(),
                limit,
            })
            .await?
        {
            ResponseResult::Search(res) => Ok(res),
            _ => anyhow::bail!("Invalid response type"),
        }
    }

    /// Get package info
    pub async fn info(&mut self, package: &str) -> Result<DetailedPackageInfo> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self
            .call(Request::Info {
                id,
                package: package.to_string(),
            })
            .await?
        {
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
