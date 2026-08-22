//! Generic (no dedicated backend) updates.
//!
//! Shares the official-repository-only update implementation with the Debian
//! backend; the generic package manager underneath is the only difference.

use anyhow::Result;

use crate::cli::packages::common;

pub async fn update_fast() -> Result<()> {
    common::update_official_only(false, true, false).await
}

pub async fn update_turbo() -> Result<()> {
    common::update_official_only(false, true, false).await
}

pub async fn update(check_only: bool, yes: bool, dry_run: bool) -> Result<()> {
    common::update_official_only(check_only, yes, dry_run).await
}
