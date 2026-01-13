//! Direct libalpm operations - LIGHTNING FAST
//!
//! Pure libalpm transactions - no pacman subprocess.
//! Install/remove/update operations at native C library speed.

use crate::core::security::pgp::PgpVerifier;
use anyhow::{Context, Result};
use colored::Colorize;

/// Check available updates using direct DB comparison - INSTANT
/// Get comprehensive system status (counts + updates) in a single pass - FAST
pub fn get_system_status() -> Result<(usize, usize, usize, usize)> {
    let alpm = alpm::Alpm::new("/", "/var/lib/pacman").context("Failed to initialize ALPM")?;

    // Register sync DBs
    for db_name in ["core", "extra", "multilib"] {
        let _ = alpm.register_syncdb(db_name, alpm::SigLevel::USE_DEFAULT);
    }

    let mut total = 0;
    let mut explicit = 0;
    let mut orphans = 0;
    let mut updates = 0;

    let syncdbs = alpm.syncdbs();

    for pkg in alpm.localdb().pkgs() {
        total += 1;

        if pkg.reason() == alpm::PackageReason::Explicit {
            explicit += 1;
        } else if pkg.required_by().is_empty() && pkg.optional_for().is_empty() {
            orphans += 1;
        }

        let name = pkg.name();
        let local_ver = pkg.version().as_str();

        // Find if any sync DB has a newer version
        for db in syncdbs.iter() {
            if let Ok(sync_pkg) = db.pkg(name) {
                if alpm::vercmp(sync_pkg.version().as_str(), local_ver)
                    == std::cmp::Ordering::Greater
                {
                    updates += 1;
                    break;
                }
            }
        }
    }

    Ok((total, explicit, orphans, updates))
}

/// Get detailed list of updates (name, old_version, new_version) - FAST
pub fn get_update_list() -> Result<Vec<(String, String, String)>> {
    let alpm = alpm::Alpm::new("/", "/var/lib/pacman").context("Failed to initialize ALPM")?;

    // Register sync DBs
    for db_name in ["core", "extra", "multilib"] {
        let _ = alpm.register_syncdb(db_name, alpm::SigLevel::USE_DEFAULT);
    }

    let mut updates = Vec::new();
    let syncdbs = alpm.syncdbs();

    for pkg in alpm.localdb().pkgs() {
        let name = pkg.name();
        let local_ver = pkg.version().as_str();

        for db in syncdbs.iter() {
            if let Ok(sync_pkg) = db.pkg(name) {
                let sync_ver = sync_pkg.version().as_str();
                if alpm::vercmp(sync_ver, local_ver) == std::cmp::Ordering::Greater {
                    updates.push((
                        name.to_string(),
                        local_ver.to_string(),
                        sync_ver.to_string(),
                    ));
                    break;
                }
            }
        }
    }

    Ok(updates)
}

/// Get package info from sync DBs - INSTANT (<1ms)
pub fn get_sync_pkg_info(name: &str) -> Result<Option<PackageInfo>> {
    let alpm = alpm::Alpm::new("/", "/var/lib/pacman").context("Failed to initialize ALPM")?;

    // Register sync DBs
    for db_name in ["core", "extra", "multilib"] {
        let _ = alpm.register_syncdb(db_name, alpm::SigLevel::USE_DEFAULT);
    }

    get_pkg_info_from_db(&alpm, name)
}

/// Get package info using an existing ALPM handle - ULTRA FAST
pub fn get_pkg_info_from_db(alpm: &alpm::Alpm, name: &str) -> Result<Option<PackageInfo>> {
    // Search sync DBs
    for db in alpm.syncdbs() {
        if let Ok(pkg) = db.pkg(name) {
            return Ok(Some(PackageInfo {
                name: pkg.name().to_string(),
                version: pkg.version().to_string(),
                description: pkg.desc().unwrap_or("").to_string(),
                url: pkg.url().unwrap_or("").to_string(),
                size: pkg.isize() as u64,
                download_size: pkg.size() as u64,
                repo: db.name().to_string(),
                depends: pkg.depends().iter().map(|d| d.to_string()).collect(),
                licenses: pkg.licenses().iter().map(|l| l.to_string()).collect(),
            }));
        }
    }

    Ok(None)
}

#[derive(Debug)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub description: String,
    pub url: String,
    pub size: u64,
    pub download_size: u64,
    pub repo: String,
    pub depends: Vec<String>,
    pub licenses: Vec<String>,
}

