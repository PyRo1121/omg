//! Pure Rust Debian/Ubuntu package manager backend
//!
//! Uses `debian_db` for ultra-fast searches and info, and spawns `apt`
//! command for transactions. This allows Debian support without C dependencies.

use std::future::Future;
use std::pin::Pin;

use anyhow::{Context, Result};

use crate::core::{Package, PackageSource};
use crate::package_managers::PackageManager;
use crate::package_managers::debian_db;
use crate::package_managers::types::UpdateInfo;

#[derive(Debug, Default)]
pub struct PureDebianPackageManager;

/// Publish authenticated package lists through APT's native trust engine.
async fn update_apt_lists(program: &std::path::Path, needs_elevation: bool) -> Result<()> {
    let program_text = program
        .to_str()
        .context("apt-get executable path is not valid UTF-8")?;
    if needs_elevation {
        crate::core::privilege::run_privileged_program(program_text, &["update"]).await?;
        return Ok(());
    }

    let status = tokio::process::Command::new(program)
        .arg("update")
        .status()
        .await
        .context("Failed to run apt-get update")?;
    anyhow::ensure!(status.success(), "apt-get update failed with {status}");
    Ok(())
}

impl PureDebianPackageManager {
    pub fn new() -> Self {
        Self
    }
}

impl PackageManager for PureDebianPackageManager {
    fn name(&self) -> &'static str {
        "apt-pure"
    }

    fn search(
        &self,
        query: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        let query = query.to_string();
        Box::pin(async move {
            // Index/mmap loading performs disk I/O; keep it off the executor.
            tokio::task::spawn_blocking(move || debian_db::search_fast(&query))
                .await
                .context("Debian search task failed")?
        })
    }

    fn install(
        &self,
        packages: &[String],
    ) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
            use std::time::Instant;

            let start = Instant::now();
            tracing::info!("Starting pure Rust install for {} packages", packages.len());

            // 1-3. Resolve, pre-flight, and URL population are all blocking
            // work (index/mmap loads, statvfs, full index reads); run the
            // whole preparation stage on the blocking pool and only the
            // download/unpack/configure pipeline stays async.
            let package_count = packages.len();
            let mut tx =
                tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                    let mut resolver = debian_db::DependencyResolver::new().context(
                        "Failed to initialize dependency resolver. Try: omg sync",
                    )?;

                    for pkg in &packages {
                        resolver.add_package(pkg).with_context(|| {
                            format!(
                                "Package '{pkg}' not found in repositories.\n\
                                \u{1f4a1} Try:\n\
                                - omg search {pkg} (find similar packages)\n\
                                - omg sync (refresh package database)\n\
                                - Check package name spelling"
                            )
                        })?;
                    }

                    let resolution = resolver.resolve().context(
                        "Dependency resolution failed. Some required dependencies may not be available.",
                    )?;

                    tracing::debug!(
                        "Dependency resolution complete in {:.2}ms: {} to install, {} to upgrade",
                        start.elapsed().as_secs_f64() * 1000.0,
                        resolution.to_install.len(),
                        resolution.to_upgrade.len()
                    );

                    // Check disk space before starting
                    tracing::debug!(
                        "Pre-flight checks: download={} bytes, installed={} bytes",
                        resolution.download_size,
                        resolution.installed_size
                    );
                    debian_db::check_disk_space(
                        resolution.download_size,
                        resolution.installed_size,
                        &std::env::temp_dir(),
                    )
                    .context("Insufficient disk space for installation")?;

                    // Create transaction and populate URLs; a missing URL/SHA256
                    // is fatal here rather than a silent skip downstream.
                    let mut tx = debian_db::Transaction::from_resolution(resolution);
                    populate_package_urls(&mut tx).context(
                        "Failed to resolve package URLs. Repository configuration may be invalid.",
                    )?;
                    Ok(tx)
                })
                .await
                .context("Debian transaction preparation task failed")??;

            // 5. Execute transaction (downloads, unpacks, configures)
            tx.execute().await
                .context("Transaction failed. System may be in inconsistent state. Try: omg install --fix-broken")?;

            let elapsed = start.elapsed();
            tracing::info!(
                "Pure Rust install completed in {:.2}s ({} packages)",
                elapsed.as_secs_f64(),
                package_count
            );

            Ok(())
        })
    }

    fn remove(&self, packages: &[String]) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        let packages = packages.to_vec();
        Box::pin(async move {
            use std::time::Instant;

            let start = Instant::now();
            tracing::info!(
                "Starting pure Rust package removal for {} packages",
                packages.len()
            );

            // Validate packages are installed
            for pkg in &packages {
                let pkg_name = pkg.clone();
                let installed =
                    tokio::task::spawn_blocking(move || debian_db::is_installed_fast(&pkg_name))
                        .await
                        .context("Debian is_installed task failed")??;
                if !installed {
                    anyhow::bail!(
                        "Package '{pkg}' is not installed.\n\
                        \u{1f4a1} Use 'omg list' to see installed packages"
                    );
                }
            }

            let mut tx = debian_db::Transaction::new();
            for pkg in &packages {
                tx.add_remove(pkg.clone());
            }

            // Execute removal
            tx.execute_removal()
                .await
                .context("Package removal failed")?;

            let elapsed = start.elapsed();
            tracing::info!(
                "Pure Rust removal completed in {:.2}s ({} packages)",
                elapsed.as_secs_f64(),
                packages.len()
            );

            Ok(())
        })
    }

    fn update(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            use std::time::Instant;

            let start = Instant::now();
            tracing::info!("Starting pure Rust system upgrade");

            // Get list of packages to upgrade
            let updates = self
                .list_updates()
                .await
                .context("Failed to list available updates. Try: omg sync")?;

            if updates.is_empty() {
                tracing::info!("System is already up to date");
                return Ok(());
            }

            tracing::info!("Found {} packages to upgrade", updates.len());

            // Resolution + URL population are blocking index work; keep them
            // off the executor thread.
            let update_count = updates.len();
            let mut tx =
                tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
                    let mut resolver = debian_db::DependencyResolver::new()
                        .context("Failed to initialize dependency resolver")?;

                    for update in &updates {
                        resolver.add_package(&update.name).with_context(|| {
                            format!(
                                "Package '{}' not found during upgrade resolution.\n\
                                \u{1f4a1} The package may have been removed from repositories.\n\
                                Try: omg sync to refresh package database",
                                update.name
                            )
                        })?;
                    }

                    let resolution = resolver.resolve().context(
                        "Failed to resolve upgrade dependencies. Some packages may have unmet dependencies.",
                    )?;

                    tracing::debug!(
                        "Upgrade resolution complete in {:.2}ms: {} packages",
                        start.elapsed().as_secs_f64() * 1000.0,
                        resolution.to_upgrade.len()
                    );

                    debian_db::check_disk_space(
                        resolution.download_size,
                        resolution.installed_size,
                        &std::env::temp_dir(),
                    )
                    .context("Insufficient disk space for upgrade")?;

                    let mut tx = debian_db::Transaction::from_resolution(resolution);
                    populate_package_urls(&mut tx)
                        .context("Failed to resolve package URLs for upgrade")?;
                    Ok(tx)
                })
                .await
                .context("Debian upgrade preparation task failed")??;

            tx.execute().await.context(
                "Upgrade transaction failed. System may need repair. Try: omg install --fix-broken",
            )?;

            let elapsed = start.elapsed();
            tracing::info!(
                "Pure Rust upgrade completed in {:.2}s ({} packages)",
                elapsed.as_secs_f64(),
                update_count
            );

            Ok(())
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            // APT owns repository authentication and publication into
            // /var/lib/apt/lists. The removed custom downloader wrote a
            // separate user cache that the Debian index never consumed and,
            // critically, did not authenticate Packages indexes against the
            // signed InRelease checksum table.
            update_apt_lists(std::path::Path::new("apt-get"), !crate::core::is_root()).await
        })
    }

    fn info(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Package>>> + Send + '_>> {
        let package = package.to_string();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || debian_db::get_info_fast(&package))
                .await
                .context("Debian info task failed")?
        })
    }

    fn list_installed(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        Box::pin(async move {
            let installed = tokio::task::spawn_blocking(debian_db::list_installed_fast)
                .await
                .context("Debian list_installed task failed")??;
            Ok(installed
                .into_iter()
                .map(|p| Package {
                    name: p.name,
                    version: crate::package_managers::types::parse_version_or_zero(&p.version),
                    description: p.description,
                    source: PackageSource::Official,
                    installed: true,
                })
                .collect())
        })
    }

    fn get_status(
        &self,
        fast: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(usize, usize, usize, usize)>> + Send + '_>> {
        Box::pin(async move {
            let fast_counts = tokio::task::spawn_blocking(debian_db::get_counts_fast)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!("Debian status task failed: {error}");
                    Err(anyhow::anyhow!("Debian status task panicked"))
                });
            tokio::task::spawn_blocking(move || {
                debian_db::resolve_status_counts(fast, &fast_counts, accurate_status_counts)
            })
            .await
            .context("Debian status task failed")?
        })
    }

    fn list_explicit(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move {
            tokio::task::spawn_blocking(debian_db::list_explicit_fast)
                .await
                .context("Debian list_explicit task failed")?
        })
    }

    fn list_updates(&self) -> Pin<Box<dyn Future<Output = Result<Vec<UpdateInfo>>> + Send + '_>> {
        Box::pin(async move {
            // Index/mmap loading plus rayon comparisons are blocking work;
            // run the whole computation on the blocking pool.
            tokio::task::spawn_blocking(compute_updates)
                .await
                .context("Debian list_updates task failed")?
        })
    }

    fn is_installed(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        let package = package.to_string();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || debian_db::is_installed_fast(&package))
                .await
                .context("Debian is_installed task failed")?
        })
    }
}

