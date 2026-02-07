//! Direct libalpm operations - LIGHTNING FAST
//!
//! Pure libalpm transactions - no pacman subprocess.
//! Install/remove/update operations at native C library speed.

use std::sync::LazyLock;

use alpm_types::Version;
use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use regex::Regex;

use crate::core::paths;

/// Regex for parsing mirror server lines from /etc/pacman.d/mirrorlist
/// Compiled once at first use, then reused for all subsequent calls.
static MIRRORLIST_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Server\s*=\s*([^#]+)").expect("valid regex pattern"));
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
        let localdb = alpm.localdb();
        let syncdbs = alpm.syncdbs();
        let local_pkg_count = localdb.pkgs().len();

        // Build HashMap of sync packages: name -> (version_str, repo_name)
        // This converts O(n×m) lookups to O(n+m) with single HashMap lookup per package
        let mut sync_map: std::collections::HashMap<&str, (&str, &str)> =
            std::collections::HashMap::with_capacity(local_pkg_count);

        for db in syncdbs {
            let repo_name = db.name();
            for pkg in db.pkgs() {
                // First repo wins (core > extra > multilib priority)
                sync_map
                    .entry(pkg.name())
                    .or_insert_with(|| (pkg.version().as_str(), repo_name));
            }
        }

        let mut updates = Vec::with_capacity(local_pkg_count / 20); // ~5% typically have updates

        for pkg in localdb.pkgs() {
            let name = pkg.name();
            let local_ver_str = pkg.version().as_str();

            if let Some(&(sync_ver_str, repo)) = sync_map.get(name)
                && alpm::vercmp(sync_ver_str, local_ver_str) == std::cmp::Ordering::Greater
            {
                updates.push(UpdateInfo {
                    name: name.to_string(),
                    old_version: local_ver_str.to_string(),
                    new_version: sync_ver_str.to_string(),
                    repo: repo.to_string(),
                });
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
        let localdb = alpm.localdb();
        let syncdbs = alpm.syncdbs();
        let local_pkg_count = localdb.pkgs().len();
        let mut downloads = Vec::with_capacity(local_pkg_count / 20);

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
            && let Some(base) = filename.rsplitn(5, '-').last()
        {
            packages.entry(base.to_string()).or_default().push(path);
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
        let mut orphans = Vec::with_capacity(32);

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
    let result = crate::package_managers::alpm_direct::with_handle_mut(|alpm| {
        alpm.syncdbs_mut()
            .update(false)
            .map_err(|e| {
                anyhow::anyhow!(
                    "✗ Sync Error: Failed to update package databases: {e}.\n  Check your internet connection or run 'omg sync' with sudo."
                )
            })?;

        Ok(())
    });

    if result.is_ok() {
        crate::package_managers::alpm_direct::clear_alpm_cache();
    }
    result
}

/// Display package info beautifully
pub fn display_pkg_info(info: &PackageInfo) {
    // Use println! instead of tracing to avoid logs bleeding into output
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
        if let Err(e) = self.0.trans_release() {
            tracing::warn!("Failed to release ALPM transaction: {e}");
        }
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
        let main_pb = setup_alpm_callbacks(alpm, &mp);
        let tx_guard = prepare_alpm_transaction(alpm, packages, remove, sysupgrade)?;
        commit_alpm_transaction(tx_guard.0, &main_pb)?;
        return Ok(());
    }

    let root = paths::pacman_root().to_string_lossy().into_owned();
    let db_path = paths::pacman_db_dir().to_string_lossy().into_owned();
    let mut alpm =
        alpm::Alpm::new(root, db_path).context("Failed to initialize ALPM (are you root?)")?;

    let repos = crate::core::pacman_conf::get_configured_repos().unwrap_or_else(|_| {
        vec![
            "core".to_string(),
            "extra".to_string(),
            "multilib".to_string(),
        ]
    });

    for db_name in &repos {
        if let Err(e) = alpm.register_syncdb(db_name.as_str(), alpm::SigLevel::USE_DEFAULT) {
            tracing::warn!("Failed to register sync database '{db_name}': {e}");
        }
    }

    configure_mirrors(&mut alpm)?;

    let mp = indicatif::MultiProgress::new();
    let main_pb = setup_alpm_callbacks(&alpm, &mp);
    let tx_guard = prepare_alpm_transaction(&mut alpm, packages, remove, sysupgrade)?;
    commit_alpm_transaction(tx_guard.0, &main_pb)?;

    Ok(())
}

/// Setup ALPM callbacks for progress bars
#[allow(clippy::expect_used)] // ALPM database operations; failure indicates corrupted pacman database
fn setup_alpm_callbacks(
    alpm: &alpm::Alpm,
    mp: &indicatif::MultiProgress,
) -> indicatif::ProgressBar {
    let main_pb = mp.add(indicatif::ProgressBar::new(100));
    main_pb.set_style(
        indicatif::ProgressStyle::default_bar()
            .template("  {spinner:.cyan} {msg} {wide_bar:.cyan/blue} {percent}%")
            .expect("valid template")
            .progress_chars("█▓▒░ "),
    );
    main_pb.set_prefix("");

    alpm.set_question_cb((), |question, ()| match question.question() {
        alpm::Question::InstallIgnorepkg(mut q) => q.set_install(true),
        alpm::Question::Replace(q) => q.set_replace(true),
        alpm::Question::Conflict(mut q) => q.set_remove(true),
        alpm::Question::RemovePkgs(mut q) => q.set_skip(false),
        alpm::Question::SelectProvider(mut q) => q.set_index(0),
        alpm::Question::ImportKey(mut q) => {
            let fingerprint = q.fingerprint();
            let uid = q.uid();
            tracing::info!("PGP key required: {fingerprint} ({uid})");

            #[cfg(feature = "pgp")]
            {
                // Skip automatic key fetching during ALPM callbacks to avoid runtime issues
                // User should import keys manually before package operations
                tracing::warn!("PGP key not trusted: {fingerprint} ({uid})");
                tracing::info!("Import key manually: omg key import {fingerprint}");
                q.set_import(false);
            }

            #[cfg(not(feature = "pgp"))]
            {
                tracing::warn!("PGP feature disabled, cannot fetch key");
                q.set_import(false);
            }
        }
        alpm::Question::Corrupted(mut q) => {
            tracing::error!("Corrupted package detected! This may indicate tampering.");
            q.set_remove(false);
        }
    });

    // Suppress ALPM log messages (we show our own progress)
    alpm.set_log_cb((), |_level, _msg, ()| {
        // Intentionally empty - suppress all log output
    });

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

    let dl_pb_map = std::sync::Arc::new(dashmap::DashMap::<String, indicatif::ProgressBar>::new());
    let mp_clone = mp.clone();

    alpm.set_dl_cb(dl_pb_map, move |filename, event, map| {
        match event.event() {
            alpm::DownloadEvent::Init(_) => {
                if map.len() < 4 {
                    let pb = mp_clone.add(indicatif::ProgressBar::new_spinner());
                    pb.set_style(
                        indicatif::ProgressStyle::default_spinner()
                            .template("  {{spinner:.cyan}} {{msg:30}}")
                            .expect("valid template"),
                    );
                    pb.set_message(format!("⬇ {filename}"));
                    map.insert(filename.to_string(), pb);
                }
            }
            alpm::DownloadEvent::Progress(prog) => {
                if let Some(pb) = map.get(filename) {
                    if pb.length().is_none() && prog.total > 0 {
                        pb.set_length(prog.total as u64);
                        pb.set_style(
                            indicatif::ProgressStyle::default_bar()
                                .template("  {{spinner:.cyan}} {{msg:30}} {{bar:30.cyan/blue}} {{bytes}}/{{total_bytes}}")
                                .expect("valid template")
                                .progress_chars("█▓▒░ "),
                        );
                        pb.set_message(format!("⬇ {filename}"));
                    }
                    pb.set_position(prog.downloaded as u64);
                }
            }
            alpm::DownloadEvent::Retry(_) => {}
            alpm::DownloadEvent::Completed(_) => {
                if let Some((_, pb)) = map.remove(filename) {
                    pb.finish_and_clear();
                }
            }
        }
    });

    main_pb
}

/// Prepare an ALPM transaction for execution
fn prepare_alpm_transaction(
    alpm: &mut alpm::Alpm,
    packages: Vec<String>,
    remove: bool,
    sysupgrade: bool,
) -> Result<AlpmTransaction<'_>> {
    use alpm::TransFlag;

    let mut flags = TransFlag::NEEDED;
    if remove {
        flags |= TransFlag::RECURSE | TransFlag::UNNEEDED;
    }

    alpm.trans_init(flags).map_err(|e| match e {
        alpm::Error::HandleLock => {
            anyhow::anyhow!(
                "✗ Database is locked by another process.\n  \
                 → Check if pacman, yay, or another package manager is running.\n  \
                 → If no other process is running, remove: /var/lib/pacman/db.lck"
            )
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
                if pkg_name.contains(".pkg.tar.") || std::path::Path::new(&pkg_name).is_absolute() {
                    let canonical_path = std::fs::canonicalize(&pkg_name)
                        .context("Failed to canonicalize package path")?;
                    let canonical_str = canonical_path
                        .to_str()
                        .context("Package path contains invalid UTF-8")?;

                    let pkg = tx_guard
                        .0
                        .pkg_load(canonical_str.to_string(), true, alpm::SigLevel::USE_DEFAULT)
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
                    anyhow::bail!(
                        "✗ Package '{pkg_name}' not found in any configured repository.\n  \
                         → Run 'omg sync' to update package databases\n  \
                         → Search AUR: omg search {pkg_name}\n  \
                         → Check package name at: https://archlinux.org/packages/"
                    );
                }
            }
        }
    }

    Ok(tx_guard)
}

/// Commit an ALPM transaction
#[allow(clippy::expect_used)] // ALPM database operations; failure indicates corrupted pacman database
fn commit_alpm_transaction(alpm: &mut alpm::Alpm, main_pb: &indicatif::ProgressBar) -> Result<()> {
    main_pb.set_message("Preparing transaction...");

    alpm.trans_prepare().map_err(|e| {
        anyhow::anyhow!(
            "✗ Transaction preparation failed: {e}\n  \
             → This may be due to conflicting packages or missing dependencies.\n  \
             → Try running: omg update && omg install <package>\n  \
             → For more details: omg info <package>"
        )
    })?;

    #[cfg(feature = "pgp")]
    {
        use rayon::prelude::*;
        use std::sync::atomic::{AtomicBool, Ordering};

        let verifier = PgpVerifier::new();
        let pkgs_to_add = alpm.trans_add();

        if !pkgs_to_add.is_empty() {
            main_pb.set_message("Verifying package signatures in parallel...");

            let verification_data: Vec<_> = pkgs_to_add
                .iter()
                .filter_map(|pkg| {
                    pkg.filename().map(|filename| {
                        let cache_path = paths::pacman_cache_dir().join(filename);
                        let sig_path =
                            std::path::PathBuf::from(format!("{}.sig", cache_path.display()));
                        (pkg.name().to_string(), cache_path, sig_path)
                    })
                })
                .filter(|(_, cache_path, sig_path)| cache_path.exists() && sig_path.exists())
                .collect();

            let failed = AtomicBool::new(false);
            let failure_msg = std::sync::Mutex::new(String::new());

            verification_data.par_iter().for_each(|(pkg_name, cache_path, sig_path)| {
                if failed.load(Ordering::Relaxed) {
                    return;
                }
                if let Err(e) = verifier.verify_package(cache_path, sig_path) {
                    failed.store(true, Ordering::Relaxed);
                    // Use unwrap_or_else to recover from potential mutex poisoning
                    let mut msg = failure_msg.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
                    *msg = format!(
                        "✗ SECURITY ALERT: PGP verification failed for {pkg_name}.\n  Error: {e}\n  The package may be corrupted or tampered with."
                    );
                }
            });

            if failed.load(Ordering::Relaxed) {
                let msg = failure_msg
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                anyhow::bail!(msg);
            }
        }
    }

    if alpm.trans_add().is_empty() && alpm.trans_remove().is_empty() {
        main_pb.finish_and_clear();
        use owo_colors::OwoColorize;
        println!();
        println!(
            "  {} Nothing to do - system is up to date",
            "✓".green().bold()
        );
        println!();
        return Ok(());
    }

    main_pb.set_message("Finalizing...");
    alpm.trans_commit()
        .context("Transaction failed to commit. Run 'omg cleanup' if issue persists.")?;

    main_pb.finish_and_clear();

    Ok(())
}

/// Configure ALPM servers for all repos (official + custom)
#[allow(clippy::expect_used)]
fn configure_mirrors(alpm: &mut alpm::Alpm) -> Result<()> {
    let conf_path = paths::pacman_conf_path();
    let arch = std::env::consts::ARCH;

    if let Ok(config) = crate::core::pacman_conf::PacmanConfig::parse(&conf_path) {
        for repo in &config.repos {
            if let Ok(servers) = config.resolve_servers(repo, arch) {
                for db in alpm.syncdbs_mut() {
                    if db.name() == repo.name {
                        for server in servers {
                            if let Err(e) = db.add_server(server.clone()) {
                                tracing::debug!(
                                    "Failed to add server '{server}' to repo '{}': {e}",
                                    db.name()
                                );
                            }
                        }
                        break;
                    }
                }
            }
        }
        return Ok(());
    }

    let mirrorlist = paths::pacman_mirrorlist_path();
    if !mirrorlist.exists() {
        return Ok(());
    }

    let content = std::fs::read_to_string(mirrorlist)?;
    let mut servers = Vec::with_capacity(16);

    for line in content.lines() {
        let line = line.trim();
        if let Some(url) = MIRRORLIST_REGEX.captures(line).and_then(|caps| caps.get(1)) {
            servers.push(url.as_str().trim().to_string());
        }
    }

    for db in alpm.syncdbs_mut() {
        let db_name = db.name();
        for server in &servers {
            let url = server.replace("$repo", db_name).replace("$arch", arch);
            if let Err(e) = db.add_server(url.clone()) {
                tracing::debug!("Failed to add server '{url}' to repo '{db_name}': {e}");
            }
        }
    }
    Ok(())
}
