use anyhow::Result;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use crate::core::paths;

/// Audit event types for security logging
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuditEventType {
    // Package operations
    PackageInstall,
    PackageRemove,
    PackageUpgrade,
    PackageDowngrade,

    // Security operations
    SecurityAudit,
    VulnerabilityDetected,
    SignatureVerified,
    SignatureFailed,
    PolicyViolation,

    // Configuration changes
    PolicyChanged,
    ConfigChanged,

    // Authentication/Authorization
    DaemonStarted,
    DaemonStopped,

    // SBOM operations
    SbomGenerated,
    SbomExported,
}

/// Severity levels for audit events
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum AuditSeverity {
    Debug = 0,
    Info = 1,
    Warning = 2,
    Error = 3,
    Critical = 4,
}

impl std::fmt::Display for AuditSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Debug => write!(f, "DEBUG"),
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARNING"),
            Self::Error => write!(f, "ERROR"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// A single audit log entry with tamper detection
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    /// Unique entry ID
    pub id: String,
    /// Timestamp in ISO 8601 format
    pub timestamp: String,
    /// Event type
    pub event_type: AuditEventType,
    /// Severity level
    pub severity: AuditSeverity,
    /// User who performed the action
    pub user: String,
    /// Affected resource (package name, config file, etc.)
    pub resource: String,
    /// Human-readable description
    pub description: String,
    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    /// Hash of previous entry (for chain integrity)
    pub prev_hash: String,
    /// Hash of this entry (computed from all fields except this one)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

impl AuditEntry {
    /// Compute the hash of this entry (excluding the hash field itself)
    pub fn compute_hash(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.id.as_bytes());
        hasher.update(self.timestamp.as_bytes());
        hasher.update(format!("{:?}", self.event_type).as_bytes());
        hasher.update(format!("{:?}", self.severity).as_bytes());
        hasher.update(self.user.as_bytes());
        hasher.update(self.resource.as_bytes());
        hasher.update(self.description.as_bytes());
        if let Some(meta) = &self.metadata {
            hasher.update(meta.to_string().as_bytes());
        }
        hasher.update(self.prev_hash.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Verify the integrity of this entry
    pub fn verify(&self) -> bool {
        if let Some(hash) = &self.hash {
            &self.compute_hash() == hash
        } else {
            false
        }
    }
}

/// Enterprise-grade audit logger with tamper detection
pub struct AuditLogger {
    log_path: PathBuf,
    last_hash: String,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new() -> Result<Self> {
        let log_dir = paths::data_dir().join("audit");
        std::fs::create_dir_all(&log_dir)?;

        let log_path = log_dir.join("audit.jsonl");

        // Get the last hash from the log file
        let last_hash = Self::get_last_hash(&log_path)?;

        Ok(Self {
            log_path,
            last_hash,
        })
    }

    /// Get the hash of the last entry in the log
    fn get_last_hash(path: &Path) -> Result<String> {
        if !path.exists() {
            return Ok("genesis".to_string());
        }

        let file = File::open(path)?;
        let reader = BufReader::new(file);

        let mut last_hash = "genesis".to_string();
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line)
                && let Some(hash) = entry.hash
            {
                last_hash = hash;
            }
        }

        Ok(last_hash)
    }

    /// Log an audit event
    pub fn log(
        &mut self,
        event: AuditEventType,
        severity: AuditSeverity,
        resource: &str,
        description: &str,
    ) -> Result<()> {
        self.log_with_metadata(event, severity, resource, description, None)
    }

    /// Log an audit event with additional metadata
    pub fn log_with_metadata(
        &mut self,
        event: AuditEventType,
        severity: AuditSeverity,
        resource: &str,
        description: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<()> {
        let timestamp = jiff::Zoned::now()
            .strftime("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let id = uuid::Uuid::new_v4().to_string();
        let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());

        let mut entry = AuditEntry {
            id,
            timestamp,
            event_type: event,
            severity,
            user,
            resource: resource.to_string(),
            description: description.to_string(),
            metadata,
            prev_hash: self.last_hash.clone(),
            hash: None,
        };

        // Compute and set the hash
        let hash = entry.compute_hash();
        entry.hash = Some(hash.clone());
        self.last_hash = hash;

        // Append to log file
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)?;

        let json = serde_json::to_string(&entry)?;
        writeln!(file, "{json}")?;

        // Also emit to tracing for real-time monitoring
        match severity {
            AuditSeverity::Debug => {
                tracing::debug!(target: "audit", "{}: {}", entry.event_type_str(), description);
            }
            AuditSeverity::Info => {
                tracing::info!(target: "audit", "{}: {}", entry.event_type_str(), description);
            }
            AuditSeverity::Warning => {
                tracing::warn!(target: "audit", "{}: {}", entry.event_type_str(), description);
            }
            AuditSeverity::Error => {
                tracing::error!(target: "audit", "{}: {}", entry.event_type_str(), description);
            }
            AuditSeverity::Critical => {
                tracing::error!(target: "audit", "CRITICAL - {}: {}", entry.event_type_str(), description);
            }
        }

        Ok(())
    }

    /// Verify the integrity of the entire audit log
    pub fn verify_integrity(&self) -> Result<AuditIntegrityReport> {
        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let mut total_entries = 0;
        let mut valid_entries = 0;
        let mut chain_valid = true;
        let mut expected_prev_hash = "genesis".to_string();
        let mut first_invalid: Option<String> = None;

        for line in reader.lines() {
            let line = line?;
            total_entries += 1;

            if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
                // Verify entry hash
                if entry.verify() {
                    valid_entries += 1;
                } else if first_invalid.is_none() {
                    first_invalid = Some(entry.id.clone());
                }

                // Verify chain integrity
                if entry.prev_hash != expected_prev_hash {
                    chain_valid = false;
                    if first_invalid.is_none() {
                        first_invalid = Some(entry.id.clone());
                    }
                }

                if let Some(hash) = entry.hash {
                    expected_prev_hash = hash;
                }
            } else {
                chain_valid = false;
                if first_invalid.is_none() {
                    first_invalid = Some(format!("line_{total_entries}"));
                }
            }
        }

        Ok(AuditIntegrityReport {
            total_entries,
            valid_entries,
            chain_valid,
            first_invalid_entry: first_invalid,
            log_path: self.log_path.clone(),
        })
    }

    /// Get recent audit entries
    pub fn get_recent(&self, limit: usize) -> Result<Vec<AuditEntry>> {
        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let entries: Vec<AuditEntry> = reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str(&line).ok())
            .collect();

        Ok(entries.into_iter().rev().take(limit).collect())
    }

    /// Filter entries by event type
    pub fn filter_by_type(&self, event_type: &AuditEventType) -> Result<Vec<AuditEntry>> {
        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let entries: Vec<AuditEntry> = reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str::<AuditEntry>(&line).ok())
            .filter(|e| &e.event_type == event_type)
            .collect();

        Ok(entries)
    }

    /// Filter entries by severity (and above)
    pub fn filter_by_severity(&self, min_severity: AuditSeverity) -> Result<Vec<AuditEntry>> {
        let file = File::open(&self.log_path)?;
        let reader = BufReader::new(file);

        let entries: Vec<AuditEntry> = reader
            .lines()
            .map_while(Result::ok)
            .filter_map(|line| serde_json::from_str::<AuditEntry>(&line).ok())
            .filter(|e| e.severity >= min_severity)
            .collect();

        Ok(entries)
    }
}

