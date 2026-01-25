//! Direct libalpm operations - LIGHTNING FAST
//!
//! Pure libalpm transactions - no pacman subprocess.
//! Install/remove/update operations at native C library speed.

use alpm_types::Version;
use anyhow::{Context, Result};
use owo_colors::OwoColorize;

use crate::core::paths;
#[cfg(feature = "pgp")]
use crate::core::security::pgp::PgpVerifier;
use crate::package_managers::pacman_db;
use crate::package_managers::types::{PackageInfo, UpdateInfo};

/// Get comprehensive system status (counts + updates) in a single pass - FAST
pub fn get_system_status() -> Result<(usize, usize, usize, usize)> {
    let (total, explicit, orphans) = crate::package_managers::get_counts()?;
    let updates = crate::package_managers::check_updates_cached()?.len();
    Ok((total, explicit, orphans, updates))
}

/// Get detailed list of updates (name, `old_version`, `new_version`) - FAST
pub fn get_update_list() -> Result<Vec<UpdateInfo>> {
    if crate::core::paths::test_mode() {
        let updates = crate::package_managers::pacman_db::check_updates_cached()?;
        return Ok(updates
            .into_iter()
            .map(|(name, old_ver, new_ver, repo, _, _)| UpdateInfo {
                name,
                old_version: old_ver.to_string(),
                new_version: new_ver.to_string(),
                repo,
            })
            .collect());
    }

    crate::package_managers::alpm_direct::with_handle(|alpm| {
        let mut updates = Vec::new();
        let localdb = alpm.localdb();
        let syncdbs = alpm.syncdbs();

        for pkg in localdb.pkgs() {
            let name = pkg.name();
            let local_ver_str = pkg.version().as_str();

            for db in syncdbs {
                if let Ok(sync_pkg) = db.pkg(name) {
                    let sync_ver_str = sync_pkg.version().as_str();
                    if alpm::vercmp(sync_ver_str, local_ver_str) == std::cmp::Ordering::Greater {
                        updates.push(UpdateInfo {
                            name: name.to_string(),
                            old_version: local_ver_str.to_string(),
                            new_version: sync_ver_str.to_string(),
                            repo: db.name().to_string(),
                        });
                        break;
                    }
                }
            }
        }

        Ok(updates)
    })
}

/// Information needed for downloading a package
#[derive(Debug, Clone)]
pub struct DownloadInfo {
    pub name: String,
    pub version: Version,
    pub repo: String,
    pub filename: String,
    pub size: u64,
}

/// Get download information for all available updates - for parallel downloads
pub fn get_update_download_list() -> Result<Vec<DownloadInfo>> {
    crate::package_managers::alpm_direct::with_handle(|alpm| {
        let mut downloads = Vec::new();
        let localdb = alpm.localdb();
        let syncdbs = alpm.syncdbs();

        for pkg in localdb.pkgs() {
            let name = pkg.name();
            let local_ver = pkg.version().as_str();

            for db in syncdbs {
                if let Ok(sync_pkg) = db.pkg(name) {
                    let sync_ver = sync_pkg.version().as_str();
                    if alpm::vercmp(sync_ver, local_ver) == std::cmp::Ordering::Greater {
                        downloads.push(DownloadInfo {
                            name: name.to_string(),
                            version: super::types::parse_version_or_zero(sync_ver),
                            repo: db.name().to_string(),
                            filename: sync_pkg.filename().unwrap_or_default().to_string(),
                            size: sync_pkg.download_size() as u64,
                        });
                        break;
                    }
                }
            }
        }

        Ok(downloads)
    })
}

/// Get package info from sync DBs - INSTANT (<1ms)
pub fn get_sync_pkg_info(name: &str) -> Result<Option<PackageInfo>> {
    if paths::test_mode() {
        if let Some(pkg) = pacman_db::get_sync_package(name)? {
            return Ok(Some(PackageInfo {
                name: pkg.name,
                version: pkg.version.clone(),
                description: pkg.desc,
                url: Some(pkg.url),
                size: pkg.isize,
                install_size: Some(i64::try_from(pkg.isize).unwrap_or(i64::MAX)),
                download_size: Some(pkg.csize),
                repo: pkg.repo,
                depends: pkg.depends,
                licenses: Vec::new(),
                installed: false,
            }));
        }
        return Ok(None);
    }

    crate::package_managers::alpm_direct::with_handle(|alpm| get_pkg_info_from_db(alpm, name))
}

