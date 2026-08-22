//! Install functionality for packages

use anyhow::Result;

use super::dispatch_backend;

/// Maximum number of user-selected AUR replacement hops before aborting.
/// Each accepted suggestion re-enters the install flow for one package; the
/// bound turns a pathological suggestion chain into a clean error instead of
/// an unbounded interactive loop.
pub(crate) const MAX_REPLACEMENT_HOPS: u32 = 3;

#[cfg(feature = "arch")]
mod arch;
#[cfg(any(feature = "debian", feature = "debian-pure"))]
mod debian;
#[cfg(all(
    not(feature = "arch"),
    not(any(feature = "debian", feature = "debian-pure"))
))]
mod generic;

/// Install packages from repositories or AUR
///
/// # Arguments
/// * `packages` - Package names to install
/// * `yes` - Skip confirmation prompts
/// * `dry_run` - Show what would be installed without actually installing
pub async fn install(packages: &[String], yes: bool, dry_run: bool) -> Result<()> {
    install_with_replacement_budget(packages, yes, dry_run, MAX_REPLACEMENT_HOPS).await
}

/// Entry point with an explicit interactive-replacement budget.
///
/// Each user-accepted suggestion re-enters the install flow for one package;
/// the budget turns a pathological suggestion chain into a clean error.
pub(crate) async fn install_with_replacement_budget(
    packages: &[String],
    yes: bool,
    dry_run: bool,
    replacement_hops: u32,
) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    if dry_run {
        return install_dry_run(packages).await;
    }

    dispatch_backend! {
        debian: { let _ = (yes, replacement_hops); debian::install(packages).await },
        arch: { arch::install(packages, yes, replacement_hops).await },
        generic: { let _ = (yes, replacement_hops); generic::install(packages).await },
    }
}

#[cfg(feature = "arch")]
async fn install_dry_run(packages: &[String]) -> Result<()> {
    dispatch_backend! {
        debian: { debian::install_dry_run(packages) },
        arch: { arch::install_dry_run(packages).await },
        generic: { generic::install_dry_run(packages) },
    }
}

#[cfg(all(
    not(feature = "arch"),
    any(feature = "debian", feature = "debian-pure")
))]
fn install_dry_run(packages: &[String]) -> impl std::future::Future<Output = Result<()>> + use<'_> {
    std::future::ready(debian::install_dry_run(packages))
}

#[cfg(not(any(feature = "arch", feature = "debian", feature = "debian-pure")))]
fn install_dry_run(packages: &[String]) -> impl std::future::Future<Output = Result<()>> + use<'_> {
    std::future::ready(generic::install_dry_run(packages))
}
