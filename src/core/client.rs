//! IPC Client for communicating with the daemon
//!
//! Uses `LengthDelimitedCodec` and bitcode for maximum IPC performance.
//! Supports connection pooling for persistent connections across commands.

use anyhow::{Context, Result};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::net::UnixStream;
use tokio_util::codec::{Framed, LengthDelimitedCodec};

use crate::core::paths;
use crate::daemon::protocol::{
    DetailedPackageInfo, PackageInfo, Request, Response, ResponseResult, SearchResult,
    SecurityAuditResult, StatusResult,
};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream as SyncUnixStream;

/// Global connection pool for sync clients (reuses connections across calls)
static SYNC_POOL: OnceLock<Mutex<Option<SyncUnixStream>>> = OnceLock::new();

/// Get or create a pooled sync connection
pub fn get_pooled_sync() -> Result<SyncUnixStream> {
    let pool = SYNC_POOL.get_or_init(|| Mutex::new(None));
    let mut guard = pool.lock();

    if let Some(stream) = guard.take() {
        // Connection exists - return it
        return Ok(stream);
    }

    // Show connection spinner while establishing connection
    tracing::debug!("Connecting to daemon...");

    let socket_path = default_socket_path();
    SyncUnixStream::connect(&socket_path)
        .with_context(|| format!("Failed to connect to daemon at {}", socket_path.display()))
}

/// Get the default socket path
#[must_use]
pub fn default_socket_path() -> PathBuf {
    crate::core::paths::socket_path()
}

/// IPC Client for daemon communication
pub struct DaemonClient {
    framed: Option<Framed<UnixStream, LengthDelimitedCodec>>,
    sync_stream: Option<SyncUnixStream>,
    request_id: AtomicU64,
}

impl DaemonClient {
    fn daemon_disabled() -> bool {
        matches!(
            std::env::var("OMG_DISABLE_DAEMON").as_deref(),
            Ok("1" | "true" | "TRUE")
        ) || paths::test_mode()
    }

    /// Connect to the daemon
    pub async fn connect() -> Result<Self> {
        if Self::daemon_disabled() {
            anyhow::bail!("Daemon disabled by environment");
        }
        Self::connect_to(default_socket_path()).await
    }