/// Get package info using an existing ALPM handle - ULTRA FAST
pub fn get_pkg_info_from_db(alpm: &alpm::Alpm, name: &str) -> Result<Option<PackageInfo>> {
    for db in alpm.syncdbs() {
        if let Ok(pkg) = db.pkg(name) {
            return Ok(Some(PackageInfo {
                name: pkg.name().to_string(),
                version: super::types::parse_version_or_zero(pkg.version()),
                description: pkg.desc().unwrap_or("").to_string(),
                url: pkg.url().map(std::string::ToString::to_string),
                size: pkg.isize() as u64,
                install_size: Some(pkg.isize()),
                download_size: Some(pkg.size() as u64),
                repo: db.name().to_string(),
                depends: pkg.depends().iter().map(|d| d.name().to_string()).collect(),
                licenses: pkg
                    .licenses()
                    .iter()
                    .map(std::string::ToString::to_string)
                    .collect(),
                installed: alpm.localdb().pkg(pkg.name()).is_ok(),
            }));
        }
    }

    Ok(None)
}

/// Clean package cache using direct file system operations - FAST
pub fn clean_cache(keep_versions: usize) -> Result<(usize, u64)> {
    let cache_dir = paths::pacman_cache_dir();

    if !cache_dir.exists() {
        return Ok((0, 0));
    }

    let mut packages: std::collections::HashMap<String, Vec<std::path::PathBuf>> =
        std::collections::HashMap::new();

    for entry in std::fs::read_dir(&cache_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(filename) = path.file_name().and_then(|n| n.to_str())
            && (filename.ends_with(".pkg.tar.zst") || filename.ends_with(".pkg.tar.xz"))
        {
            if let Some(base) = filename.rsplitn(5, '-').last() {
                packages.entry(base.to_string()).or_default().push(path);
            }
        }
    }

    let mut removed = 0;
    let mut freed = 0u64;

    for (_, mut versions) in packages {
        versions.sort_by(|a, b| {
            let a_time = a.metadata().and_then(|m| m.modified()).ok();
            let b_time = b.metadata().and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        for old in versions.into_iter().skip(keep_versions) {
            if let Ok(meta) = old.metadata() {
                freed += meta.len();
            }
            if std::fs::remove_file(&old).is_ok() {
                removed += 1;
            }
        }
    }

    Ok((removed, freed))
}

/// List orphaned packages - INSTANT
pub fn list_orphans_direct() -> Result<Vec<String>> {
    crate::package_managers::alpm_direct::with_handle(|alpm| {
        let mut orphans = Vec::new();

        for pkg in alpm.localdb().pkgs() {
            if pkg.reason() != alpm::PackageReason::Explicit
                && pkg.required_by().is_empty()
                && pkg.optional_for().is_empty()
            {
                orphans.push(pkg.name().to_string());
            }
        }

        Ok(orphans)
    })
}

/// Synchronize package databases from mirrors - FAST
pub fn sync_dbs() -> Result<()> {
    crate::package_managers::alpm_direct::with_handle_mut(|alpm| {
        alpm.syncdbs_mut()
            .update(false)
            .map_err(|e| {
                anyhow::anyhow!(
                    "✗ Sync Error: Failed to update package databases: {e}.\n  Check your internet connection or run 'omg sync' with sudo."
                )
            })?;

        Ok(())
    })
}

/// Display package info beautifully
pub fn display_pkg_info(info: &PackageInfo) {
    println!("{} {}", info.name.white().bold(), info.version.green());
    println!("  {} {}", "Description:".dimmed(), info.description);
    println!("  {} {}", "Repository:".dimmed(), info.repo.cyan());
    println!(
        "  {} {}",
        "URL:".dimmed(),
        info.url.as_deref().unwrap_or("-")
    );
    println!(
        "  {} {:.2} MB",
        "Size:".dimmed(),
        info.size as f64 / 1024.0 / 1024.0
    );
    println!(
        "  {} {:.2} MB",
        "Download:".dimmed(),
        info.download_size.unwrap_or(0) as f64 / 1024.0 / 1024.0
    );
    if !info.licenses.is_empty() {
        println!("  {} {}", "License:".dimmed(), info.licenses.join(", "));
    }
    if !info.depends.is_empty() {
        println!("  {} {}", "Depends:".dimmed(), info.depends.join(", "));
    }
}

/// RAII Guard for ALPM transactions to ensure release
struct AlpmTransaction<'a>(&'a mut alpm::Alpm);

impl Drop for AlpmTransaction<'_> {
    fn drop(&mut self) {
        let _ = self.0.trans_release();
    }
}