/// Accurate status counts with real orphan and update numbers. The fast path
/// (`debian_db::get_counts_fast`) omits both rather than reporting fake zeros;
/// this is the fallback [`debian_db::resolve_status_counts`] uses when callers
/// ask for accurate values.
fn accurate_status_counts() -> Result<(usize, usize, usize, usize)> {
    let installed = debian_db::list_installed_fast()?;
    let total = installed.len();
    let explicit = installed.iter().filter(|p| p.is_explicit).count();
    let orphans = debian_db::list_orphans_fast()?.len();
    let updates = compute_updates()?.len();
    Ok((total, explicit, orphans, updates))
}

/// Packages with an available version newer than the installed one.
///
/// Uses the zero-copy mmap index when loaded, falling back to the full
/// in-memory index; mmap failures are logged instead of silently swallowed.
fn compute_updates() -> Result<Vec<UpdateInfo>> {
    use std::collections::HashMap;

    // OPTIMIZATION: Get installed packages first (fast, from dpkg status)
    let installed = debian_db::list_installed_fast()?;
    if installed.is_empty() {
        return Ok(Vec::new());
    }

    // Build an architecture-qualified map so foreign-architecture packages
    // cannot be compared against the host package with the same name.
    let installed_map: HashMap<String, &str> = installed
        .iter()
        .map(|pkg| {
            (
                format!("{}:{}", pkg.name, pkg.architecture),
                pkg.version.as_str(),
            )
        })
        .collect();

    // ULTRA-FAST PATH: mmap index (zero-copy, no full index load)
    let _preload_result = debian_db::ensure_mmap_loaded();
    if debian_db::is_mmap_available() {
        match debian_db::get_updates_from_mmap(&installed_map) {
            Ok(updates) => {
                return Ok(updates
                    .into_iter()
                    .map(|(name, old_version, new_version)| UpdateInfo {
                        name,
                        old_version,
                        new_version,
                        repo: "official".to_string(),
                    })
                    .collect());
            }
            Err(error) => {
                tracing::debug!(
                    "mmap update comparison failed, falling back to full index: {error}"
                );
            }
        }
    }

    // Fallback: Load full index and use parallel comparison
    use crate::package_managers::types::parse_version_or_zero;
    use rayon::prelude::*;

    debian_db::ensure_index_loaded()?;
    let index_pkgs = debian_db::get_detailed_packages()?;

    // Parallel version comparisons using rayon
    let updates: Vec<UpdateInfo> = index_pkgs
        .par_iter()
        .filter_map(|pkg| {
            let installed_ver = debian_db::db::installed_version_for_arch(
                &installed_map,
                &pkg.name,
                &pkg.architecture,
                debian_db::debian_arch(),
            )?;
            let available_ver = parse_version_or_zero(&pkg.version);
            let installed_v = parse_version_or_zero(installed_ver);

            (available_ver > installed_v).then(|| UpdateInfo {
                name: pkg.name.clone(),
                old_version: (*installed_ver).to_string(),
                new_version: pkg.version.clone(),
                repo: "official".to_string(),
            })
        })
        .collect();

    Ok(updates)
}

