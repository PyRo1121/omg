//! Database sync functionality for packages

use anyhow::Result;

use super::common::use_debian_backend;

#[cfg(feature = "arch")]
use crate::package_managers::sync_databases_parallel;

#[cfg(feature = "debian")]
use crate::package_managers::AptPackageManager;

/// Sync package databases from mirrors (parallel, fast)
pub async fn sync_databases() -> Result<()> {
    if use_debian_backend() {
        #[cfg(feature = "debian")]
        {
            let apt = AptPackageManager::new();
            apt.sync_databases().await
        }
        #[cfg(not(feature = "debian"))]
        Ok(())
    } else {
        #[cfg(feature = "arch")]
        {
            sync_databases_parallel().await
        }
        #[cfg(not(feature = "arch"))]
        Ok(())
    }
}
