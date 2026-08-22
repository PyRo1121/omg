//! Tamper-proof audit logging with cryptographic integrity verification
//!
//! Provides append-only audit logs with SHA-256 chain verification to detect
//! tampering, log rotation, and compliance-ready event tracking.

use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::core::paths;

/// Failures creating, reading, or appending the integrity-bound audit log.
#[derive(Debug, Error)]
pub enum AuditError {
    #[error("Failed to create audit directory '{path}'")]
    CreateDir {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Failed to open audit log '{path}'")]
    Open {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Failed to read audit log '{path}'")]
    Read {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Failed to write audit log '{path}'")]
    Write {
        path: String,
        #[source]
        source: io::Error,
    },
    #[error("Failed to serialize audit entry")]
    Serialize {
        #[source]
        source: serde_json::Error,
    },
    #[error("Corrupt audit log '{path}' at line {line}")]
    CorruptLine {
        path: String,
        line: usize,
        #[source]
        source: serde_json::Error,
    },
    #[error("Corrupt audit log '{path}' at line {line}: missing hash")]
    MissingHash { path: String, line: usize },
    #[error("Failed to unlock audit log '{path}'")]
    Unlock {
        path: String,
        #[source]
        source: io::Error,
    },
}

impl AuditError {
    /// True when the underlying IO failure is a missing path.
    pub fn is_not_found(&self) -> bool {
        match self {
            Self::CreateDir { source, .. }
            | Self::Open { source, .. }
            | Self::Read { source, .. }
            | Self::Write { source, .. } => source.kind() == io::ErrorKind::NotFound,
            Self::Serialize { .. }
            | Self::CorruptLine { .. }
            | Self::MissingHash { .. }
            | Self::Unlock { .. } => false,
        }
    }
}

/// Audit event types for security logging
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
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

impl std::fmt::Display for AuditEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PackageInstall => write!(f, "PACKAGE_INSTALL"),
            Self::PackageRemove => write!(f, "PACKAGE_REMOVE"),
            Self::PackageUpgrade => write!(f, "PACKAGE_UPGRADE"),
            Self::PackageDowngrade => write!(f, "PACKAGE_DOWNGRADE"),
            Self::SecurityAudit => write!(f, "SECURITY_AUDIT"),
            Self::VulnerabilityDetected => write!(f, "VULNERABILITY_DETECTED"),
            Self::SignatureVerified => write!(f, "SIGNATURE_VERIFIED"),
            Self::SignatureFailed => write!(f, "SIGNATURE_FAILED"),
            Self::PolicyViolation => write!(f, "POLICY_VIOLATION"),
            Self::PolicyChanged => write!(f, "POLICY_CHANGED"),
            Self::ConfigChanged => write!(f, "CONFIG_CHANGED"),
            Self::DaemonStarted => write!(f, "DAEMON_STARTED"),
            Self::DaemonStopped => write!(f, "DAEMON_STOPPED"),
            Self::SbomGenerated => write!(f, "SBOM_GENERATED"),
            Self::SbomExported => write!(f, "SBOM_EXPORTED"),
        }
    }
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
    #[must_use]
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
        hex::encode(hasher.finalize())
    }

    /// Verify the integrity of this entry
    #[must_use]
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
    /// Diagnostic mirror of the on-disk tail hash captured at creation and
    /// after each successful append. `log_locked` always re-reads the
    /// authoritative tail under the lock; this field exists so callers and
    /// tests can assert that a failed append never advanced the chain.
    last_hash: String,
}

impl AuditLogger {
    /// Create a new audit logger writing to `<data dir>/audit/audit.jsonl`.
    pub fn new() -> Result<Self, AuditError> {
        Self::new_in(paths::data_dir().join("audit/audit.jsonl"))
    }

    /// Create an audit logger at an explicit path.
    pub fn new_in(log_path: impl AsRef<Path>) -> Result<Self, AuditError> {
        let log_path = log_path.as_ref().to_path_buf();
        let log_dir = log_path.parent().ok_or_else(|| AuditError::CreateDir {
            path: log_path.display().to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "audit log path must have a parent directory",
            ),
        })?;
        std::fs::create_dir_all(log_dir).map_err(|source| AuditError::CreateDir {
            path: log_dir.display().to_string(),
            source,
        })?;

