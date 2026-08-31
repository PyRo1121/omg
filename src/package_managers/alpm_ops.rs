//! Direct libalpm operations.
//!
//! Pure libalpm queries and transactions without spawning a pacman subprocess.

use std::sync::LazyLock;

use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use regex::Regex;

use crate::core::paths;

/// Regex for parsing mirror server lines from /etc/pacman.d/mirrorlist
/// Compiled once at first use, then reused for all subsequent calls.
static MIRRORLIST_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^Server\s*=\s*([^#]+)").expect("valid regex pattern"));
const DOWNLOAD_SPINNER_TEMPLATE: &str = "  {spinner:.cyan} {msg:30}";
/// Stable cross-layer marker for a requested package absent from every sync repository.
pub const MISSING_FROM_REPOS_MARKER: &str = "not found in any configured repository";
const DOWNLOAD_BAR_TEMPLATE: &str =
    "  {spinner:.cyan} {msg:30} {bar:30.cyan/blue} {bytes}/{total_bytes}";
const PARALLEL_DOWNLOADS: u32 = 5;
use crate::package_managers::types::{PackageInfo, UpdateInfo, contains_ignore_case};

/// Get comprehensive system status (counts + updates) in a single pass - FAST
pub fn get_system_status() -> Result<(usize, usize, usize, usize)> {
    let (total, explicit, orphans) = crate::package_managers::get_counts()?;
    let updates = crate::package_managers::check_updates_cached()?.len();
    Ok((total, explicit, orphans, updates))
}

/// Get detailed list of updates - FAST
/// Open a libalpm handle against the configured pacman root/db with a
/// canonical error context (audit typ01 C1: seven divergent inline copies).
#[cfg(feature = "arch")]
#[derive(Debug)]
pub(crate) struct LocalPackageMetadata {
    pub(crate) name: String,
    pub(crate) version: crate::package_managers::types::Version,
    pub(crate) installed_size: u64,
    pub(crate) license: Option<String>,
}

#[cfg(feature = "arch")]
pub(crate) fn load_local_package_metadata(path: &str) -> Result<LocalPackageMetadata> {
    let canonical = std::fs::canonicalize(path)
        .with_context(|| format!("Failed to resolve local package file {path}"))?;
    let canonical = canonical
        .to_str()
        .context("Local package path contains invalid UTF-8")?;
    let alpm = open_default_alpm()?;
    let package = alpm
        .pkg_load(canonical, false, alpm::SigLevel::NONE)
        .with_context(|| format!("Failed to read local package metadata from {path}"))?;
    let licenses: Vec<String> = package
        .licenses()
        .iter()
        .map(std::string::ToString::to_string)
        .collect();

    Ok(LocalPackageMetadata {
        name: package.name().to_string(),
        version: crate::package_managers::parse_version_or_zero(package.version().as_str()),
        installed_size: u64::try_from(package.isize()).unwrap_or(0),
        license: (!licenses.is_empty()).then(|| licenses.join(" AND ")),
    })
}

#[cfg(feature = "arch")]
pub fn open_default_alpm() -> anyhow::Result<alpm::Alpm> {
    use anyhow::Context as _;

    let root = crate::core::paths::pacman_root_result()?
        .to_string_lossy()
        .into_owned();
    let db_path = crate::core::paths::pacman_db_dir_result()?
        .to_string_lossy()
        .into_owned();
    alpm::Alpm::new(root, db_path).context("Failed to initialize ALPM")
}

pub fn get_update_list() -> Result<Vec<UpdateInfo>> {
    if crate::core::paths::test_mode() {
        let updates = crate::package_managers::pacman_db::check_updates_cached()?;
        return Ok(updates
            .into_iter()
            .map(|update| UpdateInfo {
                name: update.name,
                old_version: update.old_version.to_string(),
                new_version: update.new_version.to_string(),
                repo: update.repo,
            })
            .collect());
    }

    let pacman_config = crate::core::pacman_conf::PacmanConfig::parse(paths::pacman_conf_path())
        .context("Failed to load update filters from pacman.conf")?;

    crate::package_managers::alpm_direct::with_handle_mut(|alpm| {
        configure_package_filters(alpm, &pacman_config)?;
        Ok(collect_updates(alpm))
    })
}

/// Collect available updates from an ALPM handle whose ignore filters are
/// already configured.
///
/// Single authoritative update-collection implementation: shared by the CLI
/// (`get_update_list`) and the daemon worker (`alpm_worker`) so both honor
/// `IgnorePkg`/`IgnoreGroup`/`Replacement` handling identically.
pub(crate) fn collect_updates(alpm: &alpm::Alpm) -> Vec<UpdateInfo> {
    let localdb = alpm.localdb();
    let syncdbs = alpm.syncdbs();
    let local_pkg_count = localdb.pkgs().len();

    // Build HashMap of sync packages: name -> (version_str, repo_name)
    // This converts O(n×m) lookups to O(n+m) with single HashMap lookup per package
    let mut sync_map: ahash::AHashMap<&str, (&str, &str)> =
        ahash::AHashMap::with_capacity(local_pkg_count);

    for db in syncdbs {
        let repo_name = db.name();
        for pkg in db.pkgs() {
            if pkg.should_ignore() {
                continue;
            }
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

    updates
}

/// Get package info from sync DBs - INSTANT (<1ms)
pub fn get_sync_pkg_info(name: &str) -> Result<Option<PackageInfo>> {
    if paths::test_mode() {
        let manager = crate::package_managers::get_package_manager()?;
        let package = futures::executor::block_on(manager.info(name))?;
        if let Some(pkg) = package {
            return Ok(Some(PackageInfo {
                name: pkg.name,
                version: pkg.version,
                description: pkg.description,
                url: None,
                size: 0,
                install_size: None,
                download_size: None,
                repo: match pkg.source {
                    crate::core::PackageSource::Official => "official",
                    crate::core::PackageSource::Aur => "aur",
                }
                .to_string(),
                depends: Vec::new(),
                licenses: Vec::new(),
                installed: pkg.installed,
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
                // libalpm's size fields are i64; negative values from a
                // corrupt database must not wrap into huge u64 values.
                // https://doc.rust-lang.org/reference/expressions/operator-expr.html#numeric-cast
                size: u64::try_from(pkg.isize()).unwrap_or(0),
                install_size: Some(pkg.isize()),
                download_size: Some(u64::try_from(pkg.size()).unwrap_or(0)),
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

/// Extract the package base name from a pacman cache filename.
///
/// Cache files are named `{pkgname}-{version}-{release}-{arch}.pkg.tar.{zst,xz}`.
/// Neither `version` nor `release` may contain a dash (Arch packaging rules),
/// so exactly the last three dash-separated components are stripped; any
/// dashes inside the package name survive. Returns `None` for files without
/// the expected shape.
fn package_base_name(filename: &str) -> Option<&str> {
    let stem = filename
        .strip_suffix(".pkg.tar.zst")
        .or_else(|| filename.strip_suffix(".pkg.tar.xz"))?;
    // Strip exactly the trailing -arch, -release, -version components;
    // everything to the left (including any dashes in the pkgbase) stays.
    let (rest, _arch) = stem.rsplit_once('-')?;
    let (rest, _release) = rest.rsplit_once('-')?;
    let (name, version) = rest.rsplit_once('-')?;
    if name.is_empty() || version.is_empty() {
        return None;
    }
    Some(name)
}

/// Clean package cache using direct file system operations - FAST
pub fn clean_cache(keep_versions: usize) -> Result<(usize, u64)> {
    let mut packages: ahash::AHashMap<String, Vec<std::path::PathBuf>> = ahash::AHashMap::new();

    for cache_dir in paths::pacman_cache_dirs_result()? {
        if !cache_dir.exists() {
            continue;
        }
        for entry in std::fs::read_dir(&cache_dir)
            .with_context(|| format!("Failed to read pacman cache at {}", cache_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();

            if let Some(filename) = path.file_name().and_then(|name| name.to_str())
                && let Some(base) = package_base_name(filename)
            {
                packages.entry(base.to_string()).or_default().push(path);
            }
        }
    }

    let mut removed = 0;
    let mut freed = 0u64;

    for (_, mut versions) in packages {
        versions.sort_by(|a, b| {
            let a_time = a.metadata().and_then(|metadata| metadata.modified()).ok();
            let b_time = b.metadata().and_then(|metadata| metadata.modified()).ok();
            b_time.cmp(&a_time)
        });

        for old in versions.into_iter().skip(keep_versions) {
            // Only credit bytes that were actually freed; failures are
            // logged with their cause so callers are not told space was
            // reclaimed when it was not.
            freed += remove_cache_file_and_signature(&old, &mut removed);
        }
    }

    Ok((removed, freed))
}

/// Remove one cache archive plus its `.sig` companion, returning the bytes
/// actually freed. The detached signature is optional: its absence is normal
/// for unsigned custom packages and is not an error.
fn remove_cache_file_and_signature(archive: &std::path::Path, removed: &mut usize) -> u64 {
    let mut freed = 0u64;

    let archive_len = std::fs::metadata(archive)
        .map(|metadata| metadata.len())
        .ok();
    match std::fs::remove_file(archive) {
        Ok(()) => {
            *removed += 1;
            freed = freed.saturating_add(archive_len.unwrap_or(0));
        }
        Err(error) => tracing::warn!("Failed to remove cache file {}: {error}", archive.display()),
    }

    let signature = std::path::PathBuf::from(format!("{}.sig", archive.display()));
    let sig_len = std::fs::metadata(&signature)
        .map(|metadata| metadata.len())
        .ok();
    match std::fs::remove_file(&signature) {
        Ok(()) => {
            *removed += 1;
            freed = freed.saturating_add(sig_len.unwrap_or(0));
        }
        Err(error) if signature.exists() => {
            tracing::warn!(
                "Failed to remove cache signature {}: {error}",
                signature.display()
            );
        }
        Err(_) => {}
    }

    freed
}

/// List orphaned packages - INSTANT
pub use crate::package_managers::alpm_direct::list_orphans_fast as list_orphans_direct;

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
#[must_use]
struct AlpmTransaction<'a>(&'a mut alpm::Alpm);

impl Drop for AlpmTransaction<'_> {
    fn drop(&mut self) {
        if let Err(e) = self.0.trans_release() {
            tracing::warn!("Failed to release ALPM transaction: {e}");
        }
    }
}

/// Requested ALPM transaction behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionKind {
    Install,
    /// Install archives produced by OMG's AUR build pipeline. AUR artifacts
    /// are normally unsigned, so verify a detached signature when present but
    /// do not require one, matching pacman's default local-file policy.
    InstallAurArtifact,
    Remove {
        recursive: bool,
    },
    SystemUpgrade,
}

/// Execute a libalpm transaction.
///
/// The `handle` parameter exists for callers that already own a configured
/// ALPM handle (e.g. an elevated fast path); every current caller passes
/// [`None`], which creates and configures a fresh handle. NOTE(wave2): the
/// `Some(handle)` branch skips repository registration that the `None`
/// branch performs — if you ever pass a handle, it must already have sync
/// databases registered.
pub fn execute_transaction(
    packages: Vec<String>,
    kind: TransactionKind,
    handle: Option<&mut alpm::Alpm>,
) -> Result<()> {
    let pacman_config = crate::core::pacman_conf::PacmanConfig::parse(paths::pacman_conf_path())
        .context("Failed to load transaction options from pacman.conf")?;

    if let Some(alpm) = handle {
        configure_transaction_options(alpm, &pacman_config)?;
        configure_mirrors(alpm)?;
        let mp = indicatif::MultiProgress::new();
        let main_pb = setup_alpm_callbacks(alpm, &mp);
        let tx_guard = prepare_alpm_transaction(alpm, packages, kind, &pacman_config)?;
        commit_alpm_transaction(tx_guard.0, &main_pb, kind, &pacman_config.hold_pkg)?;
        return Ok(());
    }

    let mut alpm = open_default_alpm()?;
    configure_transaction_options(&mut alpm, &pacman_config)?;

    if pacman_config.repos.is_empty() {
        anyhow::bail!("pacman configuration contains no repositories");
    }

    register_configured_syncdbs(&alpm, &pacman_config)?;

    configure_mirrors(&mut alpm)?;

    let mp = indicatif::MultiProgress::new();
    let main_pb = setup_alpm_callbacks(&alpm, &mp);
    let tx_guard = prepare_alpm_transaction(&mut alpm, packages, kind, &pacman_config)?;
    commit_alpm_transaction(tx_guard.0, &main_pb, kind, &pacman_config.hold_pkg)?;

    Ok(())
}

pub(crate) fn register_configured_syncdbs(
    alpm: &alpm::Alpm,
    pacman_config: &crate::core::pacman_conf::PacmanConfig,
) -> Result<()> {
    let policy = signature_policy(pacman_config)?;
    for repo in &pacman_config.repos {
        let siglevel = repository_siglevel(policy.default, repo.sig_level.as_deref())?;
        alpm.register_syncdb(repo.name.as_str(), siglevel)
            .with_context(|| {
                format!(
                    "Failed to register configured sync database '{}'; refusing a partial repository set",
                    repo.name
                )
            })?;
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForwardedAlpmLogLevel {
    Error,
    Warning,
    Debug,
    Trace,
}

fn classify_alpm_log_level(level: alpm::LogLevel) -> ForwardedAlpmLogLevel {
    if level.contains(alpm::LogLevel::ERROR) {
        ForwardedAlpmLogLevel::Error
    } else if level.contains(alpm::LogLevel::WARNING) {
        ForwardedAlpmLogLevel::Warning
    } else if level.contains(alpm::LogLevel::DEBUG) {
        ForwardedAlpmLogLevel::Debug
    } else {
        ForwardedAlpmLogLevel::Trace
    }
}

fn forward_alpm_log(level: alpm::LogLevel, message: &str) {
    let message = crate::cli::style::sanitize_terminal_text(message.trim());
    if message.is_empty() {
        return;
    }
    match classify_alpm_log_level(level) {
        ForwardedAlpmLogLevel::Error => tracing::error!(target: "libalpm", "{message}"),
        ForwardedAlpmLogLevel::Warning => tracing::warn!(target: "libalpm", "{message}"),
        ForwardedAlpmLogLevel::Debug => tracing::debug!(target: "libalpm", "{message}"),
        ForwardedAlpmLogLevel::Trace => tracing::trace!(target: "libalpm", "{message}"),
    }
}

/// Setup ALPM callbacks for progress bars
#[expect(clippy::expect_used)] // ALPM database operations; failure indicates corrupted pacman database
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
        alpm::Question::InstallIgnorepkg(mut q) => {
            tracing::warn!("Installing explicitly requested package despite IgnorePkg");
            q.set_install(true);
        }
        alpm::Question::Replace(q) => {
            tracing::warn!(
                "Replacing {} with {}/{} as part of the confirmed transaction",
                q.oldpkg().name(),
                q.newdb().name(),
                q.newpkg().name()
            );
            q.set_replace(true);
        }
        alpm::Question::Conflict(mut q) => {
            let conflict = q.conflict();
            tracing::error!(
                "Refusing implicit removal of conflicting package {} while installing {} ({})",
                conflict.package2().name(),
                conflict.package1().name(),
                conflict.reason()
            );
            // Match pacman's fail-closed [y/N] default. A high-level explicit
            // conflict-resolution contract is required before this may remove
            // a package the user did not request.
            q.set_remove(false);
        }
        alpm::Question::RemovePkgs(mut q) => q.set_skip(false),
        alpm::Question::SelectProvider(mut q) => q.set_index(0),
        alpm::Question::ImportKey(mut q) => {
            let fingerprint = q.fingerprint();
            let uid = q.uid();
            tracing::info!("PGP key required: {fingerprint} ({uid})");

            // Never fetch keys from inside an ALPM callback: the user should
            // import and trust keys deliberately before package operations.
            tracing::warn!("PGP key not trusted: {fingerprint} ({uid})");
            tracing::info!("Import key manually: omg key import {fingerprint}");
            q.set_import(false);
        }
        alpm::Question::Corrupted(mut q) => {
            tracing::error!("Corrupted package detected! This may indicate tampering.");
            q.set_remove(false);
        }
    });

    // Progress messages are rendered below, but warnings and errors such as
    // .pacnew/.pacsave notices remain operationally significant.
    alpm.set_log_cb((), |level, message, ()| {
        forward_alpm_log(level, message);
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
        main_pb_clone.set_position(u64::try_from(percent).unwrap_or(0));
    });

    let dl_pb_map = std::sync::Arc::new(dashmap::DashMap::<String, indicatif::ProgressBar>::new());
    let mp_clone = mp.clone();

    alpm.set_dl_cb(dl_pb_map, move |filename, event, map| match event.event() {
        alpm::DownloadEvent::Init(_) => {
            if map.len() < usize::try_from(PARALLEL_DOWNLOADS).unwrap_or(usize::MAX) {
                let pb = mp_clone.add(indicatif::ProgressBar::new_spinner());
                pb.set_style(
                    indicatif::ProgressStyle::default_spinner()
                        .template(DOWNLOAD_SPINNER_TEMPLATE)
                        .expect("valid template"),
                );
                pb.set_message(format!("⬇ {filename}"));
                map.insert(filename.to_string(), pb);
            }
        }
        alpm::DownloadEvent::Progress(prog) => {
            if let Some(pb) = map.get(filename) {
                if pb.length().is_none() && prog.total > 0 {
                    pb.set_length(u64::try_from(prog.total).unwrap_or(0));
                    pb.set_style(
                        indicatif::ProgressStyle::default_bar()
                            .template(DOWNLOAD_BAR_TEMPLATE)
                            .expect("valid template")
                            .progress_chars("█▓▒░ "),
                    );
                    pb.set_message(format!("⬇ {filename}"));
                }
                pb.set_position(u64::try_from(prog.downloaded).unwrap_or(0));
            }
        }
        alpm::DownloadEvent::Retry(_) => {}
        alpm::DownloadEvent::Completed(_) => {
            if let Some((_, pb)) = map.remove(filename) {
                pb.finish_and_clear();
            }
        }
    });

    main_pb
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SignaturePolicy {
    pub(crate) default: alpm::SigLevel,
    pub(crate) local_file: alpm::SigLevel,
    pub(crate) remote_file: alpm::SigLevel,
}

#[derive(Debug, Clone, Copy)]
struct ParsedSignatureLevel {
    level: alpm::SigLevel,
    mask: alpm::SigLevel,
}

fn set_signature_flags(parsed: &mut ParsedSignatureLevel, flags: alpm::SigLevel) {
    parsed.level.insert(flags);
    parsed.mask.insert(flags);
}

fn unset_signature_flags(parsed: &mut ParsedSignatureLevel, flags: alpm::SigLevel) {
    parsed.level.remove(flags);
    parsed.mask.insert(flags);
}

fn parse_signature_level(
    value: Option<&str>,
    initial: alpm::SigLevel,
) -> Result<ParsedSignatureLevel> {
    let mut parsed = ParsedSignatureLevel {
        level: initial,
        mask: alpm::SigLevel::NONE,
    };
    let Some(value) = value else {
        return Ok(parsed);
    };

    for original in value.split_whitespace() {
        let (package, database, directive) = if let Some(value) = original.strip_prefix("Package") {
            (true, false, value)
        } else if let Some(value) = original.strip_prefix("Database") {
            (false, true, value)
        } else {
            (true, true, original)
        };

        let package_required = alpm::SigLevel::PACKAGE;
        let package_optional = alpm::SigLevel::PACKAGE_OPTIONAL;
        let package_trust =
            alpm::SigLevel::PACKAGE_MARGINAL_OK | alpm::SigLevel::PACKAGE_UNKNOWN_OK;
        let database_required = alpm::SigLevel::DATABASE;
        let database_optional = alpm::SigLevel::DATABASE_OPTIONAL;
        let database_trust =
            alpm::SigLevel::DATABASE_MARGINAL_OK | alpm::SigLevel::DATABASE_UNKNOWN_OK;

        match directive {
            "Never" => {
                if package {
                    unset_signature_flags(&mut parsed, package_required);
                }
                if database {
                    unset_signature_flags(&mut parsed, database_required);
                }
            }
            "Optional" => {
                if package {
                    set_signature_flags(&mut parsed, package_required | package_optional);
                }
                if database {
                    set_signature_flags(&mut parsed, database_required | database_optional);
                }
            }
            "Required" => {
                if package {
                    set_signature_flags(&mut parsed, package_required);
                    unset_signature_flags(&mut parsed, package_optional);
                }
                if database {
                    set_signature_flags(&mut parsed, database_required);
                    unset_signature_flags(&mut parsed, database_optional);
                }
            }
            "TrustedOnly" => {
                if package {
                    unset_signature_flags(&mut parsed, package_trust);
                }
                if database {
                    unset_signature_flags(&mut parsed, database_trust);
                }
            }
            "TrustAll" => {
                if package {
                    set_signature_flags(&mut parsed, package_trust);
                }
                if database {
                    set_signature_flags(&mut parsed, database_trust);
                }
            }
            _ => anyhow::bail!("Invalid pacman SigLevel directive '{original}'"),
        }
        parsed.level.remove(alpm::SigLevel::USE_DEFAULT);
    }
    Ok(parsed)
}

fn merge_signature_level(
    base: alpm::SigLevel,
    override_level: ParsedSignatureLevel,
) -> alpm::SigLevel {
    if override_level.mask.is_empty() {
        if override_level.level.contains(alpm::SigLevel::USE_DEFAULT) {
            base
        } else {
            override_level.level
        }
    } else {
        (override_level.level & override_level.mask) | (base & !override_level.mask)
    }
}

pub(crate) fn signature_policy(
    config: &crate::core::pacman_conf::PacmanConfig,
) -> Result<SignaturePolicy> {
    let default = parse_signature_level(
        config.sig_level.as_deref(),
        alpm::SigLevel::PACKAGE | alpm::SigLevel::DATABASE,
    )?
    .level;
    let local_file = merge_signature_level(
        default,
        parse_signature_level(
            config.local_file_sig_level.as_deref(),
            alpm::SigLevel::USE_DEFAULT,
        )?,
    );
    let remote_file = merge_signature_level(
        default,
        parse_signature_level(
            config.remote_file_sig_level.as_deref(),
            alpm::SigLevel::USE_DEFAULT,
        )?,
    );
    Ok(SignaturePolicy {
        default,
        local_file,
        remote_file,
    })
}

pub(crate) fn repository_siglevel(
    default: alpm::SigLevel,
    configured: Option<&str>,
) -> Result<alpm::SigLevel> {
    Ok(merge_signature_level(
        default,
        parse_signature_level(configured, alpm::SigLevel::USE_DEFAULT)?,
    ))
}

pub(crate) fn configure_signature_policy(
    alpm: &alpm::Alpm,
    config: &crate::core::pacman_conf::PacmanConfig,
) -> Result<SignaturePolicy> {
    let signatures = signature_policy(config)?;
    alpm.set_default_siglevel(signatures.default)
        .context("Failed to configure default package signature policy")?;
    alpm.set_local_file_siglevel(signatures.local_file)
        .context("Failed to configure local package signature policy")?;
    alpm.set_remote_file_siglevel(signatures.remote_file)
        .context("Failed to configure remote package signature policy")?;
    Ok(signatures)
}

fn local_package_siglevel(kind: TransactionKind, configured: alpm::SigLevel) -> alpm::SigLevel {
    if kind == TransactionKind::InstallAurArtifact {
        configured & (alpm::SigLevel::PACKAGE_MARGINAL_OK | alpm::SigLevel::PACKAGE_UNKNOWN_OK)
            | alpm::SigLevel::PACKAGE
            | alpm::SigLevel::PACKAGE_OPTIONAL
    } else {
        configured
    }
}

fn validate_transaction_targets(kind: TransactionKind, packages: &[String]) -> Result<()> {
    anyhow::ensure!(
        kind != TransactionKind::SystemUpgrade || packages.is_empty(),
        "System upgrade transactions do not accept explicit package targets"
    );
    Ok(())
}

fn transaction_flags(kind: TransactionKind) -> alpm::TransFlag {
    let mut flags = alpm::TransFlag::NEEDED;
    if matches!(kind, TransactionKind::Remove { recursive: true }) {
        // Match `pacman -Rs`: recurse into dependencies that become unneeded,
        // but do not set `UNNEEDED` (`pacman -Ru`), which would silently drop
        // still-required explicit targets from the transaction.
        flags |= alpm::TransFlag::RECURSE;
    }
    flags
}

/// Prepare an ALPM transaction for execution
fn prepare_alpm_transaction<'a>(
    alpm: &'a mut alpm::Alpm,
    packages: Vec<String>,
    kind: TransactionKind,
    pacman_config: &crate::core::pacman_conf::PacmanConfig,
) -> Result<AlpmTransaction<'a>> {
    validate_transaction_targets(kind, &packages)?;
    alpm.trans_init(transaction_flags(kind))
        .map_err(|e| match e {
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

    if kind == TransactionKind::SystemUpgrade {
        tx_guard
            .0
            .sync_sysupgrade(false)
            .context("Failed to setup sysupgrade")?;
    } else {
        for pkg_name in packages {
            if matches!(kind, TransactionKind::Remove { .. }) {
                ensure_removals_not_held(
                    std::iter::once(pkg_name.as_str()),
                    &pacman_config.hold_pkg,
                )?;
                if let Ok(pkg) = tx_guard.0.localdb().pkg(pkg_name.as_str()) {
                    tx_guard.0.trans_remove_pkg(pkg).map_err(|e| {
                        anyhow::anyhow!("Failed to add {pkg_name} to removal list: {e}")
                    })?;
                } else {
                    tracing::warn!("Package '{pkg_name}' is not installed; skipping removal");
                }
            } else {
                if crate::core::security::is_local_package_file(&pkg_name) {
                    let canonical_path =
                        crate::core::security::validate_local_package_file(&pkg_name)?;
                    let canonical_str = canonical_path
                        .to_str()
                        .context("Package path contains invalid UTF-8")?;

                    let pkg = tx_guard
                        .0
                        .pkg_load(
                            canonical_str.to_string(),
                            true,
                            local_package_siglevel(
                                kind,
                                signature_policy(pacman_config)?.local_file,
                            ),
                        )
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
                    if let Ok(pkg) = db.pkg(pkg_name.as_str()) {
                        tx_guard.0.trans_add_pkg(pkg).map_err(|e| {
                            anyhow::anyhow!("Failed to add {pkg_name} to installation list: {e}")
                        })?;
                        found = true;
                        break;
                    }
                }
                if !found {
                    anyhow::bail!(
                        "✗ Package '{pkg_name}' {MISSING_FROM_REPOS_MARKER}.\n  \
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
fn pacman_option_path(
    root: &std::path::Path,
    configured: Option<&str>,
    default: &str,
) -> std::path::PathBuf {
    let path = configured.map_or_else(
        || std::path::PathBuf::from(default),
        std::path::PathBuf::from,
    );
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}

fn configure_transaction_options(
    alpm: &mut alpm::Alpm,
    pacman_config: &crate::core::pacman_conf::PacmanConfig,
) -> Result<()> {
    // `Alpm::new` leaves transaction-critical pacman options empty. Configure
    // them explicitly so downloads, signature verification, architecture
    // checks, logging, and package hooks behave like a normal Arch transaction.
    let root = paths::pacman_root_result()?;
    let cache_dirs = paths::pacman_cache_dirs_result()?;
    for cache_dir in &cache_dirs {
        std::fs::create_dir_all(cache_dir).with_context(|| {
            format!(
                "Failed to create pacman package cache at {}",
                cache_dir.display()
            )
        })?;
    }
    let cache_dirs = cache_dirs
        .iter()
        .map(|cache_dir| {
            cache_dir
                .to_str()
                .context("Pacman package cache path contains invalid UTF-8")
        })
        .collect::<Result<Vec<_>>>()?;
    alpm.set_cachedirs(cache_dirs.into_iter())
        .context("Failed to configure pacman package caches")?;

    let gpg_dir = pacman_option_path(
        &root,
        pacman_config.gpg_dir.as_deref(),
        "etc/pacman.d/gnupg",
    );
    if !gpg_dir.is_dir() {
        anyhow::bail!(
            "Pacman keyring is unavailable at {}. Initialize it before installing packages.",
            gpg_dir.display()
        );
    }
    let gpg_dir = gpg_dir
        .to_str()
        .context("Pacman keyring path contains invalid UTF-8")?;
    alpm.set_gpgdir(gpg_dir)
        .context("Failed to configure pacman keyring")?;

    let log_path = pacman_option_path(
        &root,
        pacman_config.log_file.as_deref(),
        "var/log/pacman.log",
    );
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create pacman log directory at {}",
                parent.display()
            )
        })?;
    }
    let log_path = log_path
        .to_str()
        .context("Pacman log path contains invalid UTF-8")?;
    alpm.set_logfile(log_path)
        .context("Failed to configure pacman transaction log")?;

    // libalpm gives later hook directories precedence for duplicate hook
    // names. Distribution hooks come first, followed by configured overrides.
    let mut hook_dirs = vec![root.join("usr/share/libalpm/hooks")];
    hook_dirs.extend(
        pacman_config
            .hook_dirs
            .iter()
            .map(|path| pacman_option_path(&root, Some(path), path)),
    );
    let hook_dirs = hook_dirs
        .iter()
        .filter(|path| path.is_dir())
        .map(|path| {
            path.to_str()
                .context("Pacman hook path contains invalid UTF-8")
        })
        .collect::<Result<Vec<_>>>()?;
    alpm.set_hookdirs(hook_dirs.into_iter())
        .context("Failed to configure pacman hooks")?;

    let architectures = pacman_config
        .architecture
        .as_deref()
        .unwrap_or("auto")
        .split_whitespace()
        .map(|architecture| {
            if architecture == "auto" {
                std::env::consts::ARCH
            } else {
                architecture
            }
        });
    alpm.set_architectures(architectures)
        .context("Failed to configure package architecture validation")?;
    alpm.set_check_space(true);
    // Native libalpm parallel downloads during transaction commit. Mirrors
    // pacman's own `ParallelDownloads = 5` default; pacman.conf plumbing can
    // be added once the shared parser exposes the option.
    alpm.set_parallel_downloads(PARALLEL_DOWNLOADS);
    configure_package_filters(alpm, pacman_config)?;
    alpm.set_noupgrades(pacman_config.no_upgrade.iter())
        .context("Failed to configure protected upgrade paths")?;
    alpm.set_noextracts(pacman_config.no_extract.iter())
        .context("Failed to configure excluded extraction paths")?;

    configure_signature_policy(alpm, pacman_config).map(|_| ())
}

pub(crate) fn configure_package_filters(
    alpm: &mut alpm::Alpm,
    pacman_config: &crate::core::pacman_conf::PacmanConfig,
) -> Result<()> {
    alpm.set_ignorepkgs(pacman_config.ignore_pkg.iter())
        .context("Failed to configure ignored packages")?;
    alpm.set_ignoregroups(pacman_config.ignore_group.iter())
        .context("Failed to configure ignored package groups")
}

fn ensure_removals_not_held<'a>(
    package_names: impl IntoIterator<Item = &'a str>,
    hold_pkg: &[String],
) -> Result<()> {
    let matchers = hold_pkg
        .iter()
        .map(|pattern| {
            globset::Glob::new(pattern)
                .with_context(|| format!("Invalid HoldPkg pattern '{pattern}' in pacman.conf"))
                .map(|glob| (pattern, glob.compile_matcher()))
        })
        .collect::<Result<Vec<_>>>()?;

    for package_name in package_names {
        if let Some((pattern, _)) = matchers
            .iter()
            .find(|(_, matcher)| matcher.is_match(package_name))
        {
            anyhow::bail!(
                "Package '{package_name}' is protected by HoldPkg pattern '{pattern}' in pacman.conf and cannot be removed"
            );
        }
    }
    Ok(())
}

fn commit_alpm_transaction(
    alpm: &mut alpm::Alpm,
    main_pb: &indicatif::ProgressBar,
    kind: TransactionKind,
    hold_pkg: &[String],
) -> Result<()> {
    main_pb.set_message("Preparing transaction...");

    alpm.trans_prepare()
        .map_err(|e| anyhow::anyhow!(format_trans_prepare_error(&e.to_string())))?;

    // RECURSE expands the removal set during preparation. Validate the final
    // set, not only the user's explicit targets, so dependencies protected by
    // HoldPkg cannot be removed as a cascade.
    if matches!(kind, TransactionKind::Remove { .. }) {
        ensure_removals_not_held(
            alpm.trans_remove()
                .into_iter()
                .map(|package| package.name()),
            hold_pkg,
        )?;
    }

    if alpm.trans_add().is_empty() && alpm.trans_remove().is_empty() {
        main_pb.finish_and_clear();
        use owo_colors::OwoColorize;
        println!();
        // Deliberately neutral wording: this also fires when every requested
        // removal target was already absent, where "system is up to date"
        // would be misleading.
        println!("  {} Nothing to do", "✓".green().bold());
        println!();
        return Ok(());
    }

    main_pb.set_message("Finalizing...");
    alpm.trans_commit()
        .context("Transaction failed to commit. Run 'omg cleanup' if issue persists.")?;

    main_pb.finish_and_clear();

    Ok(())
}

fn is_keyring_related_error(err: &str) -> bool {
    ["keyring", "signature", "pgp", "corrupt"]
        .iter()
        .any(|keyword| contains_ignore_case(err, keyword))
}

fn format_trans_prepare_error(err: &str) -> String {
    if is_keyring_related_error(err) {
        return format!(
            "✗ Transaction preparation failed: {err}\n  \
             → Arch keyring/signature validation appears unhealthy.\n  \
             → Repair: sudo pacman -Sy archlinux-keyring\n  \
             → Reinitialize keys: sudo pacman-key --init && sudo pacman-key --populate archlinux\n  \
             → Retry: omg sync && omg install <package>"
        );
    }

    format!(
        "✗ Transaction preparation failed: {err}\n  \
         → This may be due to conflicting packages or missing dependencies.\n  \
         → Try running: omg update && omg install <package>\n  \
         → For more details: omg info <package>"
    )
}

/// Configure ALPM servers for all repos (official + custom)
fn configure_mirrors(alpm: &mut alpm::Alpm) -> Result<()> {
    let conf_path = paths::pacman_conf_path();
    let arch = std::env::consts::ARCH;

    if let Ok(config) = crate::core::pacman_conf::PacmanConfig::parse(&conf_path) {
        for repo in &config.repos {
            match config.resolve_servers(repo, arch) {
                Ok(servers) => {
                    // AlpmListMut is an IntoIterator, not an Iterator.
                    let target_db = alpm
                        .syncdbs_mut()
                        .into_iter()
                        .find(|db| db.name() == repo.name);
                    if let Some(db) = target_db {
                        for server in servers {
                            if let Err(e) = db.add_server(server.clone()) {
                                tracing::debug!(
                                    "Failed to add server '{server}' to repo '{}': {e}",
                                    db.name()
                                );
                            }
                        }
                    } else {
                        tracing::debug!(
                            "Repository '{}' from pacman.conf has no registered sync database",
                            repo.name
                        );
                    }
                }
                Err(error) => tracing::debug!(
                    "Failed to resolve mirrors for repository '{}': {error}",
                    repo.name
                ),
            }
        }
        return ensure_mirror_servers(alpm);
    }

    let mirrorlist = paths::pacman_mirrorlist_path();
    if !mirrorlist.exists() {
        return ensure_mirror_servers(alpm);
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
                tracing::debug!(
                    "Failed to add server '{}' to repo '{db_name}': {e}",
                    crate::core::http::redact_url(&url)
                );
            }
        }
    }
    ensure_mirror_servers(alpm)
}

fn ensure_mirror_servers(alpm: &alpm::Alpm) -> Result<()> {
    anyhow::ensure!(
        alpm.syncdbs()
            .into_iter()
            .any(|database| !database.servers().is_empty()),
        "No usable pacman mirror servers are configured"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        DOWNLOAD_BAR_TEMPLATE, DOWNLOAD_SPINNER_TEMPLATE, ForwardedAlpmLogLevel, TransactionKind,
        classify_alpm_log_level, configure_signature_policy, ensure_mirror_servers,
        ensure_removals_not_held, format_trans_prepare_error, is_keyring_related_error,
        local_package_siglevel, package_base_name, register_configured_syncdbs,
        repository_siglevel, signature_policy, transaction_flags, validate_transaction_targets,
    };

    #[test]
    fn alpm_warnings_and_errors_are_not_classified_as_debug_output() {
        assert_eq!(
            classify_alpm_log_level(alpm::LogLevel::ERROR | alpm::LogLevel::WARNING),
            ForwardedAlpmLogLevel::Error
        );
        assert_eq!(
            classify_alpm_log_level(alpm::LogLevel::WARNING),
            ForwardedAlpmLogLevel::Warning
        );
        assert_eq!(
            classify_alpm_log_level(alpm::LogLevel::DEBUG),
            ForwardedAlpmLogLevel::Debug
        );
        assert_eq!(
            classify_alpm_log_level(alpm::LogLevel::FUNCTION),
            ForwardedAlpmLogLevel::Trace
        );
    }

    #[test]
    fn transaction_registration_rejects_partial_repository_sets() {
        let database_path = tempfile::tempdir().expect("temporary database path");
        let database_path = database_path.path().to_string_lossy();
        let alpm = alpm::Alpm::new("/", database_path.as_ref()).expect("ALPM handle");
        alpm.register_syncdb("core", alpm::SigLevel::NONE)
            .expect("initial sync database");
        let config = crate::core::pacman_conf::PacmanConfig {
            repos: vec![crate::core::pacman_conf::RepoConfig {
                name: "core".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };

        let error = register_configured_syncdbs(&alpm, &config)
            .expect_err("a failed configured repository must abort registration");
        assert!(
            error
                .to_string()
                .contains("refusing a partial repository set")
        );
    }

    #[test]
    fn mirror_configuration_requires_at_least_one_usable_server() {
        let database_path = tempfile::tempdir().expect("temporary database path");
        let database_path = database_path.path().to_string_lossy();
        let mut alpm = alpm::Alpm::new("/", database_path.as_ref()).expect("ALPM handle");
        alpm.register_syncdb_mut("core", alpm::SigLevel::NONE)
            .expect("register sync database");

        assert!(ensure_mirror_servers(&alpm).is_err());
        alpm.syncdbs_mut()
            .into_iter()
            .next()
            .expect("registered sync database")
            .add_server("https://mirror.example/$repo/os/$arch")
            .expect("add test mirror");
        assert!(ensure_mirror_servers(&alpm).is_ok());
    }

    #[test]
    fn pacman_signature_policy_is_applied_to_handle_and_repositories() {
        let config = crate::core::pacman_conf::PacmanConfig::parse_str(
            "[options]\nSigLevel = Required DatabaseOptional\nLocalFileSigLevel = PackageOptional\nRemoteFileSigLevel = PackageNever\n\n[core]\nSigLevel = PackageOptional DatabaseNever\n",
        )
        .expect("signature configuration");
        let policy = signature_policy(&config).expect("parsed signature policy");

        assert!(policy.default.contains(alpm::SigLevel::PACKAGE));
        assert!(policy.default.contains(alpm::SigLevel::DATABASE));
        assert!(policy.default.contains(alpm::SigLevel::DATABASE_OPTIONAL));
        assert!(policy.local_file.contains(alpm::SigLevel::PACKAGE));
        assert!(policy.local_file.contains(alpm::SigLevel::PACKAGE_OPTIONAL));
        assert!(!policy.remote_file.contains(alpm::SigLevel::PACKAGE));

        let repository = repository_siglevel(policy.default, config.repos[0].sig_level.as_deref())
            .expect("repository signature policy");
        assert!(repository.contains(alpm::SigLevel::PACKAGE));
        assert!(repository.contains(alpm::SigLevel::PACKAGE_OPTIONAL));
        assert!(!repository.contains(alpm::SigLevel::DATABASE));

        let database_path = tempfile::tempdir().expect("temporary database path");
        let database_path = database_path.path().to_string_lossy();
        let alpm = alpm::Alpm::new("/", database_path.as_ref()).expect("ALPM handle");
        configure_signature_policy(&alpm, &config).expect("configure ALPM policy");
        assert_eq!(alpm.default_siglevel(), policy.default);
        assert_eq!(alpm.local_file_siglevel(), policy.local_file);
        assert_eq!(alpm.remote_file_siglevel(), policy.remote_file);
    }

    #[test]
    fn invalid_pacman_signature_policy_fails_closed() {
        let config = crate::core::pacman_conf::PacmanConfig::parse_str(
            "[options]\nSigLevel = PackageSometimes\n",
        )
        .expect("syntax parser preserves policy text");
        let error = signature_policy(&config).expect_err("unknown policy must fail");
        assert!(error.to_string().contains("PackageSometimes"), "{error:#}");
    }

    #[test]
    fn aur_artifacts_allow_missing_signatures_but_verify_present_ones() {
        let configured = alpm::SigLevel::PACKAGE | alpm::SigLevel::PACKAGE_UNKNOWN_OK;
        let regular = local_package_siglevel(TransactionKind::Install, configured);
        assert_eq!(regular, configured);

        let aur = local_package_siglevel(TransactionKind::InstallAurArtifact, configured);
        assert!(aur.contains(alpm::SigLevel::PACKAGE));
        assert!(aur.contains(alpm::SigLevel::PACKAGE_OPTIONAL));
    }

    #[test]
    fn hold_pkg_patterns_cover_explicit_and_cascade_removals() {
        let patterns = vec!["linux".to_string(), "kde-*".to_string()];

        ensure_removals_not_held(["bash"], &patterns).expect("unheld package");
        let explicit = ensure_removals_not_held(["linux"], &patterns)
            .expect_err("explicit HoldPkg entry must be rejected");
        assert!(explicit.to_string().contains("HoldPkg pattern 'linux'"));
        let cascade = ensure_removals_not_held(["kde-libs"], &patterns)
            .expect_err("glob-protected cascade must be rejected");
        assert!(cascade.to_string().contains("HoldPkg pattern 'kde-*'"));
    }

    #[test]
    fn invalid_hold_pkg_patterns_fail_closed() {
        let error = ensure_removals_not_held(["anything"], &["[".to_string()])
            .expect_err("invalid HoldPkg patterns must not be ignored");
        assert!(error.to_string().contains("Invalid HoldPkg pattern"));
    }

    #[test]
    fn system_upgrade_rejects_targets_instead_of_discarding_them() {
        validate_transaction_targets(TransactionKind::SystemUpgrade, &[])
            .expect("targetless system upgrade");
        let error = validate_transaction_targets(
            TransactionKind::SystemUpgrade,
            &["explicit-target".to_string()],
        )
        .expect_err("explicit target must not be silently discarded");
        assert!(error.to_string().contains("explicit package targets"));
        validate_transaction_targets(TransactionKind::Install, &["package".to_string()])
            .expect("install accepts targets");
    }

    #[test]
    fn recursive_removal_matches_pacman_rs_flags() {
        let explicit = transaction_flags(TransactionKind::Remove { recursive: false });
        assert!(!explicit.contains(alpm::TransFlag::RECURSE));
        assert!(!explicit.contains(alpm::TransFlag::UNNEEDED));

        let recursive = transaction_flags(TransactionKind::Remove { recursive: true });
        assert!(recursive.contains(alpm::TransFlag::RECURSE));
        assert!(!recursive.contains(alpm::TransFlag::UNNEEDED));
    }

    #[test]
    fn download_templates_use_live_indicatif_placeholders() {
        for template in [DOWNLOAD_SPINNER_TEMPLATE, DOWNLOAD_BAR_TEMPLATE] {
            assert!(
                !template.contains("{{"),
                "escaped placeholders render literally"
            );
            indicatif::ProgressStyle::with_template(template).expect("valid progress template");
        }
    }

    #[test]
    fn base_name_parses_simple_and_dash_containing_pkgbases() {
        assert_eq!(
            package_base_name("linux-6.7.0-1-x86_64.pkg.tar.zst"),
            Some("linux")
        );
        // Regression: pkgbases containing dashes must stay one group.
        assert_eq!(
            package_base_name("python-pip-24.0-1-any.pkg.tar.zst"),
            Some("python-pip")
        );
        assert_eq!(
            package_base_name("haskell-aeson-2.1.2.1-269-x86_64.pkg.tar.zst"),
            Some("haskell-aeson")
        );
        assert_eq!(
            package_base_name("foo-1.2-2-x86_64.pkg.tar.xz"),
            Some("foo")
        );
        assert_eq!(
            package_base_name("lib32-mesa-1:24.0.1-1-x86_64.pkg.tar.zst"),
            Some("lib32-mesa")
        );
    }

    #[test]
    fn base_name_rejects_unexpected_shapes() {
        assert_eq!(package_base_name("not-a-package-file.tar.zst"), None);
        assert_eq!(package_base_name("linux-6.7.0-1-x86_64.pkg.tar.gz"), None);
        assert_eq!(package_base_name("-1-2-x86_64.pkg.tar.zst"), None);
    }

    #[test]
    fn keyring_error_detection_matches_expected_keywords() {
        assert!(is_keyring_related_error(
            "invalid or corrupted package (PGP signature)"
        ));
        assert!(is_keyring_related_error("keyring is not writable"));
        assert!(!is_keyring_related_error("conflicting dependencies"));
    }

    #[test]
    fn keyring_prepare_errors_include_repair_commands() {
        let msg = format_trans_prepare_error("invalid or corrupted package");
        assert!(msg.contains("archlinux-keyring"));
        assert!(msg.contains("pacman-key --init"));
        assert!(msg.contains("omg sync && omg install <package>"));
    }

    #[test]
    fn generic_prepare_errors_keep_dependency_guidance() {
        let msg = format_trans_prepare_error("unresolvable package conflicts detected");
        assert!(msg.contains("conflicting packages or missing dependencies"));
        assert!(msg.contains("omg update && omg install <package>"));
    }
}
