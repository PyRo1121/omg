//! Database sync functionality for packages

use crate::package_managers::get_package_manager;
use anyhow::Result;

/// Synchronize package databases via the active system package manager
pub async fn sync_databases() -> Result<()> {
    let pm = get_package_manager()?;
    pm.sync().await
}
