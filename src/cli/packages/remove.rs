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

/// Remove packages.
///
/// # Arguments
/// * `packages` - Package names to remove (each is validated)
/// * `recursive` - *Advisory only.* The Arch backend always cleans unneeded
///   dependencies (libalpm `RECURSE | UNNEEDED`), while the Debian and
///   generic backends never do. The flag is kept for CLI symmetry until
///   per-backend recursion policy is decided; the dry runs state the truth
///   for their backend.
/// * `yes` - Skip the package-removal confirmation
/// * `dry_run` - Preview what would be removed without touching the system
pub async fn remove(packages: &[String], recursive: bool, yes: bool, dry_run: bool) -> Result<()> {
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

    if !super::common::confirm_package_mutation("removal", packages.len(), yes).await? {
        crate::cli::modern_ui::print_warning("Removal cancelled");
        return Ok(());
    }

    super::common::remove_via_service(packages).await
}

#[allow(
    clippy::unnecessary_wraps,
    reason = "backend feature dispatch may select fallible implementations"
)]
#[cfg_attr(
    not(feature = "arch"),
    allow(
        unused_variables,
        reason = "only the Arch dry run states recursion truthfully; other backends never recurse"
    )
)]
fn remove_dry_run(packages: &[String], recursive: bool) -> Result<()> {
    dispatch_backend! {
        debian: { debian::remove_dry_run(packages); Ok(()) },
        arch: { arch::remove_dry_run(packages, recursive) },
        generic: { generic::remove_dry_run(packages) },
    }
}
