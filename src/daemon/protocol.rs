//! IPC Protocol Types (Binary)
//!
//! Uses bitcode for maximum performance (fastest Rust serializer).
//! Uses serde integration to avoid recursion limit issues with recursive types.

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};

/// Request ID type
pub type RequestId = u64;

/// Unified Request Enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Request {
    Search {
        id: RequestId,
        query: String,
        limit: Option<usize>,
    },
    Info {
        id: RequestId,
        package: String,
    },
    Status {
        id: RequestId,
    },
    Explicit {
        id: RequestId,
    },
    ExplicitCount {
        id: RequestId,
    },
    SecurityAudit {
        id: RequestId,
    },
    Ping {
        id: RequestId,
    },
    CacheStats {
        id: RequestId,
    },
    CacheClear {
        id: RequestId,
    },
    /// Get system metrics (Prometheus-style)
    Metrics {
        id: RequestId,
    },
    /// Get fuzzy suggestions for a package name
    Suggest {
        id: RequestId,
        query: String,
        limit: Option<usize>,
    },
    /// Batch multiple requests in a single IPC round-trip
    Batch {
        id: RequestId,
        requests: Vec<Request>,
    },
    /// Search Debian/Ubuntu packages (apt)
    DebianSearch {
        id: RequestId,
        query: String,
        limit: Option<usize>,
    },
    /// Get daemon health status
    Health {
        id: RequestId,
    },
    /// List available package updates (uses hot ALPM worker)
    ListUpdates {
        id: RequestId,
    },
}

impl Request {
    #[must_use]
    pub const fn id(&self) -> RequestId {
        match self {
            Self::Search { id, .. }
            | Self::Info { id, .. }
            | Self::Status { id }
            | Self::Explicit { id }
            | Self::ExplicitCount { id }
            | Self::SecurityAudit { id }
            | Self::Ping { id }
            | Self::CacheStats { id }
            | Self::CacheClear { id }
            | Self::Metrics { id }
            | Self::Suggest { id, .. }
            | Self::Batch { id, .. }
            | Self::DebianSearch { id, .. }
            | Self::Health { id }
            | Self::ListUpdates { id } => *id,
        }
    }

    /// Returns the variant name as a static string for tracing/logging
    #[must_use]
    pub const fn variant_name(&self) -> &'static str {
        match self {
            Self::Search { .. } => "search",
            Self::Info { .. } => "info",
            Self::Status { .. } => "status",
            Self::Explicit { .. } => "explicit",
            Self::ExplicitCount { .. } => "explicit_count",
            Self::SecurityAudit { .. } => "security_audit",
            Self::Ping { .. } => "ping",
            Self::CacheStats { .. } => "cache_stats",
            Self::CacheClear { .. } => "cache_clear",
            Self::Metrics { .. } => "metrics",
            Self::Suggest { .. } => "suggest",
            Self::Batch { .. } => "batch",
            Self::DebianSearch { .. } => "debian_search",
            Self::Health { .. } => "health",
            Self::ListUpdates { .. } => "list_updates",
        }
    }

    /// Estimated heap footprint in bytes (stack size + owned heap contents).
    ///
    /// Used by the daemon's compression-bomb guard. `std::mem::size_of_val`
    /// only measures the enum's stack size and can never see `String`/`Vec`
    /// payloads, so a guard built on it can never fire.
    ///
    /// Recursion over `Batch` children is safe because the daemon validates
    /// batch nesting depth before calling this.
    #[must_use]
    pub fn heap_size(&self) -> usize {
        let mut size = std::mem::size_of::<Self>();
        match self {
            Self::Search { query, .. }
            | Self::Suggest { query, .. }
            | Self::DebianSearch { query, .. } => size += query.capacity(),
            Self::Info { package, .. } => size += package.capacity(),
            Self::Batch { requests, .. } => {
                for request in requests {
                    size += request.heap_size();
                }
                // Account for over-allocated but unused slots in the Vec buffer.
                size += (requests.capacity() - requests.len()) * std::mem::size_of::<Self>();
            }
            Self::Status { .. }
            | Self::Explicit { .. }
            | Self::ExplicitCount { .. }
            | Self::SecurityAudit { .. }
            | Self::Ping { .. }
            | Self::CacheStats { .. }
            | Self::CacheClear { .. }
            | Self::Metrics { .. }
            | Self::Health { .. }
            | Self::ListUpdates { .. } => {}
        }
        size
    }
}

