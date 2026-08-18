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
        Box::pin(async move { debian_db::search_fast(&query) })
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

            // 1. Resolve dependencies
            let mut resolver = debian_db::DependencyResolver::new()
                .context("Failed to initialize dependency resolver. Try: omg sync")?;

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

            // 2. Pre-flight checks
            tracing::debug!(
                "Pre-flight checks: download={} bytes, installed={} bytes",
                resolution.download_size,
                resolution.installed_size
            );

            // Check disk space before starting
            let temp_dir = std::env::temp_dir();
            debian_db::check_disk_space(
                resolution.download_size,
                resolution.installed_size,
                &temp_dir,
            )
            .context("Insufficient disk space for installation")?;

            // 3. Create transaction and populate URLs
            let mut tx = debian_db::Transaction::from_resolution(resolution);
            populate_package_urls(&mut tx).context(
                "Failed to resolve package URLs. Repository configuration may be invalid.",
            )?;

            // 4. Check for conflicts
            let conflicts = tx.check_file_conflicts()?;
            if !conflicts.is_empty() {
                tracing::warn!("Found {} file conflicts", conflicts.len());
                for (path, owner) in conflicts.iter().take(5) {
                    tracing::warn!("  {} owned by {}", path.display(), owner);
                }
                anyhow::bail!(
                    "File conflicts detected with {} files. Aborting installation.\n\
                    \u{1f4a1} Some files would be overwritten by this installation.\n\
                    This usually means package conflicts that should be resolved first.",
                    conflicts.len()
                );
            }

            // 5. Execute transaction (downloads, unpacks, configures)
            tx.execute().await
                .context("Transaction failed. System may be in inconsistent state. Try: omg install --fix-broken")?;

            let elapsed = start.elapsed();
            tracing::info!(
                "Pure Rust install completed in {:.2}s ({} packages)",
                elapsed.as_secs_f64(),
                packages.len()
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
                if !debian_db::is_installed_fast(pkg)? {
                    anyhow::bail!(
                        "Package '{pkg}' is not installed.\n\
                        \u{1f4a1} Use 'omg list' to see installed packages"
                    );
                }
            }

            // Create transaction
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

            tracing::info!(
                "Found {} packages to upgrade (total: {} MB)",
                updates.len(),
                updates.iter().map(|_| 0).sum::<usize>() / 1_048_576 // Placeholder size
            );

            // Create resolver and add all packages to upgrade
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

            let resolution = resolver.resolve()
                .context("Failed to resolve upgrade dependencies. Some packages may have unmet dependencies.")?;

            tracing::debug!(
                "Upgrade resolution complete in {:.2}ms: {} packages",
                start.elapsed().as_secs_f64() * 1000.0,
                resolution.to_upgrade.len()
            );

            // Check disk space before upgrade
            let temp_dir = std::env::temp_dir();
            debian_db::check_disk_space(
                resolution.download_size,
                resolution.installed_size,
                &temp_dir,
            )
            .context("Insufficient disk space for upgrade")?;

            // Execute upgrade transaction
            let mut tx = debian_db::Transaction::from_resolution(resolution);
            populate_package_urls(&mut tx).context("Failed to resolve package URLs for upgrade")?;

            tx.execute().await.context(
                "Upgrade transaction failed. System may need repair. Try: omg install --fix-broken",
            )?;

            let elapsed = start.elapsed();
            tracing::info!(
                "Pure Rust upgrade completed in {:.2}s ({} packages)",
                elapsed.as_secs_f64(),
                updates.len()
            );

            Ok(())
        })
    }

    fn sync(&self) -> Pin<Box<dyn Future<Output = Result<()>> + Send + '_>> {
        Box::pin(async move {
            use std::time::Instant;

            let start = Instant::now();
            tracing::info!("Starting pure Rust repository sync");

            // Use parallel sync for maximum performance
            // Note: show_progress=false to avoid tty issues in some contexts
            match debian_db::sync_all_repositories(false).await {
                Ok(()) => {
                    let elapsed = start.elapsed();
                    tracing::info!(
                        "Pure Rust sync completed in {:.2}s (faster than apt update)",
                        elapsed.as_secs_f64()
                    );
                    Ok(())
                }
                Err(e) => {
                    tracing::error!("Pure Rust sync failed: {}", e);
                    Err(e)
                }
            }
        })
    }

    fn info(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = Result<Option<Package>>> + Send + '_>> {
        let package = package.to_string();
        Box::pin(async move { debian_db::get_info_fast(&package) })
    }

    fn list_installed(&self) -> Pin<Box<dyn Future<Output = Result<Vec<Package>>> + Send + '_>> {
        Box::pin(async move {
            let installed = debian_db::list_installed_fast()?;
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
        _fast: bool,
    ) -> Pin<Box<dyn Future<Output = Result<(usize, usize, usize, usize)>> + Send + '_>> {
        Box::pin(async move { debian_db::get_counts_fast() })
    }

    fn list_explicit(&self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + '_>> {
        Box::pin(async move { debian_db::list_explicit_fast() })
    }

    fn list_updates(&self) -> Pin<Box<dyn Future<Output = Result<Vec<UpdateInfo>>> + Send + '_>> {
        Box::pin(async move {
            use std::collections::HashMap;

            // OPTIMIZATION: Get installed packages first (fast, from dpkg status)
            let installed = debian_db::list_installed_fast()?;
            if installed.is_empty() {
                return Ok(Vec::new());
            }

            // Build set of installed package names for O(1) lookup
            let installed_map: HashMap<&str, &str> = installed
                .iter()
                .map(|pkg| (pkg.name.as_str(), pkg.version.as_str()))
                .collect();

            // ULTRA-FAST PATH: Use mmap index if available (zero-copy, no full index load)
            // ensure_mmap_loaded() loads from disk if not yet in memory (nearly instant)
            debian_db::ensure_mmap_loaded();
            if debian_db::is_mmap_available()
                && let Ok(updates) = debian_db::get_updates_from_mmap(&installed_map)
            {
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

            // Fallback: Load full index and use parallel comparison
            use crate::package_managers::types::parse_version_or_zero;
            use rayon::prelude::*;

            debian_db::ensure_index_loaded()?;
            let index_pkgs = debian_db::get_detailed_packages()?;

            // Parallel version comparisons using rayon
            let updates: Vec<UpdateInfo> = index_pkgs
                .par_iter()
                .filter_map(|pkg| {
                    let installed_ver = installed_map.get(pkg.name.as_str())?;
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
        })
    }

    fn is_installed(
        &self,
        package: &str,
    ) -> Pin<Box<dyn Future<Output = Result<bool>> + Send + '_>> {
        let result = debian_db::is_installed_fast(package);
        Box::pin(async move { result })
    }
}

