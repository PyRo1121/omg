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
        requests: Box<Vec<Request>>,
    },
    /// Search Debian/Ubuntu packages (apt)
    DebianSearch {
        id: RequestId,
        query: String,
        limit: Option<usize>,
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
            | Self::DebianSearch { id, .. } => *id,
        }
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
    Batch(Box<Vec<Response>>),
    /// Debian search results (list of package info)
    DebianSearch(Vec<PackageInfo>),
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
    pub runtime_versions: Vec<(String, String)>,
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
