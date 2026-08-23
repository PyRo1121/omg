//! Database sync functionality for packages

use crate::package_managers::get_package_manager;
use anyhow::{Context, Result};

/// Synchronize package databases via the active system package manager
pub async fn sync_databases() -> Result<()> {
    let pm = get_package_manager()?;
    pm.sync().await?;

    // A running daemon owns an immutable package-index snapshot. Refresh it
    // after the package databases commit so subsequent searches cannot serve
    // stale package metadata. Daemon absence is normal; a connected daemon
    // that rejects the refresh is a consistency failure and is reported.
    #[cfg(unix)]
    match crate::core::client::DaemonClient::connect().await {
        Ok(mut client) => {
            let packages = client
                .refresh_index()
                .await
                .context("Package databases synced, but daemon index refresh failed")?;
            tracing::debug!(packages, "Daemon package index refreshed after sync");
        }
        Err(error) => {
            tracing::debug!("Daemon unavailable after package database sync: {error}");
        }
    }

    Ok(())
}
