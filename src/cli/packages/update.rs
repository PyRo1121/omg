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

pub async fn update_fast() -> Result<()> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return debian::update_fast().await;
    }

    #[cfg(feature = "arch")]
    return arch::update_fast().await;

    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    return debian::update_fast().await;

    #[cfg(all(
        not(feature = "arch"),
        not(any(feature = "debian", feature = "debian-pure"))
    ))]
    generic::update_fast().await
}

pub async fn update_turbo() -> Result<()> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return debian::update_turbo().await;
    }

    #[cfg(feature = "arch")]
    return arch::update_turbo().await;

    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    return debian::update_turbo().await;

    #[cfg(all(
        not(feature = "arch"),
        not(any(feature = "debian", feature = "debian-pure"))
    ))]
    generic::update_turbo().await
}

pub async fn update(check_only: bool, yes: bool, dry_run: bool) -> Result<()> {
    #[cfg(any(feature = "debian", feature = "debian-pure"))]
    if crate::core::env::distro::is_debian_like() {
        return debian::update(check_only, yes, dry_run).await;
    }

    #[cfg(feature = "arch")]
    return arch::update(check_only, yes, dry_run).await;

    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    return debian::update(check_only, yes, dry_run).await;

    #[cfg(all(
        not(feature = "arch"),
        not(any(feature = "debian", feature = "debian-pure"))
    ))]
    generic::update(check_only, yes, dry_run).await
}
