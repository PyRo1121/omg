//! Install functionality for packages

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
        return install_dry_run(packages);
    }

    #[cfg(feature = "arch")]
    {
        return arch::install(packages, yes).await;
    }

    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    {
        let _ = yes;
        return debian::install(packages).await;
    }

    #[cfg(all(
        not(feature = "arch"),
        not(any(feature = "debian", feature = "debian-pure"))
    ))]
    {
        let _ = yes;
        return generic::install(packages).await;
    }

    #[allow(unreachable_code)]
    Ok(())
}

pub fn install_dry_run_cli(packages: &[String]) -> Result<bool> {
    if packages.is_empty() {
        return Ok(false);
    }
    install_dry_run(packages)?;
    Ok(true)
}

#[allow(clippy::unnecessary_wraps)]
fn install_dry_run(packages: &[String]) -> Result<()> {
    #[cfg(feature = "arch")]
    {
        return arch::install_dry_run(packages);
    }

    #[cfg(all(
        not(feature = "arch"),
        any(feature = "debian", feature = "debian-pure")
    ))]
    {
        return debian::install_dry_run(packages);
    }

    #[cfg(all(
        not(feature = "arch"),
        not(any(feature = "debian", feature = "debian-pure"))
    ))]
    {
        return generic::install_dry_run(packages);
    }

    #[allow(unreachable_code)]
    Ok(())
}