/// Execute a libalpm transaction (install/remove/sysupgrade)
pub fn execute_transaction(
    packages: Vec<String>,
    remove: bool,
    sysupgrade: bool,
    handle: Option<&mut alpm::Alpm>,
) -> Result<()> {
    if let Some(alpm) = handle {
        configure_mirrors(alpm)?;
        let mp = indicatif::MultiProgress::new();
        let main_pb = setup_alpm_callbacks(alpm, &mp)?;
        let mut tx_guard = prepare_alpm_transaction(alpm, packages, remove, sysupgrade)?;
        commit_alpm_transaction(&mut tx_guard.0, &main_pb)?;
        return Ok(());
    }

    let root = paths::pacman_root().to_string_lossy().into_owned();
    let db_path = paths::pacman_db_dir().to_string_lossy().into_owned();
    let mut alpm =
        alpm::Alpm::new(root, db_path).context("Failed to initialize ALPM (are you root?)")?;

    for db_name in ["core", "extra", "multilib"] {
        let _ = alpm.register_syncdb(db_name, alpm::SigLevel::USE_DEFAULT);
    }

    configure_mirrors(&mut alpm)?;

    let mp = indicatif::MultiProgress::new();
    let main_pb = setup_alpm_callbacks(&mut alpm, &mp)?;
    let mut tx_guard = prepare_alpm_transaction(&mut alpm, packages, remove, sysupgrade)?;
    commit_alpm_transaction(&mut tx_guard.0, &main_pb)?;

    Ok(())
}

/// Setup ALPM callbacks for progress bars
#[allow(clippy::expect_used)]
fn setup_alpm_callbacks(
    alpm: &mut alpm::Alpm,
    mp: &indicatif::MultiProgress,
) -> Result<indicatif::ProgressBar> {
    let main_pb = mp.add(indicatif::ProgressBar::new(100));
    main_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("{spinner:.green} {prefix:.bold.dim} {msg} {wide_bar:.cyan/blue} {pos}/{len}")
            .expect("valid template")
            .progress_chars("━━╸ "),
    );
    main_pb.set_prefix("Transaction");

    let main_pb_clone = main_pb.clone();
    alpm.set_progress_cb((), move |op, name, percent, _n, _max, ()| {
        let msg = match op {
            alpm::Progress::AddStart => "Installing",
            alpm::Progress::UpgradeStart => "Upgrading",
            alpm::Progress::DowngradeStart => "Downgrading",
            alpm::Progress::ReinstallStart => "Reinstalling",
            alpm::Progress::RemoveStart => "Removing",
            alpm::Progress::ConflictsStart => "Conflict check",
            alpm::Progress::DiskspaceStart => "Checking disk space",
            alpm::Progress::IntegrityStart => "Checking integrity",
            alpm::Progress::LoadStart => "Loading",
            alpm::Progress::KeyringStart => "Checking keyring",
        };
        main_pb_clone.set_message(format!("{msg}: {name}"));
        main_pb_clone.set_position(percent as u64);
    });

    let dl_pb_map = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
        String,
        indicatif::ProgressBar,
    >::new()));
    let mp_clone = mp.clone();

    alpm.set_dl_cb(dl_pb_map, move |filename, event, map| {
        let Ok(mut map) = map.lock() else { return };
        match event.event() {
            alpm::DownloadEvent::Init(_) => {
                if map.len() < 4 {
                    let pb = mp_clone.add(indicatif::ProgressBar::new_spinner());
                    pb.set_style(
                        indicatif::ProgressStyle::default_spinner()
                            .template("  {spinner:.green} {msg:20} [Starting download...]")
                            .expect("valid template"),
                    );
                    pb.set_message(filename.to_string());
                    map.insert(filename.to_string(), pb);
                }
            }
            alpm::DownloadEvent::Progress(prog) => {
                if let Some(pb) = map.get(filename) {
                    if pb.length().is_none() && prog.total > 0 {
                        pb.set_length(prog.total as u64);
                        pb.set_style(
                            indicatif::ProgressStyle::default_bar()
                                .template("  {spinner:.green} {msg:20} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
                                .expect("valid template")
                                .progress_chars("█▓▒░"),
                        );
                    }
                    pb.set_position(prog.downloaded as u64);
                }
            }
            alpm::DownloadEvent::Retry(_) => {}
            alpm::DownloadEvent::Completed(_) => {
                if let Some(pb) = map.remove(filename) {
                    pb.finish_and_clear();
                }
            }
        }
    });

    Ok(main_pb)
}

