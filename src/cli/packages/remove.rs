//! Remove functionality for packages

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

pub async fn remove(packages: &[String], recursive: bool, _yes: bool, dry_run: bool) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    for pkg in packages {
        if let Err(e) = crate::core::security::validate_package_name(pkg) {
            anyhow::bail!("Invalid package name '{pkg}': {e}");
        }
    }

    if dry_run {
        return remove_dry_run(packages, recursive);
    }

    dispatch_backend! {
        debian: { debian::remove(packages, recursive).await },
        arch: { arch::remove(packages, recursive).await },
        generic: { generic::remove(packages, recursive).await },
    }
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "backend feature dispatch may select fallible implementations"
)]
fn remove_dry_run(packages: &[String], recursive: bool) -> Result<()> {
    dispatch_backend! {
        debian: { debian::remove_dry_run(packages, recursive) },
        arch: { arch::remove_dry_run(packages, recursive) },
        generic: { generic::remove_dry_run(packages, recursive) },
    }
}
