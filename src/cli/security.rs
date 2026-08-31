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