/// Populate package URLs in a transaction by looking up package info from the database
fn populate_package_urls(tx: &mut debian_db::Transaction) -> Result<()> {
    // Get repository information
    let repos = debian_db::get_enabled_binary_repos()?;
    if repos.is_empty() {
        anyhow::bail!("No enabled repositories found");
    }

    // OPTIMIZATION: Look up all packages (can be optimized with get_packages_by_names later)
    let all_packages = debian_db::get_detailed_packages()?;
    let package_map: std::collections::HashMap<_, _> = all_packages
        .into_iter()
        .map(|pkg| (pkg.name.clone(), pkg))
        .collect();

    // Primary repository for URL construction (use first enabled repo)
    let primary_repo = &repos[0];
    let base_url = primary_repo.uri.trim_end_matches('/');

    // Populate URLs for packages to install
    for action in &mut tx.to_install {
        if let Some(pkg) = package_map.get(&action.name) {
            action.version.clone_from(&pkg.version);
            action.size = pkg.size;
            action.sha256 = Some(pkg.sha256.clone());
            // Construct full URL: base_url + filename
            // filename is typically like "pool/main/v/vim/vim_9.0.1234-1_amd64.deb"
            let url = format!("{}/{}", base_url, pkg.filename);
            tracing::debug!(
                "Package {} v{}: {} bytes from {}",
                action.name,
                action.version,
                action.size,
                url
            );
            action.url = Some(url);
        } else {
            tracing::warn!("Package {} not found in database", action.name);
        }
    }

    // Populate URLs for packages to upgrade
    for action in &mut tx.to_upgrade {
        if let Some(pkg) = package_map.get(&action.name) {
            action.size = pkg.size;
            action.sha256 = Some(pkg.sha256.clone());
            let url = format!("{}/{}", base_url, pkg.filename);
            tracing::debug!(
                "Upgrade {} to v{}: {} bytes from {}",
                action.name,
                action.version,
                action.size,
                url
            );
            action.url = Some(url);
        }
    }

    Ok(())
}
