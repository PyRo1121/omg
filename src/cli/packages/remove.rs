//! Remove functionality for packages

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

    #[cfg(feature = "arch")]
    {
        return arch::remove(packages, recursive).await;
    }
    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    {
        return debian::remove(packages, recursive).await;
    }
    #[cfg(all(
        not(feature = "arch"),
        not(any(feature = "debian", feature = "debian-pure"))
    ))]
    {
        return generic::remove(packages, recursive).await;
    }

    #[allow(unreachable_code)]
    Ok(())
}

#[allow(clippy::unnecessary_wraps)]
fn remove_dry_run(packages: &[String], recursive: bool) -> Result<()> {
    #[cfg(feature = "arch")]
    {
        return arch::remove_dry_run(packages, recursive);
    }
    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    {
        return debian::remove_dry_run(packages, recursive);
    }
    #[cfg(all(
        not(feature = "arch"),
        not(any(feature = "debian", feature = "debian-pure"))
    ))]
    {
        return generic::remove_dry_run(packages, recursive);
    }

    #[allow(unreachable_code)]
    Ok(())
}