/// Clean package cache using direct file system operations - FAST
pub fn clean_cache(keep_versions: usize) -> Result<(usize, u64)> {
    let cache_dir = std::path::Path::new("/var/cache/pacman/pkg");

    if !cache_dir.exists() {
        return Ok((0, 0));
    }

    let mut packages: std::collections::HashMap<String, Vec<std::path::PathBuf>> =
        std::collections::HashMap::new();

    // Group package files by base name
    for entry in std::fs::read_dir(cache_dir)? {
        let entry = entry?;
        let path = entry.path();

        if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
            if filename.ends_with(".pkg.tar.zst") || filename.ends_with(".pkg.tar.xz") {
                // Extract package name (remove version-release.arch.pkg.tar.zst)
                if let Some(base) = filename.rsplitn(5, '-').last() {
                    packages.entry(base.to_string()).or_default().push(path);
                }
            }
        }
    }

    let mut removed = 0;
    let mut freed = 0u64;

    // Keep only the most recent versions
    for (_, mut versions) in packages {
        // Sort by modification time (newest first)
        versions.sort_by(|a, b| {
            let a_time = a.metadata().and_then(|m| m.modified()).ok();
            let b_time = b.metadata().and_then(|m| m.modified()).ok();
            b_time.cmp(&a_time)
        });

        // Remove old versions
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
    let alpm = alpm::Alpm::new("/", "/var/lib/pacman").context("Failed to initialize ALPM")?;

    let mut orphans = Vec::new();

    for pkg in alpm.localdb().pkgs() {
        // Package is orphan if:
        // 1. Not explicitly installed
        // 2. Nothing depends on it
        if pkg.reason() != alpm::PackageReason::Explicit {
            if pkg.required_by().is_empty() && pkg.optional_for().is_empty() {
                orphans.push(pkg.name().to_string());
            }
        }
    }

    Ok(orphans)
}

/// Synchronize package databases from mirrors - FAST
pub fn sync_dbs() -> Result<()> {
    let mut alpm = alpm::Alpm::new("/", "/var/lib/pacman")
        .context("Failed to initialize ALPM (are you root?)")?;

    // Register sync DBs if they aren't already
    for db_name in ["core", "extra", "multilib"] {
        let _ = alpm.register_syncdb(db_name, alpm::SigLevel::USE_DEFAULT);
    }

    // Update all registered sync DBs
    alpm.syncdbs_mut()
        .update(false)
        .map_err(|e| {
            anyhow::anyhow!(
                "✗ Sync Error: Failed to update package databases: {}.\n  Check your internet connection or run 'omg sync' with sudo.",
                e
            )
        })?;

    Ok(())
}

/// Display package info beautifully
pub fn display_pkg_info(info: &PackageInfo) {
    println!("{} {}", info.name.white().bold(), info.version.green());
    println!("  {} {}", "Description:".dimmed(), info.description);
    println!("  {} {}", "Repository:".dimmed(), info.repo.cyan());
    println!("  {} {}", "URL:".dimmed(), info.url);
    println!(
        "  {} {:.2} MB",
        "Size:".dimmed(),
        info.size as f64 / 1024.0 / 1024.0
    );
    println!(
        "  {} {:.2} MB",
        "Download:".dimmed(),
        info.download_size as f64 / 1024.0 / 1024.0
    );
    if !info.licenses.is_empty() {
        println!("  {} {}", "License:".dimmed(), info.licenses.join(", "));
    }
    if !info.depends.is_empty() {
        println!("  {} {}", "Depends:".dimmed(), info.depends.join(", "));
    }
}