impl AuditEntry {
    fn event_type_str(&self) -> &'static str {
        match self.event_type {
            AuditEventType::PackageInstall => "PACKAGE_INSTALL",
            AuditEventType::PackageRemove => "PACKAGE_REMOVE",
            AuditEventType::PackageUpgrade => "PACKAGE_UPGRADE",
            AuditEventType::PackageDowngrade => "PACKAGE_DOWNGRADE",
            AuditEventType::SecurityAudit => "SECURITY_AUDIT",
            AuditEventType::VulnerabilityDetected => "VULNERABILITY_DETECTED",
            AuditEventType::SignatureVerified => "SIGNATURE_VERIFIED",
            AuditEventType::SignatureFailed => "SIGNATURE_FAILED",
            AuditEventType::PolicyViolation => "POLICY_VIOLATION",
            AuditEventType::PolicyChanged => "POLICY_CHANGED",
            AuditEventType::ConfigChanged => "CONFIG_CHANGED",
            AuditEventType::DaemonStarted => "DAEMON_STARTED",
            AuditEventType::DaemonStopped => "DAEMON_STOPPED",
            AuditEventType::SbomGenerated => "SBOM_GENERATED",
            AuditEventType::SbomExported => "SBOM_EXPORTED",
        }
    }
}

/// Report from audit log integrity verification
#[derive(Debug, Serialize, Deserialize)]
pub struct AuditIntegrityReport {
    pub total_entries: usize,
    pub valid_entries: usize,
    pub chain_valid: bool,
    pub first_invalid_entry: Option<String>,
    pub log_path: PathBuf,
}

