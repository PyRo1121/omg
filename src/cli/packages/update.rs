//! Update functionality for packages

use anyhow::Result;

use super::dispatch_backend;

#[cfg(feature = "arch")]
mod arch;
#[cfg(any(feature = "debian", feature = "debian-pure"))]
mod debian;
#[cfg(all(
    not(feature = "arch"),
    not(any(feature = "debian", feature = "debian-pure"))
))]
mod generic;

pub async fn update_fast() -> Result<()> {
    dispatch_backend! {
        debian: { debian::update_fast().await },
        arch: { arch::update_fast().await },
        generic: { generic::update_fast().await },
    }
}

pub async fn update_turbo() -> Result<()> {
    dispatch_backend! {
        debian: { debian::update_turbo().await },
        arch: { arch::update_turbo().await },
        generic: { generic::update_turbo().await },
    }
}

pub async fn update(check_only: bool, yes: bool, dry_run: bool) -> Result<()> {
    dispatch_backend! {
        debian: { debian::update(check_only, yes, dry_run).await },
        arch: { arch::update(check_only, yes, dry_run).await },
        generic: { generic::update(check_only, yes, dry_run).await },
    }
}
