//! Install functionality for packages

use anyhow::Result;

use super::dispatch_backend;

/// Maximum number of user-selected AUR replacement hops before aborting.
/// Each accepted suggestion re-enters the install flow for one package; the
/// bound turns a pathological suggestion chain into a clean error instead of
/// an unbounded interactive loop.
pub(crate) const MAX_REPLACEMENT_HOPS: u32 = 3;

pub(crate) async fn enforce_install_policy(
    policy: &crate::core::security::SecurityPolicy,
    scanner: &dyn crate::core::security::vulnerability::VulnerabilitySource,
    name: &str,
    version: &crate::package_managers::types::Version,
    is_community_source: bool,
    license: Option<&str>,
) -> Result<()> {
    if crate::core::paths::test_mode() {
        return policy
            .check_source(name, is_community_source, license)
            .map_err(Into::into);
    }

    let grade = match policy
        .assign_grade(scanner, name, version, !is_community_source)
        .await
    {
        Ok(grade) => grade,
        Err(error)
            if !crate::core::paths::config_dir()
                .join("policy.toml")
                .is_file() =>
        {
            // The built-in defaults remain usable when a platform has no OSV
            // ecosystem or the evidence service is temporarily unavailable.
            // An explicit policy file is different: its control must fail
            // closed rather than silently degrading.
            tracing::warn!("Vulnerability grading unavailable for {name}: {error}");
            return policy
                .check_source(name, is_community_source, license)
                .map_err(Into::into);
        }
        Err(error) => return Err(error.into()),
    };
    policy
        .check_package(name, is_community_source, license, grade)
        .map_err(Into::into)
}

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
/// * `allow_local_file` - Explicit consent for privileged local archive installation
pub async fn install(
    packages: &[String],
    yes: bool,
    dry_run: bool,
    allow_local_file: bool,
) -> Result<()> {
    install_with_replacement_budget(
        packages,
        yes,
        dry_run,
        allow_local_file,
        MAX_REPLACEMENT_HOPS,
    )
    .await
}

/// Entry point with an explicit interactive-replacement budget.
///
/// Each user-accepted suggestion re-enters the install flow for one package;
/// the budget turns a pathological suggestion chain into a clean error.
pub(crate) async fn install_with_replacement_budget(
    packages: &[String],
    yes: bool,
    dry_run: bool,
    allow_local_file: bool,
    replacement_hops: u32,
) -> Result<()> {
    if packages.is_empty() {
        anyhow::bail!("No packages specified");
    }

    let includes_local_file = packages.iter().any(|package| {
        if crate::core::security::is_local_package_file(package) {
            return true;
        }
        #[cfg(any(feature = "debian", feature = "debian-pure"))]
        if crate::core::env::distro::is_debian_like()
            && crate::core::security::is_local_debian_package_file(package)
        {
            return true;
        }
        false
    });
    anyhow::ensure!(
        !includes_local_file || allow_local_file,
        "Local package archives require explicit consent: pass --allow-local-file after reviewing the archive source"
    );

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