        let last_hash = get_last_hash(&log_path)?;
        Ok(Self {
            log_path,
            last_hash,
        })
    }

    /// Log an audit event
    pub fn log(
        &mut self,
        event: AuditEventType,
        severity: AuditSeverity,
        resource: &str,
        description: &str,
    ) -> Result<(), AuditError> {
        self.log_with_metadata(event, severity, resource, description, None)
    }

    /// Log an audit event with additional metadata
    ///
    /// Appends are serialized across processes with a lock file (the same
    /// pattern as [`crate::core::history::HistoryManager`]) and the previous
    /// entry hash is re-read inside the critical section, so concurrent CLI
    /// and daemon writers cannot fork the integrity chain.
    pub fn log_with_metadata(
        &mut self,
        event: AuditEventType,
        severity: AuditSeverity,
        resource: &str,
        description: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<(), AuditError> {
        let lock_path = self.log_path.with_extension("lock");
        let lock = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&lock_path)
            .map_err(|source| AuditError::Open {
                path: lock_path.display().to_string(),
                source,
            })?;
        lock.lock().map_err(|source| AuditError::Open {
            path: lock_path.display().to_string(),
            source,
        })?;

        let result = self.log_locked(event, severity, resource, description, metadata);
        if let Err(source) = lock.unlock() {
            return match result {
                Ok(()) => Err(AuditError::Unlock {
                    path: self.log_path.display().to_string(),
                    source,
                }),
                Err(operation_error) => Err(operation_error),
            };
        }
        result
    }

    fn log_locked(
        &mut self,
        event: AuditEventType,
        severity: AuditSeverity,
        resource: &str,
        description: &str,
        metadata: Option<serde_json::Value>,
    ) -> Result<(), AuditError> {
        // Re-read the on-disk tail hash while holding the lock so entries
        // written by another process since this logger was created chain
        // correctly instead of sharing our stale prev_hash.
        let prev_hash = get_last_hash(&self.log_path)?;
        let timestamp = jiff::Zoned::now()
            .strftime("%Y-%m-%dT%H:%M:%S%.3fZ")
            .to_string();
        let id = uuid::Uuid::new_v4().to_string();
        let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());
        let path_str = self.log_path.display().to_string();

        let mut entry = AuditEntry {
            id,
            timestamp,
            event_type: event,
            severity,
            user,
            resource: resource.to_string(),
            description: description.to_string(),
            metadata,
            prev_hash,
            hash: None,
        };

        // Compute the hash, but do not advance the in-memory chain until the
        // entry is durably on disk. Otherwise a failed write leaves the next
        // event pointing at a hash that never landed.
        let hash = entry.compute_hash();
        entry.hash = Some(hash);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_path)
            .map_err(|source| AuditError::Open {
                path: path_str.clone(),
                source,
            })?;

        let json =
            serde_json::to_string(&entry).map_err(|source| AuditError::Serialize { source })?;
        writeln!(file, "{json}").map_err(|source| AuditError::Write {
            path: path_str.clone(),
            source,
        })?;
        file.flush().map_err(|source| AuditError::Write {
            path: path_str.clone(),
            source,
        })?;
        file.sync_all().map_err(|source| AuditError::Write {
            path: path_str,
            source,
        })?;
        self.last_hash = entry.hash.clone().unwrap_or_default();

        match severity {
            AuditSeverity::Debug => {
                tracing::debug!(target: "audit", "{}: {}", entry.event_type, description);
            }
            AuditSeverity::Info => {
                tracing::info!(target: "audit", "{}: {}", entry.event_type, description);
            }
            AuditSeverity::Warning => {
                tracing::warn!(target: "audit", "{}: {}", entry.event_type, description);
            }
            AuditSeverity::Error => {
                tracing::error!(target: "audit", "{}: {}", entry.event_type, description);
            }
            AuditSeverity::Critical => {
                tracing::error!(target: "audit", "CRITICAL - {}: {}", entry.event_type, description);
            }
        }

        Ok(())
    }

    /// Verify the integrity of the entire audit log
    pub fn verify_integrity(&self) -> Result<AuditIntegrityReport, AuditError> {
        let path_str = self.log_path.display().to_string();
        let file = File::open(&self.log_path).map_err(|source| AuditError::Open {
            path: path_str.clone(),
            source,
        })?;
        let reader = BufReader::new(file);

        let mut total_entries = 0;
        let mut valid_entries = 0;
        let mut chain_valid = true;
        let mut expected_prev_hash = "genesis".to_string();
        let mut first_invalid: Option<String> = None;

        for line in reader.lines() {
            let line = line.map_err(|source| AuditError::Read {
                path: path_str.clone(),
                source,
            })?;
            total_entries += 1;

            if let Ok(entry) = serde_json::from_str::<AuditEntry>(&line) {
                if entry.verify() {
                    valid_entries += 1;
                } else if first_invalid.is_none() {
                    first_invalid = Some(entry.id.clone());
                }

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

    /// Get recent audit entries. A missing log is empty; a corrupt line is an error.
    pub fn get_recent(&self, limit: usize) -> Result<Vec<AuditEntry>, AuditError> {
        let mut entries = read_all_entries(&self.log_path)?;
        entries.reverse();
        entries.truncate(limit);
        Ok(entries)
    }

    /// Filter entries by event type
    pub fn filter_by_type(
        &self,
        event_type: &AuditEventType,
    ) -> Result<Vec<AuditEntry>, AuditError> {
        let entries = read_all_entries(&self.log_path)?;
        Ok(entries
            .into_iter()
            .filter(|e| &e.event_type == event_type)
            .collect())
    }

    /// Filter entries by severity (and above)
    pub fn filter_by_severity(
        &self,
        min_severity: AuditSeverity,
    ) -> Result<Vec<AuditEntry>, AuditError> {
        let entries = read_all_entries(&self.log_path)?;
        Ok(entries
            .into_iter()
            .filter(|e| e.severity >= min_severity)
            .collect())
    }
}

/// Read every JSONL entry. Missing files are empty; IO and parse failures are errors.
fn read_all_entries(path: &Path) -> Result<Vec<AuditEntry>, AuditError> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let path_str = path.display().to_string();
    let file = File::open(path).map_err(|source| AuditError::Open {
        path: path_str.clone(),
        source,
    })?;
    let mut entries = Vec::new();
    for (index, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(|source| AuditError::Read {
            path: path_str.clone(),
            source,
        })?;
        if line.is_empty() {
            continue;
        }
        let entry = serde_json::from_str::<AuditEntry>(&line).map_err(|source| {
            AuditError::CorruptLine {
                path: path_str.clone(),
                line: index + 1,
                source,
            }
        })?;
        entries.push(entry);
    }
    Ok(entries)
}

fn get_last_hash(path: &Path) -> Result<String, AuditError> {
    let entries = read_all_entries(path)?;
    match entries.last() {
        None => Ok("genesis".to_string()),
        Some(entry) => entry.hash.clone().ok_or_else(|| AuditError::MissingHash {
            path: path.display().to_string(),
            line: entries.len(),
        }),
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
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.chain_valid && self.total_entries == self.valid_entries
    }
}

/// Global audit logger instance
static AUDIT_LOGGER: std::sync::LazyLock<std::sync::Mutex<Option<AuditLogger>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// Initialize the global audit logger eagerly.
///
/// Optional: [`audit_log`] lazily self-initializes the same global on first
/// use, so callers that never ran this function still persist events.
pub fn init_audit_logger() -> Result<(), AuditError> {
    let logger = AuditLogger::new()?;
    *AUDIT_LOGGER.lock().expect("audit logger mutex poisoned") = Some(logger);
    Ok(())
}

/// Persist one event through the global audit logger.
///
/// The global is lazily created on first use: daemon code paths call this
/// without a prior `init_audit_logger`, and security events must reach the
/// tamper-evident log rather than silently degrade to `tracing` output.
fn record_global(
    event: AuditEventType,
    severity: AuditSeverity,
    resource: &str,
    description: &str,
    metadata: Option<serde_json::Value>,
) {
    let mut guard = AUDIT_LOGGER.lock().expect("audit logger mutex poisoned");
    if guard.is_none() {
        match AuditLogger::new() {
            Ok(logger) => *guard = Some(logger),
            Err(error) => {
                tracing::warn!(
                    "Audit logger unavailable, dropping event {event} for {resource}: {error}"
                );
                return;
            }
        }
    }
    let Some(logger) = guard.as_mut() else {
        return;
    };
    let result = match metadata {
        Some(metadata) => {
            logger.log_with_metadata(event, severity, resource, description, Some(metadata))
        }
        None => logger.log(event, severity, resource, description),
    };
    if let Err(error) = result {
        tracing::warn!("Failed to persist audit event {event} for {resource}: {error}");
    }
}

/// Log an audit event using the global logger
pub fn audit_log(
    event: AuditEventType,
    severity: AuditSeverity,
    resource: &str,
    description: &str,
) {
    record_global(event, severity, resource, description, None);
}

/// Log an audit event with metadata using the global logger
pub fn audit_log_with_metadata(
    event: AuditEventType,
    severity: AuditSeverity,
    resource: &str,
    description: &str,
    metadata: serde_json::Value,
) {
    record_global(event, severity, resource, description, Some(metadata));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_audit_write_does_not_advance_the_chain() {
        let temp = tempfile::TempDir::new().unwrap();
        let log_path = temp.path().join("audit.jsonl");
        let mut logger = AuditLogger::new_in(log_path.clone()).unwrap();

        logger
            .log(
                AuditEventType::PackageInstall,
                AuditSeverity::Info,
                "firefox",
                "Installed firefox",
            )
            .unwrap();
        let hash_after_first = logger.last_hash.clone();
        assert_ne!(hash_after_first, "genesis");

        std::fs::remove_file(&log_path).unwrap();
        std::fs::create_dir(&log_path).unwrap();

        let error = logger
            .log(
                AuditEventType::PackageRemove,
                AuditSeverity::Info,
                "firefox",
                "Removed firefox",
            )
            .expect_err("appending over a directory must fail");
        // The failure surfaces either at the locked tail re-read (Read) or
        // at the append open (Open); both must abort before the chain moves.
        assert!(
            matches!(error, AuditError::Open { .. } | AuditError::Read { .. }),
            "got: {error}"
        );
        assert_eq!(logger.last_hash, hash_after_first);
    }

    fn expect_audit_err<T>(result: Result<T, AuditError>, what: &str) -> AuditError {
        match result {
            Ok(_) => panic!("{what}"),
            Err(err) => err,
        }
    }

    #[test]
    fn corrupt_line_is_not_used_as_chain_head() {
        let temp = tempfile::TempDir::new().unwrap();
        let log_path = temp.path().join("audit.jsonl");
        std::fs::write(&log_path, "not-json\n").unwrap();
        let err = expect_audit_err(
            AuditLogger::new_in(log_path),
            "corrupt log must not initialize a chain",
        );
        assert!(
            matches!(err, AuditError::CorruptLine { line: 1, .. }),
            "got: {err}"
        );
    }

    #[test]
    fn missing_hash_is_not_used_as_chain_head() {
        let temp = tempfile::TempDir::new().unwrap();
        let log_path = temp.path().join("audit.jsonl");
        std::fs::write(
            &log_path,
            r#"{"id":"x","timestamp":"2026-01-16T00:00:00Z","event_type":"package_install","severity":"info","user":"test","resource":"firefox","description":"Installed firefox","prev_hash":"genesis"}
"#,
        )
        .unwrap();
        let err = expect_audit_err(
            AuditLogger::new_in(log_path),
            "entry without a hash must not initialize a chain",
        );
        assert!(
            matches!(err, AuditError::MissingHash { line: 1, .. }),
            "got: {err}"
        );
    }

    #[test]
    fn get_recent_rejects_corrupt_lines() {
        let temp = tempfile::TempDir::new().unwrap();
        let log_path = temp.path().join("audit.jsonl");
        let mut logger = AuditLogger::new_in(log_path.clone()).unwrap();
        logger
            .log(
                AuditEventType::PackageInstall,
                AuditSeverity::Info,
                "firefox",
                "Installed firefox",
            )
            .unwrap();
        std::fs::write(&log_path, "not-json\n").unwrap();
        let err = logger
            .get_recent(10)
            .expect_err("viewing a corrupt log must fail closed");
        assert!(
            matches!(err, AuditError::CorruptLine { line: 1, .. }),
            "got: {err}"
        );
    }

    #[test]
    fn verify_integrity_reports_corrupt_json() {
        let temp = tempfile::TempDir::new().unwrap();
        let log_path = temp.path().join("audit.jsonl");
        std::fs::write(&log_path, "not-json\n").unwrap();
        let logger = AuditLogger {
            log_path,
            last_hash: "genesis".to_string(),
        };
        let report = logger.verify_integrity().unwrap();
        assert!(!report.is_valid());
        assert_eq!(report.total_entries, 1);
        assert_eq!(report.valid_entries, 0);
        assert!(!report.chain_valid);
    }

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

    #[test]
    fn concurrent_writers_keep_the_integrity_chain_valid() {
        // Regression: each writer used to cache the tail hash at logger
        // creation, so concurrent CLI + daemon writers forked the chain.
        const WRITERS: usize = 8;
        const EVENTS_PER_WRITER: usize = 4;
        let temp = tempfile::TempDir::new().unwrap();
        let log_path = temp.path().join("audit.jsonl");

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(WRITERS));
        let mut writers = Vec::new();
        for writer_index in 0..WRITERS {
            let log_path = log_path.clone();
            let barrier = std::sync::Arc::clone(&barrier);
            writers.push(std::thread::spawn(move || {
                let mut logger = AuditLogger::new_in(log_path).unwrap();
                barrier.wait();
                for event_index in 0..EVENTS_PER_WRITER {
                    logger
                        .log(
                            AuditEventType::PackageInstall,
                            AuditSeverity::Info,
                            "firefox",
                            &format!("concurrent writer {writer_index} event {event_index}"),
                        )
                        .unwrap();
                }
            }));
        }
        for writer in writers {
            writer.join().expect("audit writer panicked");
        }

        let report = AuditLogger::new_in(log_path)
            .unwrap()
            .verify_integrity()
            .unwrap();
        assert_eq!(report.total_entries, WRITERS * EVENTS_PER_WRITER);
        assert!(
            report.is_valid(),
            "chain broke under concurrency: {report:?}"
        );
    }

    #[test]
    #[serial_test::serial]
    fn audit_log_lazy_initializes_and_persists_events() {
        // Regression: production daemons called `audit_log` without ever
        // running `init_audit_logger`, so every security event degraded to a
        // tracing warning and the tamper-evident log stayed empty.
        let temp = tempfile::TempDir::new().unwrap();
        // SAFETY: test-only environment override, serialized via #[serial]
        #[expect(unsafe_code)]
        unsafe {
            std::env::set_var("OMG_DATA_DIR", temp.path());
        }

        audit_log(
            AuditEventType::PolicyViolation,
            AuditSeverity::Warning,
            "daemon_handler",
            "lazy-init regression event",
        );

        // SAFETY: test cleanup, serialized
        #[expect(unsafe_code)]
        unsafe {
            std::env::remove_var("OMG_DATA_DIR");
        }

        let log_path = temp.path().join("audit/audit.jsonl");
        let report = AuditLogger::new_in(&log_path)
            .expect("lazy audit_log must have created the audit log")
            .verify_integrity()
            .unwrap();
        assert_eq!(report.total_entries, 1, "event must be persisted");
        assert!(report.is_valid(), "persisted event must chain: {report:?}");
        let entries = AuditLogger::new_in(&log_path)
            .unwrap()
            .get_recent(10)
            .unwrap();
        assert_eq!(entries[0].description, "lazy-init regression event");
        assert_eq!(entries[0].event_type, AuditEventType::PolicyViolation);
    }
}
