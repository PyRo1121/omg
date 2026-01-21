//! Database sync functionality for packages

use anyhow::Result;
use crate::package_managers::get_package_manager;

/// Sync package databases from mirrors (parallel, fast)
pub async fn sync_databases() -> Result<()> {
    let pm = get_package_manager();
    pm.sync().await
}
