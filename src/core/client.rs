//! IPC Client for communicating with the daemon
//!
//! Uses `LengthDelimitedCodec` and bitcode for maximum IPC performance.
//! Only available on Unix platforms (uses Unix domain sockets).

use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::paths;

#[cfg(unix)]
use futures::sink::SinkExt;
#[cfg(unix)]
use futures::stream::StreamExt;
#[cfg(unix)]
use tokio::net::UnixStream;
#[cfg(unix)]
use tokio_util::codec::{Framed, LengthDelimitedCodec};

#[cfg(unix)]
use crate::daemon::protocol::{
    DetailedPackageInfo, PackageInfo, Request, Response, ResponseResult, SearchResult,
    SecurityAuditResult, StatusResult, UpdateEntry,
};
#[cfg(unix)]
use std::io::{Read, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream as SyncUnixStream;

/// Create a new sync connection to the daemon
fn connect_sync_stream() -> Result<SyncUnixStream> {
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

    /// Connect to daemon at specific socket path with fast retry on transient errors.
    ///
    /// Retries up to 2 times on `ECONNREFUSED` (daemon restarting) and `EAGAIN`
    /// (temporary resource exhaustion). Does NOT retry on `ENOENT` (socket missing)
    /// or `EACCES` (permission denied).
    pub async fn connect_to(socket_path: PathBuf) -> Result<Self> {
        if Self::daemon_disabled() {
            anyhow::bail!("Daemon disabled by environment");
        }
        tracing::debug!("Connecting to daemon at {:?}", socket_path);

        const MAX_CONNECT_RETRIES: u32 = 2;
        const CONNECT_BACKOFF_MS: &[u64] = &[25, 50, 100];

        let mut last_err = None;
        for attempt in 0..=MAX_CONNECT_RETRIES {
            match UnixStream::connect(&socket_path).await {
                Ok(stream) => {
                    if attempt > 0 {
                        tracing::debug!("Connected to daemon after {} retries", attempt);
                    } else {
                        tracing::debug!("Connected to daemon");
                    }
                    let framed = Framed::new(stream, LengthDelimitedCodec::new());
                    return Ok(Self {
                        framed: Some(framed),
                        sync_stream: None,
                        request_id: AtomicU64::new(1),
                    });
                }
                Err(e) => {
                    let retryable = matches!(
                        e.kind(),
                        std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::WouldBlock
                    );

                    if !retryable || attempt == MAX_CONNECT_RETRIES {
                        return Err(anyhow::Error::new(e).context(format!(
                            "Failed to connect to daemon at {}",
                            socket_path.display()
                        )));
                    }

                    let backoff_ms = CONNECT_BACKOFF_MS
                        .get(attempt as usize)
                        .copied()
                        .unwrap_or(100);
                    tracing::debug!(
                        "Connect attempt {} failed ({}), retrying in {}ms",
                        attempt + 1,
                        e,
                        backoff_ms
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    last_err = Some(e);
                }
            }
        }

        let final_err = last_err.unwrap_or_else(|| {
            std::io::Error::other("daemon connection retries exhausted with no captured error")
        });

        Err(anyhow::Error::new(final_err).context(format!(
            "Failed to connect to daemon at {} after retries",
            socket_path.display()
        )))
    }

    /// Connect to the daemon, auto-spawning it if not running.
    ///
    /// 1. Tries `connect()` first (fast path).
    /// 2. If that fails and `OMG_NO_AUTO_DAEMON` is not set, spawns the daemon.
    /// 3. Polls up to 2 seconds (20 x 100ms) for the daemon to become ready.
    pub async fn connect_or_spawn() -> Result<Self> {
        struct SpawnLockGuard {
            path: PathBuf,
            _file: File,
        }

        impl SpawnLockGuard {
            fn acquire(path: PathBuf) -> Result<Self> {
                let file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .with_context(|| {
                        format!("Failed to create daemon spawn lock: {}", path.display())
                    })?;

                Ok(Self { path, _file: file })
            }
        }

        impl Drop for SpawnLockGuard {
            fn drop(&mut self) {
                if let Err(err) = std::fs::remove_file(&self.path)
                    && err.kind() != std::io::ErrorKind::NotFound
                {
                    tracing::warn!(
                        "Failed to remove daemon spawn lock {}: {}",
                        self.path.display(),
                        err
                    );
                }
            }
        }

        // Fast path: try connecting directly
        if let Ok(client) = Self::connect().await {
            return Ok(client);
        }

        // Check if auto-spawn is disabled
        if matches!(
            std::env::var("OMG_NO_AUTO_DAEMON").as_deref(),
            Ok("1" | "true" | "TRUE")
        ) {
            anyhow::bail!("Daemon is not running and auto-spawn is disabled (OMG_NO_AUTO_DAEMON)");
        }

        tracing::info!("Daemon not running, auto-spawning...");

        let socket_path = default_socket_path();
        let lock_path = socket_path.with_extension("lock");

        let _spawn_lock = match SpawnLockGuard::acquire(lock_path.clone()) {
            Ok(lock) => lock,
            Err(err)
                if err
                    .downcast_ref::<std::io::Error>()
                    .is_some_and(|io| io.kind() == std::io::ErrorKind::AlreadyExists) =>
            {
                tracing::debug!(
                    "Daemon spawn lock exists, waiting for peer process to finish spawning"
                );

                for attempt in 1..=20u32 {
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

                    if let Ok(client) = Self::connect().await {
                        tracing::info!("Daemon ready after {}ms", attempt * 100);
                        return Ok(client);
                    }
                }

                anyhow::bail!(
                    "Another omg process is spawning the daemon, but it did not become ready within 2 seconds (socket: {})",
                    socket_path.display()
                )
            }
            Err(err) => return Err(err),
        };

        // Remove stale socket file if present
        if let Err(e) = std::fs::remove_file(&socket_path)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("Failed to remove stale socket: {e}");
        }

        // Find the omgd binary (prefer sibling of current executable for version consistency)
        let omgd_path = if let Ok(exe) = std::env::current_exe()
            && let Some(dir) = exe.parent()
        {
            let local_omgd = dir.join("omgd");
            if local_omgd.exists() {
                local_omgd
            } else {
                PathBuf::from("omgd")
            }
        } else {
            PathBuf::from("omgd")
        };

        // Spawn the daemon in background
        std::process::Command::new(&omgd_path)
            .arg("--")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .with_context(|| format!("Failed to spawn daemon: {}", omgd_path.display()))?;

        // Poll for daemon readiness: 20 attempts x 100ms = 2 seconds max
        for attempt in 1..=20u32 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;

            if let Ok(client) = Self::connect().await {
                tracing::info!("Daemon ready after {}ms", attempt * 100);
                return Ok(client);
            }
        }

        anyhow::bail!(
            "Daemon was spawned but did not become ready within 2 seconds (socket: {})",
            socket_path.display()
        )
    }

    /// Connect to the daemon synchronously (sub-millisecond)
    pub fn connect_sync() -> Result<Self> {
        if Self::daemon_disabled() {
            anyhow::bail!("Daemon disabled by environment");
        }
        let stream = connect_sync_stream()?;

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
        let request_bytes =
            bitcode::serialize(request).context("Failed to serialize daemon request")?;
        let len =
            u32::try_from(request_bytes.len()).context("Request too large for protocol framing")?;

        // 2. Send length-delimited (Big Endian) combined to save a syscall
        let mut send_buf = Vec::with_capacity(4 + request_bytes.len());
        send_buf.extend_from_slice(&len.to_be_bytes());
        send_buf.extend_from_slice(&request_bytes);
        stream
            .write_all(&send_buf)
            .context("Failed to write request to daemon socket")?;

        // 3. Read length
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .context("Failed to read response length from daemon")?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;

        // 4. Read body
        let mut resp_bytes = vec![0u8; resp_len];
        stream
            .read_exact(&mut resp_bytes)
            .context("Failed to read response body from daemon")?;

        // 5. Decode
        let response: Response =
            bitcode::deserialize(&resp_bytes).context("Failed to deserialize daemon response")?;

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

    /// List available package updates via daemon (uses hot ALPM worker)
    pub async fn list_updates(&mut self) -> Result<Vec<UpdateEntry>> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(Request::ListUpdates { id }).await? {
            ResponseResult::ListUpdates(updates) => Ok(updates),
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
        match self.call(Request::Batch { id, requests }).await? {
            ResponseResult::Batch(responses) => Ok(responses),
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

/// Synchronous client for non-async contexts
pub struct PooledSyncClient {
    stream: Option<SyncUnixStream>,
    request_id: AtomicU64,
}

impl PooledSyncClient {
    /// Create a new sync connection to the daemon
    pub fn acquire() -> Result<Self> {
        if DaemonClient::daemon_disabled() {
            anyhow::bail!("Daemon disabled by environment");
        }
        Ok(Self {
            stream: Some(connect_sync_stream()?),
            request_id: AtomicU64::new(1),
        })
    }

    /// Send a request and get response
    pub fn call(&mut self, request: &Request) -> Result<ResponseResult> {
        let id = request.id();
        let stream = self.stream.as_mut().context("Connection not available")?;

        // Encode
        let request_bytes =
            bitcode::serialize(request).context("Failed to serialize daemon request")?;
        let len =
            u32::try_from(request_bytes.len()).context("Request too large for protocol framing")?;

        // Send length-delimited
        let mut send_buf = Vec::with_capacity(4 + request_bytes.len());
        send_buf.extend_from_slice(&len.to_be_bytes());
        send_buf.extend_from_slice(&request_bytes);
        stream
            .write_all(&send_buf)
            .context("Failed to write request to daemon socket")?;

        // Read length
        let mut len_buf = [0u8; 4];
        stream
            .read_exact(&mut len_buf)
            .context("Failed to read response length from daemon")?;
        let resp_len = u32::from_be_bytes(len_buf) as usize;

        // Read body
        let mut resp_bytes = vec![0u8; resp_len];
        stream
            .read_exact(&mut resp_bytes)
            .context("Failed to read response body from daemon")?;

        // Decode
        let response: Response =
            bitcode::deserialize(&resp_bytes).context("Failed to deserialize daemon response")?;

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
        // Stream will be closed automatically when dropped
    }
}
