//! Install functionality for packages

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

/// Install packages from repositories or AUR
///
/// # Arguments
/// * `packages` - Package names to install
/// * `yes` - Skip confirmation prompts
/// * `dry_run` - Show what would be installed without actually installing
pub async fn install(packages: &[String], yes: bool, dry_run: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    if dry_run {
        return install_dry_run(packages).await;
    }

    dispatch_backend! {
        debian: { let _ = yes; debian::install(packages).await },
        arch: { arch::install(packages, yes).await },
        generic: { let _ = yes; generic::install(packages).await },
    }
}

pub fn install_dry_run_cli(packages: &[String]) -> Result<bool> {
    crate::core::security::validate_package_names_or_files(packages)?;
    // Source resolution can require an AUR request, so the synchronous startup
    // fast path must defer to the normal asynchronous command implementation.
    Ok(false)
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
