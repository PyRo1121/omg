//! Security audit command implementations
//!
//! Provides CLI handlers for vulnerability scanning, SBOM generation, secret detection,
//! license compliance, SLSA verification, and audit log management.

use anyhow::{Context, Result};
use owo_colors::OwoColorize;

fn write_private_export(path: &std::path::Path, contents: impl AsRef<[u8]>) -> Result<()> {
    crate::core::safe_ops::atomic_write_file_sync(path, contents)
        .with_context(|| format!("Failed to write security export to {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).with_context(
            || format!("Failed to restrict security export permissions on {}", path.display()),
        )?;
    }
    Ok(())
}

fn read_audit_entries(
    logger: &AuditLogger,
    limit: usize,
    severity_filter: Option<&str>,
) -> Result<Vec<crate::core::security::audit::AuditEntry>> {
    let result = if let Some(sev) = severity_filter {
        let min_severity = match sev.to_lowercase().as_str() {
            "debug" => AuditSeverity::Debug,
            "info" => AuditSeverity::Info,
            "warning" | "warn" => AuditSeverity::Warning,
            "error" => AuditSeverity::Error,
            "critical" => AuditSeverity::Critical,
            _ => anyhow::bail!("Invalid severity: {sev}"),
        };
        logger.filter_by_severity(min_severity).map(|mut entries| {
            entries.reverse();
            entries
        })
    } else {
        logger.get_recent(limit)
    };
    match result {
        Ok(entries) => Ok(entries.into_iter().take(limit).collect()),
        Err(error) if error.is_not_found() => Ok(Vec::new()),
        Err(error) => Err(error).context("Failed to read audit log entries"),
    }
}