/// Prepare an ALPM transaction for execution
fn prepare_alpm_transaction<'a>(
    alpm: &'a mut alpm::Alpm,
    packages: Vec<String>,
    remove: bool,
    sysupgrade: bool,
) -> Result<AlpmTransaction<'a>> {
    use alpm::TransFlag;

    let mut flags = TransFlag::NEEDED;
    if remove {
        flags |= TransFlag::RECURSE | TransFlag::UNNEEDED;
    }

    alpm.trans_init(flags).map_err(|e| match e {
        alpm::Error::HandleLock => {
            anyhow::anyhow!("Database is locked. Is another package manager running?")
        }
        _ => anyhow::anyhow!("Failed to initialize transaction: {e}"),
    })?;

    let tx_guard = AlpmTransaction(alpm);

    if sysupgrade {
        tx_guard
            .0
            .sync_sysupgrade(false)
            .context("Failed to setup sysupgrade")?;
    } else {
        for pkg_name in packages {
            if remove {
                if let Ok(pkg) = tx_guard.0.localdb().pkg(pkg_name.clone()) {
                    tx_guard.0.trans_remove_pkg(pkg).map_err(|e| {
                        anyhow::anyhow!("Failed to add {pkg_name} to removal list: {e}")
                    })?;
                }
            } else {
                let path = std::path::Path::new(&pkg_name);
                if path.exists() && (pkg_name.contains(".pkg.tar.") || path.is_absolute()) {
                    let pkg = tx_guard
                        .0
                        .pkg_load(pkg_name.clone(), true, alpm::SigLevel::USE_DEFAULT)
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to load local package {pkg_name}: {e}")
                        })?;
                    tx_guard.0.trans_add_pkg(pkg).map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to add local package {pkg_name} to transaction: {e}"
                        )
                    })?;
                    continue;
                }

                let mut found = false;
                for db in tx_guard.0.syncdbs() {
                    if let Ok(pkg) = db.pkg(pkg_name.clone()) {
                        tx_guard.0.trans_add_pkg(pkg).map_err(|e| {
                            anyhow::anyhow!("Failed to add {pkg_name} to installation list: {e}")
                        })?;
                        found = true;
                        break;
                    }
                }
                if !found {
                    anyhow::bail!("Package {pkg_name} not found in any repository");
                }
            }
        }
    }

    Ok(tx_guard)
}

/// Commit an ALPM transaction
#[allow(clippy::expect_used)]
fn commit_alpm_transaction(alpm: &mut alpm::Alpm, main_pb: &indicatif::ProgressBar) -> Result<()> {
    alpm.trans_prepare().map_err(|e| {
        anyhow::anyhow!(
            "✗ Preparation Error: Transaction failed to prepare: {e}.\n  This may be due to conflicting packages or missing dependencies."
        )
    })?;

    #[cfg(feature = "pgp")]
    {
        let verifier = PgpVerifier::new();
        let pkgs_to_add = alpm.trans_add();

        if !pkgs_to_add.is_empty() {
            main_pb.set_message("Verifying package signatures with Sequoia...");
            for pkg in pkgs_to_add {
                let pkg_name: &str = pkg.name();
                if let Some(pkg_filename) = pkg.filename() {
                    let cache_path = paths::pacman_cache_dir().join(pkg_filename);
                    let sig_path =
                        std::path::PathBuf::from(format!("{}.sig", cache_path.display()));

                    if cache_path.exists()
                        && sig_path.exists()
                        && let Err(e) = verifier.verify_package(&cache_path, &sig_path)
                    {
                        anyhow::bail!(
                            "✗ SECURITY ALERT: PGP verification failed for {pkg_name}.\n  Error: {e}\n  The package may be corrupted or tampered with."
                        );
                    }
                }
            }
        }
    }

    if alpm.trans_add().is_empty() && alpm.trans_remove().is_empty() {
        main_pb.finish_with_message("Nothing to do: system is already up to date.");
        return Ok(());
    }

    main_pb.set_message("Committing transaction...");
    alpm.trans_commit()
        .context("Transaction failed to commit. Run 'omg cleanup' if issue persists.")?;

    main_pb.finish_with_message("Transaction successful");

    Ok(())
}

/// Parse /etc/pacman.d/mirrorlist and configure ALPM servers
#[allow(clippy::expect_used)]
fn configure_mirrors(alpm: &mut alpm::Alpm) -> Result<()> {
    let mirrorlist = paths::pacman_mirrorlist_path();
    if !mirrorlist.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(mirrorlist)?;
    let re = regex::Regex::new(r"^Server\s*=\s*([^#]+)").expect("valid regex");
    let mut servers = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if let Some(url) = re.captures(line).and_then(|caps| caps.get(1)) {
            servers.push(url.as_str().trim().to_string());
        }
    }

    for db in alpm.syncdbs_mut() {
        let db_name = db.name().to_string();
        for server in &servers {
            let url = server
                .replace("$repo", &db_name)
                .replace("$arch", std::env::consts::ARCH);
            let _ = db.add_server(url);
        }
    }
    Ok(())
}
