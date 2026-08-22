//! Debian backend updates.
//!
//! The Debian flow has no AUR counterpart, so it shares the official-repository
//! update implementation with the generic backend; only the compiled package
//! manager underneath differs.

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