/// Populate package URLs in a transaction by looking up package info from the
/// database.
///
/// Each package is matched against the repository that actually publishes it
/// (suite + component recorded per index entry), so security/custom mirrors
/// are not silently rewritten to the first enabled repo. Failures are fatal:
/// an action without URL/SHA256 would be skipped by the downloader and the
/// package would silently never be installed.
fn populate_package_urls(tx: &mut debian_db::Transaction) -> Result<()> {
    let repos = debian_db::get_enabled_binary_repos()?;
    if repos.is_empty() {
        anyhow::bail!("No enabled repositories found");
    }

    // OPTIMIZATION: Look up all packages (can be optimized with get_packages_by_names later)
    //
    // SECURITY (audit ADV-18-01): duplicate index entries must resolve to the
    // FIRST occurrence (repository priority order), not an arbitrary
    // last-wins pick from HashMap::from_iter — a later duplicate from a
    // lower-priority component could otherwise substitute a wrong-version
    // download silently.
    let all_packages = debian_db::get_detailed_packages()?;
    let mut package_map: std::collections::HashMap<_, _> = std::collections::HashMap::new();
    for pkg in all_packages {
        package_map.entry(pkg.name.clone()).or_insert(pkg);
    }

    for action in &mut tx.to_install {
        populate_action_url(action, &package_map, &repos)
            .with_context(|| format!("resolving download URL for {}", action.name))?;
        tracing::debug!(
            "Package {name} v{version}: {size} bytes from {url}",
            name = action.name,
            version = action.version,
            size = action.size,
            url = crate::core::http::redact_url(action.url.as_deref().unwrap_or(""))
        );
    }

    for action in &mut tx.to_upgrade {
        populate_action_url(action, &package_map, &repos)
            .with_context(|| format!("resolving download URL for upgrade {}", action.name))?;
        tracing::debug!(
            "Upgrade {} to v{}: {} bytes from {}",
            action.name,
            action.version,
            action.size,
            crate::core::http::redact_url(action.url.as_deref().unwrap_or(""))
        );
    }

    Ok(())
}