/// Unified Response Enum
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Response {
    Success {
        id: RequestId,
        result: ResponseResult,
    },
    Error {
        id: RequestId,
        code: i32,
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResponseResult {
    Search(SearchResult),
    Info(DetailedPackageInfo),
    Status(StatusResult),
    Explicit(ExplicitResult),
    ExplicitCount(usize),
    SecurityAudit(SecurityAuditResult),
    Ping(String),
    CacheStats {
        size: usize,
        max_size: usize,
    },
    Metrics(MetricsSnapshot),
    Suggest(Vec<String>),
    Message(String),
    /// Batch response containing multiple results
    Batch(Vec<Response>),
    /// Debian search results (list of package info)
    DebianSearch(Vec<PackageInfo>),
    Health(HealthStatus),
    ListUpdates(Vec<UpdateEntry>),
}

// Error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const PACKAGE_NOT_FOUND: i32 = -1001;
    pub const RATE_LIMITED: i32 = -1002;
}

/// Search result
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub packages: Vec<PackageInfo>,
    pub total: usize,
}

/// Explicit packages result
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Serialize, Deserialize)]
pub struct ExplicitResult {
    pub packages: Vec<String>,
}

/// Status result
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Serialize, Deserialize)]
pub struct StatusResult {
    pub total_packages: usize,
    pub explicit_packages: usize,
    pub orphan_packages: usize,
    pub updates_available: usize,
    pub security_vulnerabilities: usize,
    /// False when `security_vulnerabilities` was not produced by a scan.
    pub vulnerabilities_scanned: bool,
    pub runtime_versions: Vec<(String, String)>,
}

impl StatusResult {
    /// Vulnerability count from a completed scan. `None` means not scanned.
    #[must_use]
    pub const fn scanned_vulnerability_count(&self) -> Option<usize> {
        if self.vulnerabilities_scanned {
            Some(self.security_vulnerabilities)
        } else {
            None
        }
    }
}

/// Map a vulnerability scan onto a previously published count.
///
/// A failed scan keeps the previous count and must not invent a clean zero.
pub fn vulnerability_count_from_scan<E>(
    scan: &Result<usize, E>,
    previous: Option<usize>,
) -> Option<usize> {
    match scan {
        Ok(count) => Some(*count),
        Err(_) => previous,
    }
}

/// Build a status snapshot. Only a completed vulnerability scan may be cached.
pub fn status_snapshot(
    total_packages: usize,
    explicit_packages: usize,
    orphan_packages: usize,
    updates_available: usize,
    runtime_versions: Vec<(String, String)>,
    scanned_vulnerabilities: Option<usize>,
) -> (StatusResult, bool) {
    (
        StatusResult {
            total_packages,
            explicit_packages,
            orphan_packages,
            updates_available,
            security_vulnerabilities: scanned_vulnerabilities.unwrap_or(0),
            vulnerabilities_scanned: scanned_vulnerabilities.is_some(),
            runtime_versions,
        },
        scanned_vulnerabilities.is_some(),
    )
}

/// Package info for IPC (minimal)
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String,
}

/// Detailed package info for IPC
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Serialize, Deserialize)]
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

/// Vulnerability info for IPC
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub id: String,
    pub summary: String,
    pub score: Option<String>,
}

/// Security audit result
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditResult {
    pub total_vulnerabilities: usize,
    pub high_severity: usize,
    pub vulnerabilities: Vec<(String, Vec<Vulnerability>)>,
}

/// System metrics snapshot
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Serialize, Deserialize)]
pub struct MetricsSnapshot {
    pub requests_total: u64,
    pub requests_failed: u64,
    pub rate_limit_hits: u64,
    pub validation_failures: u64,
    pub active_connections: i64,
    pub security_audit_requests: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub search_requests: u64,
    pub info_requests: u64,
    pub status_requests: u64,
}

#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub uptime_seconds: u64,
    pub memory_usage_mb: u64,
    pub cache_size: usize,
    pub active_connections: i64,
}

/// Update entry for IPC (matches `UpdateInfo` from `package_managers::types`)
#[derive(Archive, RkyvSerialize, RkyvDeserialize, Debug, Clone, Serialize, Deserialize)]
pub struct UpdateEntry {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
    pub repo: String,
}

