//! Shim generator for optional IDE compatibility

use anyhow::Result;
use std::path::Path;

use crate::config::Settings;
use crate::core::Runtime;

/// Generate shims for all runtimes
///
/// This creates small executable scripts in ~/.omg/shims/ that delegate
/// to the correct version based on configuration.
pub fn generate_shims(settings: &Settings) -> Result<()> {
    if !settings.shims_enabled {
        tracing::info!("Shims disabled - using PATH modification (faster)");
        return Ok(());
    }

    let shims_dir = settings.shims_dir();
    std::fs::create_dir_all(&shims_dir)?;

    for runtime in Runtime::all() {
        for binary in runtime.binaries() {
            generate_shim(&shims_dir, binary)?;
        }
    }

    tracing::info!("Generated shims in {:?}", shims_dir);
    Ok(())
}

/// Generate a single shim script
fn generate_shim(shims_dir: &Path, binary: &str) -> Result<()> {
    let shim_path = shims_dir.join(binary);

    // Create a simple shell script shim
    // In production, this would be a native binary for speed
    let shim_content = format!(
        r#"#!/bin/sh
# OMG Shim for {binary}
# This shim delegates to the correct version based on configuration

exec omg exec {binary} "$@"
"#
    );

    std::fs::write(&shim_path, shim_content)?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&shim_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&shim_path, perms)?;
    }

    Ok(())
}
