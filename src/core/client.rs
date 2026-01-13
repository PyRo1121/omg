//! IPC Client for communicating with the daemon

use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// Get the default socket path
pub fn default_socket_path() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(|d| PathBuf::from(d).join("omg.sock"))
        .unwrap_or_else(|_| PathBuf::from("/tmp/omg.sock"))
}

/// IPC Client for daemon communication
pub struct DaemonClient {
    reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    writer: tokio::net::unix::OwnedWriteHalf,
    request_id: AtomicU64,
}

/// Request structure (matches daemon/protocol.rs)
#[derive(serde::Serialize)]
struct Request {
    id: u64,
    method: String,
    params: serde_json::Value,
}

/// Response structure (matches daemon/protocol.rs)
#[derive(serde::Deserialize, Debug)]
pub struct Response {
    pub id: u64,
    pub result: Option<serde_json::Value>,
    pub error: Option<RpcError>,
}

#[derive(serde::Deserialize, Debug)]
pub struct RpcError {
    pub code: i32,
    pub message: String,
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
        let (reader, writer) = stream.into_split();
        let reader = BufReader::new(reader);

        Ok(DaemonClient {
            reader,
            writer,
            request_id: AtomicU64::new(1),
        })
    }

    /// Check if daemon is running
    pub async fn is_running() -> bool {
        Self::connect().await.is_ok()
    }

    /// Send a request and get response
    pub async fn call<T: serde::de::DeserializeOwned>(
        &mut self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<T> {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);

        let request = Request {
            id,
            method: method.to_string(),
            params,
        };

        // Send request
        let request_json = serde_json::to_string(&request)?;
        self.writer.write_all(request_json.as_bytes()).await?;
        self.writer.write_all(b"\n").await?;
        self.writer.flush().await?;

        // Read response
        let mut line = String::new();
        self.reader.read_line(&mut line).await?;

        let response: Response = serde_json::from_str(&line)
            .with_context(|| format!("Failed to parse response: {}", line))?;

        // Check for error
        if let Some(error) = response.error {
            anyhow::bail!("Daemon error ({}): {}", error.code, error.message);
        }

        // Parse result
        let result = response
            .result
            .ok_or_else(|| anyhow::anyhow!("Empty response"))?;
        serde_json::from_value(result).context("Failed to parse result")
    }

    /// Ping the daemon
    pub async fn ping(&mut self) -> Result<String> {
        self.call("ping", serde_json::Value::Null).await
    }

    /// Search for packages
    pub async fn search(&mut self, query: &str, limit: Option<usize>) -> Result<SearchResult> {
        self.call(
            "search",
            serde_json::json!({
                "query": query,
                "limit": limit,
            }),
        )
        .await
    }

    /// Get package info
    pub async fn info(&mut self, package: &str) -> Result<DetailedPackageInfo> {
        self.call(
            "info",
            serde_json::json!({
                "package": package,
            }),
        )
        .await
    }

    /// Get system status
    pub async fn status(&mut self) -> Result<crate::daemon::protocol::StatusResult> {
        self.call("status", serde_json::Value::Null).await
    }

    /// Trigger a security audit
    pub async fn security_audit(&mut self) -> Result<SecurityAuditResult> {
        self.call("security_audit", serde_json::Value::Null).await
    }
}

/// Search result from daemon
#[derive(Debug, serde::Deserialize)]
pub struct SearchResult {
    pub packages: Vec<PackageInfo>,
    pub total: usize,
}

/// Package info from daemon (minimal)
#[derive(Debug, serde::Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String,
}

/// Detailed package info from daemon
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DetailedPackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub url: String,
    pub size: u64,
    pub download_size: u64,
    pub repo: String,
    pub depends: Vec<String>,
    pub licenses: Vec<String>,
    pub source: String,
}

/// Cache statistics
#[derive(Debug, serde::Deserialize)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
}

/// Vulnerability info from daemon
#[derive(Debug, serde::Deserialize)]
pub struct Vulnerability {
    pub id: String,
    pub summary: String,
    pub score: Option<String>,
}

/// Security audit result from daemon
#[derive(Debug, serde::Deserialize)]
pub struct SecurityAuditResult {
    pub total_vulnerabilities: usize,
    pub high_severity: usize,
    pub vulnerabilities: Vec<(String, Vec<Vulnerability>)>,
}