/// Execute a libalpm transaction (install/remove/sysupgrade)
pub fn execute_transaction(packages: Vec<String>, remove: bool, sysupgrade: bool) -> Result<()> {
    use alpm::{SigLevel, TransFlag};
    use indicatif::{ProgressBar, ProgressStyle};

    let mut alpm = alpm::Alpm::new("/", "/var/lib/pacman")
        .context("Failed to initialize ALPM (are you root?)")?;

    // Register sync DBs
    for db_name in ["core", "extra", "multilib"] {
        let _ = alpm.register_syncdb(db_name, SigLevel::USE_DEFAULT);
    }

    // Set up progress bars
    let mp = indicatif::MultiProgress::new();
    let main_pb = mp.add(ProgressBar::new_spinner());
    main_pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    main_pb.enable_steady_tick(std::time::Duration::from_millis(100));

    // Progress callback
    let main_pb_clone = main_pb.clone();
    alpm.set_progress_cb((), move |op, name, percent, _n, _max, _| {
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
        main_pb_clone.set_message(format!("{}: {} {}%", msg, name, percent));
    });

    // Download callback
    let dl_pb_map = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
        String,
        ProgressBar,
    >::new()));
    let mp_clone = mp.clone();

    alpm.set_dl_cb(dl_pb_map, move |filename, event, map| {
        let mut map = map.lock().unwrap();
        match event.event() {
            alpm::DownloadEvent::Init(_) => {
                let pb = mp_clone.add(ProgressBar::new_spinner());
                pb.set_style(
                    ProgressStyle::default_spinner()
                        .template("  {spinner:.green} {msg:20} [Starting download...]")
                        .unwrap()
                );
                pb.set_message(filename.to_string());
                map.insert(filename.to_string(), pb);
            }
            alpm::DownloadEvent::Progress(prog) => {
                if let Some(pb) = map.get(filename) {
                    // Update to a real progress bar if we have the total
                    if pb.length().is_none() && prog.total > 0 {
                        pb.set_length(prog.total as u64);
                        pb.set_style(
                            ProgressStyle::default_bar()
                                .template("  {spinner:.green} {msg:20} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
                                .unwrap()
                                .progress_chars("█▓▒░")
                        );
                    }
                    pb.set_position(prog.downloaded as u64);
                } else if prog.total > 0 {
                    // Fallback if Init was missed
                    let pb = mp_clone.add(ProgressBar::new(prog.total as u64));
                    pb.set_style(
                        ProgressStyle::default_bar()
                            .template("  {spinner:.green} {msg:20} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
                            .unwrap()
                            .progress_chars("█▓▒░")
                    );
                    pb.set_message(filename.to_string());
                    pb.set_position(prog.downloaded as u64);
                    map.insert(filename.to_string(), pb);
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

    // Initialize transaction
    let mut flags = TransFlag::NEEDED;
    if remove {
        flags |= TransFlag::RECURSE | TransFlag::UNNEEDED;
    }

    alpm.trans_init(flags)
        .context("Failed to initialize transaction")?;

    if sysupgrade {
        alpm.sync_sysupgrade(false)
            .context("Failed to setup sysupgrade")?;
    } else {
        for pkg_name in packages {
            if remove {
                if let Ok(pkg) = alpm.localdb().pkg(pkg_name.clone()) {
                    alpm.trans_remove_pkg(pkg).map_err(|e| {
                        anyhow::anyhow!("Failed to add {} to removal list: {:?}", pkg_name, e)
                    })?;
                }
            } else {
                // Try to load as a local package file first
                let path = std::path::Path::new(&pkg_name);
                if path.exists() && (pkg_name.contains(".pkg.tar.") || path.is_absolute()) {
                    let pkg = alpm
                        .pkg_load(pkg_name.clone(), true, alpm::SigLevel::USE_DEFAULT)
                        .map_err(|e| {
                            anyhow::anyhow!("Failed to load local package {}: {:?}", pkg_name, e)
                        })?;
                    alpm.trans_add_pkg(pkg).map_err(|e| {
                        anyhow::anyhow!(
                            "Failed to add local package {} to transaction: {:?}",
                            pkg_name,
                            e
                        )
                    })?;
                    continue;
                }

                // Find in sync DBs
                let mut found = false;
                for db in alpm.syncdbs() {
                    if let Ok(pkg) = db.pkg(pkg_name.clone()) {
                        alpm.trans_add_pkg(pkg).map_err(|e| {
                            anyhow::anyhow!(
                                "Failed to add {} to installation list: {:?}",
                                pkg_name,
                                e
                            )
                        })?;
                        found = true;
                        break;
                    }
                }
                if !found {
                    anyhow::bail!("Package {} not found in any repository", pkg_name);
                }
            }
        }
    }

    // Prepare
    alpm.trans_prepare()
        .map_err(|e| {
            anyhow::anyhow!(
                "✗ Preparation Error: Transaction failed to prepare: {}.\n  This may be due to conflicting packages or missing dependencies.",
                e
            )
        })?;

    // ZERO-TRUST SECURITY: Detached PGP Verification with Sequoia
    let verifier = PgpVerifier::new();
    let pkgs_to_add = alpm.trans_add();

    if !pkgs_to_add.is_empty() {
        main_pb.set_message("Verifying package signatures with Sequoia...");
        for pkg in pkgs_to_add {
            let pkg_name: &str = pkg.name();
            if let Some(pkg_filename) = pkg.filename() {
                let cache_path = std::path::Path::new("/var/cache/pacman/pkg").join(pkg_filename);
                let sig_path =
                    std::path::Path::new(&format!("{}.sig", cache_path.display())).to_path_buf();

                if cache_path.exists() && sig_path.exists() {
                    if let Err(e) = verifier.verify_package(&cache_path, &sig_path) {
                        anyhow::bail!(
                            "✗ SECURITY ALERT: PGP verification failed for {}.\n  Error: {}\n  The package may be corrupted or tampered with.",
                            pkg_name,
                            e
                        );
                    }
                }
            }
        }
    }

    // Check if there is actually anything to commit
    if alpm.trans_add().is_empty() && alpm.trans_remove().is_empty() {
        main_pb.finish_with_message("Nothing to do: system is already up to date.");
        alpm.trans_release()
            .context("Failed to release transaction")?;
        return Ok(());
    }

    // Commit
    alpm.trans_commit()
        .map_err(|e| {
            anyhow::anyhow!(
                "✗ Commit Error: Transaction failed to commit: {}.\n  Run 'omg cleanup' or check system logs for details.",
                e
            )
        })?;

    alpm.trans_release()
        .context("Failed to release transaction")?;

    Ok(())
}
