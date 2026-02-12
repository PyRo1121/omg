//! Update functionality for packages

use anyhow::Result;

#[cfg(feature = "arch")]
mod arch;
#[cfg(any(feature = "debian", feature = "debian-pure"))]
mod debian;
#[cfg(all(
    not(feature = "arch"),
    not(any(feature = "debian", feature = "debian-pure"))
))]
mod generic;

#[cfg(feature = "arch")]
pub async fn update_fast() -> Result<()> {
    arch::update_fast().await
}

#[cfg(all(
    not(feature = "arch"),
    any(feature = "debian", feature = "debian-pure")
))]
pub async fn update_fast() -> Result<()> {
    debian::update_fast().await
}

#[cfg(all(
    not(feature = "arch"),
    not(any(feature = "debian", feature = "debian-pure"))
))]
pub async fn update_fast() -> Result<()> {
    generic::update_fast().await
}

#[cfg(feature = "arch")]
pub async fn update_turbo() -> Result<()> {
    arch::update_turbo().await
}

#[cfg(all(
    not(feature = "arch"),
    any(feature = "debian", feature = "debian-pure")
))]
pub async fn update_turbo() -> Result<()> {
    debian::update_turbo().await
}

#[cfg(all(
    not(feature = "arch"),
    not(any(feature = "debian", feature = "debian-pure"))
))]
pub async fn update_turbo() -> Result<()> {
    generic::update_turbo().await
}

pub async fn update(check_only: bool, yes: bool, dry_run: bool) -> Result<()> {
    #[cfg(feature = "arch")]
    {
        return arch::update(check_only, yes, dry_run).await;
    }
    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    {
        return debian::update(check_only, yes, dry_run).await;
    }
    #[cfg(all(
        not(feature = "arch"),
        not(any(feature = "debian", feature = "debian-pure"))
    ))]
    {
        return generic::update(check_only, yes, dry_run).await;
    }

    #[allow(unreachable_code)]
    Ok(())
}