/// Resolve one action's `version`/`size`/`sha256`/`url`.
///
/// Repository selection: exact suite+component match first, then suite-only
/// (for flat or componentless entries), then any repo publishing the
/// component. An empty index `suite` degrades to component matching.
fn populate_action_url(
    action: &mut debian_db::PackageAction,
    package_map: &std::collections::HashMap<String, debian_db::DebianPackage>,
    repos: &[debian_db::Repository],
) -> Result<()> {
    let Some(pkg) = package_map.get(&action.name) else {
        anyhow::bail!("package {} not found in database", action.name);
    };
    if pkg.sha256.is_empty() {
        anyhow::bail!(
            "package {} has no SHA256 in repository metadata; refusing unverified install",
            action.name
        );
    }
    if pkg.filename.is_empty() {
        anyhow::bail!(
            "package {} has no Filename in repository metadata",
            action.name
        );
    }

    let repo = repos
        .iter()
        .find(|r| r.suite == pkg.suite && r.components.iter().any(|c| c == &pkg.component))
        .or_else(|| {
            repos
                .iter()
                .find(|r| !pkg.suite.is_empty() && r.suite == pkg.suite)
        })
        .or_else(|| repos.iter().find(|r| r.components.contains(&pkg.component)));
    let Some(repo) = repo else {
        anyhow::bail!(
            "no enabled repository provides suite {:?} / component {:?} for package {}",
            pkg.suite,
            pkg.component,
            action.name
        );
    };

    // Upgrades already carry their target version; installs get the index's.
    if action.version.is_empty() {
        action.version.clone_from(&pkg.version);
    }
    action.size = pkg.size;
    action.sha256 = Some(pkg.sha256.clone());
    // filename is relative to the repo root:
    // "pool/main/v/vim/vim_9.0.1234-1_amd64.deb"
    action.url = Some(format!(
        "{}/{}",
        repo.uri.trim_end_matches('/'),
        pkg.filename
    ));
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::update_apt_lists;
    use std::os::unix::fs::PermissionsExt;

    fn fake_apt_script(body: &str) -> (tempfile::TempDir, std::path::PathBuf) {
        let directory = tempfile::tempdir().expect("temporary fake apt directory");
        let program = directory.path().join("apt-get");
        std::fs::write(&program, format!("#!/bin/sh\n{body}\n")).expect("write fake apt-get");
        let mut permissions = std::fs::metadata(&program)
            .expect("fake apt metadata")
            .permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&program, permissions).expect("make fake apt executable");
        (directory, program)
    }

    #[tokio::test]
    async fn native_sync_executes_exact_apt_update_command() {
        let (_directory, program) = fake_apt_script("[ \"$#\" -eq 1 ] && [ \"$1\" = update ]");

        update_apt_lists(&program, false)
            .await
            .expect("apt update command must succeed");
    }

    #[tokio::test]
    async fn native_sync_propagates_apt_failure() {
        let (_directory, program) = fake_apt_script("exit 23");

        let error = update_apt_lists(&program, false)
            .await
            .expect_err("apt failure must fail sync");
        let chain = format!("{error:#}");
        assert!(
            chain.contains("exit status: 23"),
            "unexpected apt failure: {chain}"
        );
    }
}
