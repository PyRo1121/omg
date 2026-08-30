//! IPC Client for communicating with the daemon
//!
//! Uses `LengthDelimitedCodec` and bitcode for maximum IPC performance.
//! Only available on Unix platforms (uses Unix domain sockets).

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use crate::core::paths;

/// Per-operation timeout budget for one daemon request (send *and* receive).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

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
    DetailedPackageInfo, MAX_FRAME_SIZE, Request, Response, ResponseResult, SearchResult,
    SecurityAuditResult, StatusResult, UpdateEntry,
};
#[cfg(unix)]
use std::os::unix::net::UnixStream as SyncUnixStream;

/// Create a new sync connection to the daemon
fn connect_sync_stream() -> Result<SyncUnixStream> {
    tracing::debug!("Connecting to daemon...");

    let socket_path = default_socket_path();
    paths::validate_socket_parent(&socket_path).with_context(|| {
        format!(
            "Refusing insecure daemon socket directory for {}",
            socket_path.display()
        )
    })?;
    let stream = SyncUnixStream::connect(&socket_path)
        .with_context(|| format!("Failed to connect to daemon at {}", socket_path.display()))?;

    stream
        .set_read_timeout(Some(REQUEST_TIMEOUT))
        .context("Failed to set daemon read timeout")?;
    stream
        .set_write_timeout(Some(REQUEST_TIMEOUT))
        .context("Failed to set daemon write timeout")?;

    Ok(stream)
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
    pub(crate) fn daemon_disabled() -> bool {
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
        paths::validate_socket_parent(&socket_path).with_context(|| {
            format!(
                "Refusing insecure daemon socket directory for {}",
                socket_path.display()
            )
        })?;
        tracing::debug!("Connecting to daemon at {:?}", socket_path);

        const MAX_CONNECT_RETRIES: u32 = 2;
        // Indexed by `attempt`; `attempt < MAX_CONNECT_RETRIES == len()`
        // whenever a sleep happens, so indexing is in-bounds.
        const CONNECT_BACKOFF_MS: &[u64] = &[25, 50];

        let mut attempt: u32 = 0;
        loop {
            match UnixStream::connect(&socket_path).await {
                Ok(stream) => {
                    if attempt > 0 {
                        tracing::debug!("Connected to daemon after {} retries", attempt);
                    } else {
                        tracing::debug!("Connected to daemon");
                    }
                    let framed = Framed::new(
                        stream,
                        LengthDelimitedCodec::builder()
                            .max_frame_length(MAX_FRAME_SIZE)
                            .new_codec(),
                    );
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
                    if !retryable || attempt >= MAX_CONNECT_RETRIES {
                        let suffix = if attempt >= MAX_CONNECT_RETRIES {
                            " after retries"
                        } else {
                            ""
                        };
                        return Err(anyhow::Error::new(e).context(format!(
                            "Failed to connect to daemon at {}{suffix}",
                            socket_path.display()
                        )));
                    }

                    let backoff_ms = CONNECT_BACKOFF_MS[attempt as usize];
                    tracing::debug!(
                        "Connect attempt {} failed ({}), retrying in {}ms",
                        attempt + 1,
                        e,
                        backoff_ms
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
                    attempt += 1;
                }
            }
        }
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

    /// Send a request and get response
    pub async fn call(&mut self, request: Request) -> Result<ResponseResult> {
        let id = request.id();
        let framed = self.framed.as_mut().context("Client is in sync mode")?;

        // Encode and send (versioned frame). The send shares the same timeout
        // budget as the read: a wedged daemon that stops draining its socket
        // would otherwise block us indefinitely once the OS send buffer fills.
        // See https://docs.rs/tokio/latest/tokio/time/fn.timeout.html
        let request_bytes =
            crate::daemon::protocol::encode_frame(&request).context("Failed to encode request")?;
        tokio::time::timeout(REQUEST_TIMEOUT, framed.send(request_bytes.into()))
            .await
            .context("Timed out sending request to daemon")??;

        // Read and decode response (version-checked)
        let response_bytes = tokio::time::timeout(REQUEST_TIMEOUT, framed.next())
            .await
            .context("Timed out waiting for daemon response")?
            .ok_or_else(|| anyhow::anyhow!("Daemon disconnected"))??;
        decode_response(&response_bytes, id)
    }

    /// Send a request and get response synchronously (ultra fast)
    pub fn call_sync(&mut self, request: &Request) -> Result<ResponseResult> {
        let stream = self
            .sync_stream
            .as_mut()
            .context("Client is in async mode")?;
        sync_roundtrip(stream, request)
    }

    /// Get package info synchronously
    pub fn info_sync(&mut self, package: &str) -> Result<DetailedPackageInfo> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self.call_sync(&Request::Info {
            id,
            package: package.to_string(),
        })?;
        extract_response(&response, id, as_info)
    }

    /// Ping the daemon
    pub async fn ping(&mut self) -> Result<String> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self.call(Request::Ping { id }).await?;
        extract_response(&response, id, as_ping)
    }

    /// Ping the daemon synchronously
    pub fn ping_sync(&mut self) -> Result<String> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self.call_sync(&Request::Ping { id })?;
        extract_response(&response, id, as_ping)
    }

    /// Search for packages
    pub async fn search(&mut self, query: &str, limit: Option<usize>) -> Result<SearchResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self
            .call(Request::Search {
                id,
                query: query.to_string(),
                limit,
            })
            .await?;
        extract_response(&response, id, as_search)
    }

    /// Get package info
    pub async fn info(&mut self, package: &str) -> Result<DetailedPackageInfo> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self
            .call(Request::Info {
                id,
                package: package.to_string(),
            })
            .await?;
        extract_response(&response, id, as_info)
    }

    /// Get system status
    pub async fn status(&mut self) -> Result<StatusResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self.call(Request::Status { id }).await?;
        extract_response(&response, id, as_status)
    }

    /// List available package updates via daemon (uses hot ALPM worker)
    pub async fn list_updates(&mut self) -> Result<Vec<UpdateEntry>> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self.call(Request::ListUpdates { id }).await?;
        extract_response(&response, id, as_updates)
    }

    /// Rebuild the daemon's immutable package index after a database sync.
    pub async fn refresh_index(&mut self) -> Result<usize> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        match self.call(Request::RefreshIndex { id }).await? {
            ResponseResult::IndexRefreshed { packages } => Ok(packages),
            other => anyhow::bail!("Unexpected response to RefreshIndex request {id}: {other:?}"),
        }
    }

    /// Trigger a security audit
    pub async fn security_audit(&mut self) -> Result<SecurityAuditResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self.call(Request::SecurityAudit { id }).await?;
        extract_response(&response, id, as_audit)
    }

    /// Get fuzzy suggestions for a package name
    pub async fn suggest(&mut self, query: &str, limit: Option<usize>) -> Result<Vec<String>> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self
            .call(Request::Suggest {
                id,
                query: query.to_string(),
                limit,
            })
            .await?;
        extract_response(&response, id, as_suggest)
    }
}