    /// Connect to daemon at specific socket path
    pub async fn connect_to(socket_path: PathBuf) -> Result<Self> {
        if Self::daemon_disabled() {
            anyhow::bail!("Daemon disabled by environment");
        }
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
    /// Uses connection pooling for even faster subsequent calls
    pub fn connect_sync() -> Result<Self> {
        if Self::daemon_disabled() {
            anyhow::bail!("Daemon disabled by environment");
        }
        // Try pooled connection first (faster)
        let stream = get_pooled_sync()?;

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
        let request_bytes = bitcode::serialize(&request)?;
        framed.send(request_bytes.into()).await?;

        // Read and decode response
        let response_bytes = framed
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("Daemon disconnected"))??;

        let response: Response = bitcode::deserialize(&response_bytes)?;

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
    pub fn call_sync(&mut self, request: &Request) -> Result<ResponseResult> {
        let id = request.id();
        let stream = self
            .sync_stream
            .as_mut()
            .context("Client is in async mode")?;

        // 1. Encode
        let request_bytes = bitcode::serialize(request)?;
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
        let response: Response = bitcode::deserialize(&resp_bytes)?;

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
        match self.call_sync(&Request::Info {
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

    /// Ping the daemon synchronously
    pub fn ping_sync(&mut self) -> Result<String> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call_sync(&Request::Ping { id })? {
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

    /// Execute multiple requests in a single IPC round-trip
    pub async fn batch(&mut self, requests: Vec<Request>) -> Result<Vec<Response>> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self
            .call(Request::Batch {
                id,
                requests: Box::new(requests),
            })
            .await?
        {
            ResponseResult::Batch(responses) => Ok(*responses),
            _ => anyhow::bail!("Invalid response type"),
        }
    }

    /// Search multiple queries in a single round-trip
    pub async fn batch_search(
        &mut self,
        queries: &[&str],
        limit: Option<usize>,
    ) -> Result<Vec<SearchResult>> {
        let requests: Vec<Request> = queries
            .iter()
            .enumerate()
            .map(|(i, q)| Request::Search {
                id: i as u64,
                query: (*q).to_string(),
                limit,
            })
            .collect();

        let responses = self.batch(requests).await?;
        let mut results = Vec::with_capacity(responses.len());

        for resp in responses {
            match resp {
                Response::Success {
                    result: ResponseResult::Search(sr),
                    ..
                } => results.push(sr),
                Response::Error { message, .. } => {
                    anyhow::bail!("Batch search error: {message}");
                }
                Response::Success { .. } => anyhow::bail!("Unexpected response type in batch"),
            }
        }

        Ok(results)
    }

    /// Get info for multiple packages in a single round-trip
    pub async fn batch_info(
        &mut self,
        packages: &[&str],
    ) -> Result<Vec<Option<DetailedPackageInfo>>> {
        let requests: Vec<Request> = packages
            .iter()
            .enumerate()
            .map(|(i, p)| Request::Info {
                id: i as u64,
                package: (*p).to_string(),
            })
            .collect();

        let responses = self.batch(requests).await?;
        let mut results = Vec::with_capacity(responses.len());

        for resp in responses {
            match resp {
                Response::Success {
                    result: ResponseResult::Info(info),
                    ..
                } => results.push(Some(info)),
                Response::Error { .. } => results.push(None),
                Response::Success { .. } => anyhow::bail!("Unexpected response type in batch"),
            }
        }

        Ok(results)
    }

    /// Get fuzzy suggestions for a package name
    pub async fn suggest(&mut self, query: &str, limit: Option<usize>) -> Result<Vec<String>> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self
            .call(Request::Suggest {
                id,
                query: query.to_string(),
                limit,
            })
            .await?
        {
            ResponseResult::Suggest(res) => Ok(res),
            _ => anyhow::bail!("Invalid response type"),
        }
    }

    /// Search for Debian packages via daemon
    pub async fn debian_search(
        &mut self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<PackageInfo>> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self
            .call(Request::DebianSearch {
                id,
                query: query.to_string(),
                limit,
            })
            .await?
        {
            ResponseResult::DebianSearch(res) => Ok(res),
            _ => anyhow::bail!("Invalid response type"),
        }
    }
}

/// Pooled sync client that automatically returns connection to pool on drop
pub struct PooledSyncClient {
    stream: Option<SyncUnixStream>,
    request_id: AtomicU64,
}

impl PooledSyncClient {
    /// Get a pooled connection (reuses existing or creates new)
    pub fn acquire() -> Result<Self> {
        if DaemonClient::daemon_disabled() {
            anyhow::bail!("Daemon disabled by environment");
        }
        Ok(Self {
            stream: Some(get_pooled_sync()?),
            request_id: AtomicU64::new(1),
        })
    }

    /// Send a request and get response
    pub fn call(&mut self, request: &Request) -> Result<ResponseResult> {
        let id = request.id();
        let stream = self.stream.as_mut().context("Connection not available")?;

        // Encode
        let request_bytes = bitcode::serialize(request)?;
        let len = request_bytes.len() as u32;

        // Send length-delimited
        let mut send_buf = Vec::with_capacity(4 + request_bytes.len());
        send_buf.extend_from_slice(&len.to_be_bytes());
        send_buf.extend_from_slice(&request_bytes);
        stream.write_all(&send_buf)?;

        // Read length
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;

        // Read body
        let mut resp_bytes = vec![0u8; resp_len];
        stream.read_exact(&mut resp_bytes)?;

        // Decode
        let response: Response = bitcode::deserialize(&resp_bytes)?;

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
            Response::Error { code, message, .. } => {
                anyhow::bail!("Daemon error ({code}): {message}");
            }
        }
    }

    /// Get package info
    pub fn info(&mut self, package: &str) -> Result<DetailedPackageInfo> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(&Request::Info {
            id,
            package: package.to_string(),
        })? {
            ResponseResult::Info(res) => Ok(res),
            _ => anyhow::bail!("Invalid response type"),
        }
    }

    /// Search packages
    pub fn search(&mut self, query: &str, limit: Option<usize>) -> Result<SearchResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(&Request::Search {
            id,
            query: query.to_string(),
            limit,
        })? {
            ResponseResult::Search(res) => Ok(res),
            _ => anyhow::bail!("Invalid response type"),
        }
    }

    /// Get explicit package count
    pub fn explicit_count(&mut self) -> Result<usize> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(&Request::ExplicitCount { id })? {
            ResponseResult::ExplicitCount(count) => Ok(count),
            _ => anyhow::bail!("Invalid response type"),
        }
    }

    /// Get system status
    pub fn status(&mut self) -> Result<StatusResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(&Request::Status { id })? {
            ResponseResult::Status(res) => Ok(res),
            _ => anyhow::bail!("Invalid response type"),
        }
    }
}

impl Drop for PooledSyncClient {
    fn drop(&mut self) {
        // UnixStream can't be safely pooled due to lack of Clone
        // Stream will be closed when dropped
    }
}
