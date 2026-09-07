//! Update functionality for packages

use anyhow::Result;

use super::dispatch_backend;

#[cfg(feature = "arch")]
mod arch;

pub async fn update_fast() -> Result<()> {
    dispatch_backend! {
        debian: { super::common::update_official_only(false, true, false, false).await },
        arch: { arch::update_fast().await },
        generic: { super::common::update_official_only(false, true, false, false).await },
    }
}

pub async fn update_turbo() -> Result<()> {
    dispatch_backend! {
        debian: { super::common::update_official_only(false, true, false, false).await },
        arch: { arch::update_turbo().await },
        generic: { super::common::update_official_only(false, true, false, false).await },
    }
}

pub async fn update(check_only: bool, yes: bool, dry_run: bool, no_sync: bool) -> Result<()> {
    dispatch_backend! {
        debian: { super::common::update_official_only(check_only, yes, dry_run, no_sync).await },
        arch: { arch::update(check_only, yes, dry_run, no_sync).await },
        generic: { super::common::update_official_only(check_only, yes, dry_run, no_sync).await },
    }
}
