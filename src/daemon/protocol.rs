//! IPC Protocol Types (Binary)
//!
//! Uses bincode for maximum performance.

use bincode::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// Request ID type
pub type RequestId = u64;

/// Unified Request Enum
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
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
}

impl Request {
    #[must_use]
    pub const fn id(&self) -> RequestId {
        match self {
            Self::Search { id, .. }
            | Self::Info { id, .. }
            | Self::Status { id }
            | Self::Explicit { id }
            | Self::SecurityAudit { id }
            | Self::Ping { id }
            | Self::CacheStats { id }
            | Self::CacheClear { id } => *id,
        }
    }
}

/// Unified Response Enum
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
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

#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub enum ResponseResult {
    Search(SearchResult),
    Info(DetailedPackageInfo),
    Status(StatusResult),
    Explicit(ExplicitResult),
    SecurityAudit(SecurityAuditResult),
    Ping(String),
    CacheStats { size: usize, max_size: usize },
    Message(String),
}

// Error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const PACKAGE_NOT_FOUND: i32 = -1001;
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SearchResult {
    pub packages: Vec<PackageInfo>,
    pub total: usize,
}

/// Explicit packages result
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct ExplicitResult {
    pub packages: Vec<String>,
}

/// Status result
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct StatusResult {
    pub total_packages: usize,
    pub explicit_packages: usize,
    pub orphan_packages: usize,
    pub updates_available: usize,
    pub security_vulnerabilities: usize,
    pub runtime_versions: Vec<(String, String)>,
}

/// Package info for IPC (minimal)
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String,
}

/// Detailed package info for IPC
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
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
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct Vulnerability {
    pub id: String,
    pub summary: String,
    pub score: Option<String>,
}

/// Security audit result
#[derive(Debug, Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SecurityAuditResult {
    pub total_vulnerabilities: usize,
    pub high_severity: usize,
    pub vulnerabilities: Vec<(String, Vec<Vulnerability>)>,
}