/// The single conversion point from a daemon response to a typed client
/// result. Every accessor on [`DaemonClient`] and [`SyncDaemonClient`]
/// funnels through here, so a protocol mismatch surfaces as one canonical
/// error instead of a dozen ad-hoc bail sites.
/// Decode one received frame into the [`ResponseResult`] answering
/// `expected_id`. Shared by the async [`DaemonClient::call`] path and the
/// sync roundtrip so both wire paths enforce identical protocol rules:
/// version check, bitcode deserialization, ID correlation, and error
/// propagation all live here instead of being duplicated per transport.
fn decode_response(frame: &[u8], expected_id: u64) -> Result<ResponseResult> {
    let (_, payload) = crate::daemon::protocol::split_frame(frame)
        .map_err(|e| anyhow::anyhow!("Daemon protocol error: {e}"))?;
    let response: Response = bitcode::deserialize(payload)
        .map_err(|e| anyhow::anyhow!("Failed to deserialize daemon response: {e}"))?;

    match response {
        Response::Success {
            id: resp_id,
            result,
        } => {
            if resp_id != expected_id {
                anyhow::bail!("Request ID mismatch: sent {expected_id}, got {resp_id}");
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

fn extract_response<T>(
    response: &ResponseResult,
    request_id: u64,
    extract: fn(&ResponseResult) -> Option<T>,
) -> Result<T> {
    extract(response)
        .ok_or_else(|| anyhow::anyhow!("Invalid response type for request {request_id}"))
}

fn as_ping(response: &ResponseResult) -> Option<String> {
    if let ResponseResult::Ping(value) = response {
        Some(value.clone())
    } else {
        None
    }
}

fn as_info(response: &ResponseResult) -> Option<DetailedPackageInfo> {
    if let ResponseResult::Info(value) = response {
        Some(value.clone())
    } else {
        None
    }
}

fn as_search(response: &ResponseResult) -> Option<SearchResult> {
    if let ResponseResult::Search(value) = response {
        Some(value.clone())
    } else {
        None
    }
}

fn as_status(response: &ResponseResult) -> Option<StatusResult> {
    if let ResponseResult::Status(value) = response {
        Some(value.clone())
    } else {
        None
    }
}

fn as_updates(response: &ResponseResult) -> Option<Vec<UpdateEntry>> {
    if let ResponseResult::ListUpdates(value) = response {
        Some(value.clone())
    } else {
        None
    }
}

fn as_audit(response: &ResponseResult) -> Option<SecurityAuditResult> {
    if let ResponseResult::SecurityAudit(value) = response {
        Some(value.clone())
    } else {
        None
    }
}

fn as_suggest(response: &ResponseResult) -> Option<Vec<String>> {
    if let ResponseResult::Suggest(value) = response {
        Some(value.clone())
    } else {
        None
    }
}

/// Serialize `request`, exchange one length-delimited frame with the daemon,
/// and validate the response ID. Shared by the sync client paths above and
/// [`SyncDaemonClient`].
fn sync_roundtrip(stream: &mut SyncUnixStream, request: &Request) -> Result<ResponseResult> {
    let id = request.id();
    let request_bytes = crate::daemon::protocol::encode_frame(request)
        .context("Failed to encode daemon request")?;
    crate::daemon::protocol::write_frame(stream, &request_bytes)
        .context("Failed to write request to daemon socket")?;
    let resp_bytes = crate::daemon::protocol::read_frame(stream)
        .context("Failed to read response from daemon")?;
    decode_response(&resp_bytes, id)
}

/// Synchronous client for non-async contexts
pub struct SyncDaemonClient {
    stream: Option<SyncUnixStream>,
    request_id: AtomicU64,
}

impl SyncDaemonClient {
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
        let stream = self.stream.as_mut().context("Connection not available")?;
        sync_roundtrip(stream, request)
    }

    /// Get package info
    pub fn info(&mut self, package: &str) -> Result<DetailedPackageInfo> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self.call(&Request::Info {
            id,
            package: package.to_string(),
        })?;
        extract_response(&response, id, as_info)
    }

    /// Search packages
    pub fn search(&mut self, query: &str, limit: Option<usize>) -> Result<SearchResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self.call(&Request::Search {
            id,
            query: query.to_string(),
            limit,
        })?;
        extract_response(&response, id, as_search)
    }

    /// Get system status
    pub fn status(&mut self) -> Result<StatusResult> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let response = self.call(&Request::Status { id })?;
        extract_response(&response, id, as_status)
    }
}