impl AuditIntegrityReport {
    pub fn is_valid(&self) -> bool {
        self.chain_valid && self.total_entries == self.valid_entries
    }
}

/// Global audit logger instance
static AUDIT_LOGGER: std::sync::LazyLock<std::sync::Mutex<Option<AuditLogger>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Initialize the global audit logger
pub fn init_audit_logger() -> Result<()> {
    let logger = AuditLogger::new()?;
    *AUDIT_LOGGER.lock().unwrap() = Some(logger);
    Ok(())
}

/// Log an audit event using the global logger
pub fn audit_log(
    event: AuditEventType,
    severity: AuditSeverity,
    resource: &str,
    description: &str,
) {
    if let Ok(mut guard) = AUDIT_LOGGER.lock()
        && let Some(logger) = guard.as_mut()
    {
        let _ = logger.log(event, severity, resource, description);
    }
}

/// Log an audit event with metadata using the global logger
pub fn audit_log_with_metadata(
    event: AuditEventType,
    severity: AuditSeverity,
    resource: &str,
    description: &str,
    metadata: serde_json::Value,
) {
    if let Ok(mut guard) = AUDIT_LOGGER.lock()
        && let Some(logger) = guard.as_mut()
    {
        let _ = logger.log_with_metadata(event, severity, resource, description, Some(metadata));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_hash() {
        let entry = AuditEntry {
            id: "test-id".to_string(),
            timestamp: "2026-01-16T00:00:00Z".to_string(),
            event_type: AuditEventType::PackageInstall,
            severity: AuditSeverity::Info,
            user: "test".to_string(),
            resource: "firefox".to_string(),
            description: "Installed firefox".to_string(),
            metadata: None,
            prev_hash: "genesis".to_string(),
            hash: None,
        };

        let hash = entry.compute_hash();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA-256 hex
    }

    #[test]
    fn test_audit_entry_verify() {
        let mut entry = AuditEntry {
            id: "test-id".to_string(),
            timestamp: "2026-01-16T00:00:00Z".to_string(),
            event_type: AuditEventType::PackageInstall,
            severity: AuditSeverity::Info,
            user: "test".to_string(),
            resource: "firefox".to_string(),
            description: "Installed firefox".to_string(),
            metadata: None,
            prev_hash: "genesis".to_string(),
            hash: None,
        };

        entry.hash = Some(entry.compute_hash());
        assert!(entry.verify());

        // Tamper with the entry
        entry.description = "Tampered".to_string();
        assert!(!entry.verify());
    }
}
