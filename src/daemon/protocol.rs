//! IPC Protocol Types
//!
//! JSON-RPC style protocol for daemon communication.

use serde::{Deserialize, Serialize};

/// Request ID type
pub type RequestId = u64;

/// IPC Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// Unique request ID for correlation
    pub id: RequestId,
    /// Method name
    pub method: String,
    /// Parameters (JSON value)
    #[serde(default)]
    pub params: serde_json::Value,
}

/// IPC Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Request ID this responds to
    pub id: RequestId,
    /// Result if successful
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    /// Error if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

/// RPC Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcError {
    /// Error code
    pub code: i32,
    /// Error message
    pub message: String,
    /// Additional data
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl Response {
    /// Create a successful response
    pub fn success<T: Serialize>(id: RequestId, result: T) -> Self {
        Response {
            id,
            result: Some(serde_json::to_value(result).unwrap_or(serde_json::Value::Null)),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: RequestId, code: i32, message: impl Into<String>) -> Self {
        Response {
            id,
            result: None,
            error: Some(RpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

// Standard error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
    pub const PACKAGE_NOT_FOUND: i32 = -1001;
    pub const INSTALL_FAILED: i32 = -1002;
}

/// Search request parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParams {
    pub query: String,
    #[serde(default)]
    pub limit: Option<usize>,
}

/// Search result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub packages: Vec<PackageInfo>,
    pub total: usize,
}

/// Status result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusResult {
    pub total_packages: usize,
    pub explicit_packages: usize,
    pub orphan_packages: usize,
    pub updates_available: usize,
    pub security_vulnerabilities: usize,
}

/// Package info for IPC (minimal)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub source: String, // "official" or "aur"
}

/// Detailed package info for IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub source: String, // "official" or "aur"
}

/// Vulnerability info for IPC
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vulnerability {
    pub id: String,
    pub summary: String,
    pub score: Option<String>,
}

/// Security audit result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityAuditResult {
    pub total_vulnerabilities: usize,
    pub high_severity: usize,
    pub vulnerabilities: Vec<(String, Vec<Vulnerability>)>, // (PackageName, [Vulns])
}