// ═══════════════════════════════════════════════════════════════════════════
// Synchronous framing helpers
// ═══════════════════════════════════════════════════════════════════════════

/// Maximum frame size accepted by [`read_frame`] (10 MiB). Matches the
/// daemon's response budget and prevents a hostile or corrupt peer from
/// triggering unbounded client-side allocation.
pub const MAX_FRAME_SIZE: usize = 10 * 1024 * 1024;

/// Write a length-delimited frame: big-endian `u32` length prefix + payload.
///
/// # Errors
/// Returns `InvalidInput` if the payload exceeds `u32::MAX`, or propagates
/// the underlying I/O error.
pub fn write_frame<W: std::io::Write>(writer: &mut W, payload: &[u8]) -> std::io::Result<()> {
    let len = u32::try_from(payload.len()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "payload too large for protocol framing",
        )
    })?;
    // Single write: one syscall, no interleaving risk on a shared socket.
    let mut buf = Vec::with_capacity(4 + payload.len());
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(payload);
    writer.write_all(&buf)
}

/// Read a length-delimited frame written by [`write_frame`].
///
/// # Errors
/// Returns `InvalidData` if the announced length exceeds [`MAX_FRAME_SIZE`],
/// or propagates the underlying I/O error.
pub fn read_frame<R: std::io::Read>(reader: &mut R) -> std::io::Result<Vec<u8>> {
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf)?;
    let len = u32::from_be_bytes(len_buf) as usize;
    if len > MAX_FRAME_SIZE {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("frame length {len} exceeds maximum {MAX_FRAME_SIZE}"),
        ));
    }
    let mut payload = vec![0u8; len];
    reader.read_exact(&mut payload)?;
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::{status_snapshot, vulnerability_count_from_scan};

    #[test]
    fn successful_scan_replaces_the_previous_count() {
        assert_eq!(
            vulnerability_count_from_scan::<()>(&Ok(3), Some(5)),
            Some(3)
        );
    }

    #[test]
    fn failed_scan_keeps_the_previous_count() {
        assert_eq!(
            vulnerability_count_from_scan(&Err("alsa unavailable"), Some(5)),
            Some(5)
        );
    }

    #[test]
    fn failed_scan_without_a_prior_count_does_not_invent_zero() {
        assert_eq!(
            vulnerability_count_from_scan(&Err("alsa unavailable"), None),
            None
        );
    }

    #[test]
    fn unscanned_status_is_not_cacheable() {
        let (status, cacheable) = status_snapshot(10, 4, 1, 2, vec![], None);
        assert!(!cacheable);
        assert!(!status.vulnerabilities_scanned);
        assert_eq!(status.scanned_vulnerability_count(), None);
        assert_eq!(status.security_vulnerabilities, 0);
        assert_eq!(status.total_packages, 10);
    }

    #[test]
    fn scanned_status_is_cacheable() {
        let (status, cacheable) = status_snapshot(10, 4, 1, 2, vec![], Some(7));
        assert!(cacheable);
        assert!(status.vulnerabilities_scanned);
        assert_eq!(status.scanned_vulnerability_count(), Some(7));
        assert_eq!(status.security_vulnerabilities, 7);
    }

    #[test]
    fn heap_size_counts_string_payloads() {
        let request = super::Request::Search {
            id: 1,
            query: "x".repeat(2048),
            limit: None,
        };
        let size = request.heap_size();
        assert!(
            size >= 2048,
            "heap_size must include String payloads, got {size}"
        );
    }

    #[test]
    fn heap_size_counts_nested_batch_payloads() {
        let inner = super::Request::Batch {
            id: 2,
            requests: vec![super::Request::Info {
                id: 3,
                package: "y".repeat(1024),
            }],
        };
        let outer = super::Request::Batch {
            id: 1,
            requests: vec![inner],
        };
        let size = outer.heap_size();
        assert!(
            size >= 1024 + 2 * std::mem::size_of::<super::Request>(),
            "heap_size must walk nested batch payloads, got {size}"
        );
    }

    #[test]
    fn heap_size_of_unit_variants_is_stack_only() {
        let size = super::Request::Ping { id: 1 }.heap_size();
        assert_eq!(size, std::mem::size_of::<super::Request>());
    }
}
