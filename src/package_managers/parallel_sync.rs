//! Parallel package-database synchronization with bounded mirror selection.

use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use reqwest::Client;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::config::Settings;
use crate::core::{http::download_client, paths};
use crate::package_managers::aur_metadata::sync_aur_metadata;

fn get_configured_repos() -> Result<Vec<String>> {
    crate::core::pacman_conf::get_configured_repos()
        .context("Failed to load repositories from pacman.conf")
}

/// Extract the URL from a `Server = <url>` mirrorlist line.
fn parse_server_line(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("Server")?;
    let rest = rest.trim_start();
    rest.strip_prefix('=').map(str::trim)
}

/// Parse all mirrors from mirrorlist
fn get_mirrors() -> Result<Vec<String>> {
    let mirrorlist_path = paths::pacman_mirrorlist_path();
    let mirrorlist = std::fs::read_to_string(&mirrorlist_path)
        .with_context(|| format!("Failed to read {}", mirrorlist_path.display()))?;

    let mut mirrors = Vec::with_capacity(16);
    for line in mirrorlist.lines().map(str::trim) {
        // Skip comments and empty lines
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if let Some(url) = parse_server_line(line) {
            mirrors.push(url.to_string());
        }
    }

    if mirrors.is_empty() {
        anyhow::bail!("No mirrors found in {}", mirrorlist_path.display());
    }

    Ok(mirrors)
}

fn begin_same_dir_temp(dest: &Path) -> Result<(std::fs::File, tempfile::TempPath)> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create download directory: {}", parent.display()))?;
    tempfile::Builder::new()
        .prefix(".download-")
        .tempfile_in(parent)
        .with_context(|| {
            format!(
                "Failed to create temporary download in {}",
                parent.display()
            )
        })
        .map(tempfile::NamedTempFile::into_parts)
}

async fn persist_same_dir_temp(
    mut file: File,
    temporary_path: tempfile::TempPath,
    dest: &Path,
) -> Result<()> {
    file.flush().await.context("Failed to flush download")?;
    file.sync_all().await.context("Failed to sync download")?;
    drop(file);
    temporary_path
        .persist(dest)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to persist download at {}", dest.display()))?;
    crate::core::safe_ops::sync_parent_directory(dest.to_path_buf()).await?;
    Ok(())
}

/// Build the URL for a database file
fn build_db_url(mirror_template: &str, repo: &str) -> String {
    mirror_template
        .replace("$repo", repo)
        .replace("$arch", std::env::consts::ARCH)
        + "/"
        + repo
        + ".db"
}

const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;
const MAX_SYNC_DB_BYTES: u64 = 512 * 1024 * 1024;
const MIRROR_RACE_TIMEOUT_MS: u64 = 2000;
const MAX_MIRRORS_PER_REPO: usize = 5;
/// Repositories synced from the system mirrorlist instead of their own
/// `Server` entries. Keep in sync with `pacman_db::collect_sync_db_paths`.
const MIRRORLIST_REPOS: [&str; 6] = [
    "core",
    "extra",
    "multilib",
    "core-testing",
    "extra-testing",
    "multilib-testing",
];

async fn race_mirrors(client: &Client, urls: &[String]) -> Option<usize> {
    use futures::future::select_all;

    if urls.is_empty() {
        return None;
    }
    if urls.len() == 1 {
        return Some(0);
    }

    let futures: Vec<_> = urls
        .iter()
        .enumerate()
        .map(|(idx, url)| {
            let client = client.clone();
            let url = url.clone();
            Box::pin(async move {
                let result = tokio::time::timeout(
                    Duration::from_millis(MIRROR_RACE_TIMEOUT_MS),
                    client.head(&url).send(),
                )
                .await;
                match result {
                    Ok(Ok(resp))
                        if resp.status().is_success()
                            || resp.status() == reqwest::StatusCode::NOT_MODIFIED =>
                    {
                        Some(idx)
                    }
                    _ => None,
                }
            })
        })
        .collect();

    let mut remaining = futures;
    while !remaining.is_empty() {
        let (result, _idx, rest) = select_all(remaining).await;
        if let Some(winner_idx) = result {
            return Some(winner_idx);
        }
        remaining = rest;
    }

    Some(0)
}

async fn download_response_to_dest(mut response: reqwest::Response, dest: &Path) -> Result<()> {
    if let Some(length) = response.content_length() {
        anyhow::ensure!(
            length <= MAX_SYNC_DB_BYTES,
            "Package database declares {length} bytes, exceeding the {MAX_SYNC_DB_BYTES}-byte limit"
        );
    }

    let (std_file, temporary_path) = begin_same_dir_temp(dest)?;
    let mut file = File::from_std(std_file);
    let mut downloaded = 0_u64;
    while let Some(chunk) = response.chunk().await? {
        downloaded = downloaded
            .checked_add(u64::try_from(chunk.len()).context("Download chunk is too large")?)
            .context("Package database byte count overflowed")?;
        anyhow::ensure!(
            downloaded <= MAX_SYNC_DB_BYTES,
            "Package database exceeded the {MAX_SYNC_DB_BYTES}-byte limit"
        );
        file.write_all(&chunk)
            .await
            .context("Write error during download")?;
    }
    persist_same_dir_temp(file, temporary_path, dest).await
}

#[expect(clippy::literal_string_with_formatting_args, clippy::expect_used)] // Static indicatif templates are always valid; braces are template syntax
async fn download_db(
    client: &Client,
    urls: Vec<String>,
    dest: &PathBuf,
    pb: &ProgressBar,
) -> Result<()> {
    let repo_name = dest.file_stem().map_or_else(
        || "unknown".to_string(),
        |s| s.to_string_lossy().to_string(),
    );
    pb.set_message(format!("{repo_name} (racing mirrors...)"));

    let urls = if urls.len() > 1 {
        if let Some(fastest_idx) = race_mirrors(client, &urls).await {
            let mut reordered = Vec::with_capacity(urls.len());
            reordered.push(urls[fastest_idx].clone());
            for (i, url) in urls.iter().enumerate() {
                if i != fastest_idx {
                    reordered.push(url.clone());
                }
            }
            reordered
        } else {
            urls
        }
    } else {
        urls
    };

    pb.set_message(repo_name.clone());

    let existing_mtime = if dest.exists() {
        tokio::fs::metadata(dest)
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| httpdate::fmt_http_date(std::time::UNIX_EPOCH + d))
            })
    } else {
        None
    };

    let mut last_error = None;

    for (mirror_idx, url) in urls.iter().enumerate() {
        let safe_url = crate::core::http::redact_url(url);
        if mirror_idx > 0 {
            pb.set_message(format!("{} (mirror {})", repo_name, mirror_idx + 1));
        }

        for retry in 0..MAX_RETRIES {
            if retry > 0 {
                let backoff = crate::core::http::retry_backoff(
                    Duration::from_millis(INITIAL_BACKOFF_MS),
                    retry - 1,
                );
                pb.set_message(format!("{repo_name} (retry {retry})"));
                tokio::time::sleep(backoff).await;
            }

            let mut req = client.get(url);
            if let Some(ref mtime) = existing_mtime {
                req = req.header(reqwest::header::IF_MODIFIED_SINCE, mtime);
            }

            let response = match req.send().await {
                Ok(resp) => resp,
                Err(e) => {
                    last_error = Some(anyhow::anyhow!("Failed to connect to {safe_url}: {e}"));
                    if crate::core::http::is_retryable_error(&e) {
                        continue;
                    }
                    break;
                }
            };

            if response.status() == reqwest::StatusCode::NOT_MODIFIED {
                pb.finish_with_message(format!("{repo_name} ✓"));
                return Ok(());
            }

            if crate::core::http::is_retryable_status(response.status()) {
                last_error = Some(anyhow::anyhow!("HTTP {}: {safe_url}", response.status()));
                continue;
            }

            if !response.status().is_success() {
                last_error = Some(anyhow::anyhow!("HTTP {}: {safe_url}", response.status()));
                break;
            }

            let total_size = response.content_length().unwrap_or(0);
            if total_size > 0 {
                pb.set_length(total_size);
                pb.set_style(
                    ProgressStyle::default_bar()
                        .template(
                            "  {spinner:.green} {msg:12} [{bar:30.cyan/blue}] {bytes}/{total_bytes}",
                        )
                        .expect("valid template")
                        .progress_chars("█▓▒░"),
                );
            }

            if let Err(e) = download_response_to_dest(response, dest).await {
                last_error = Some(e);
                continue;
            }

            pb.finish_with_message(format!("{repo_name} ✓"));
            return Ok(());
        }
    }

    pb.finish_with_message(format!("{repo_name} failed"));
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("No mirrors available")))
}

struct DatabasePublication {
    staged: PathBuf,
    destination: PathBuf,
    backup: Option<PathBuf>,
    published: bool,
}

fn rollback_database_publication(publications: &mut [DatabasePublication]) -> Vec<String> {
    let mut errors = Vec::new();
    for publication in publications.iter_mut().rev() {
        if publication.published
            && let Err(error) = std::fs::rename(&publication.destination, &publication.staged)
        {
            errors.push(format!(
                "failed to withdraw {}: {error}",
                publication.destination.display()
            ));
        }
        if let Some(backup) = &publication.backup
            && let Err(error) = std::fs::rename(backup, &publication.destination)
        {
            errors.push(format!(
                "failed to restore {}: {error}",
                publication.destination.display()
            ));
        }
    }
    errors
}

fn commit_staged_databases(
    databases: &[(PathBuf, PathBuf)],
    failed_downloads: usize,
) -> Result<()> {
    anyhow::ensure!(
        failed_downloads == 0,
        "Failed to sync {failed_downloads} database(s); live databases were left unchanged"
    );

    let mut publications = databases
        .iter()
        .map(|(staged, destination)| DatabasePublication {
            staged: staged.clone(),
            destination: destination.clone(),
            backup: None,
            published: false,
        })
        .collect::<Vec<_>>();

    for publication in &publications {
        let metadata = std::fs::symlink_metadata(&publication.staged)
            .with_context(|| format!("Missing staged database {}", publication.staged.display()))?;
        anyhow::ensure!(
            metadata.file_type().is_file(),
            "Staged database is not a regular file: {}",
            publication.staged.display()
        );
    }

    for index in 0..publications.len() {
        match std::fs::symlink_metadata(&publications[index].destination) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                let rollback_errors = rollback_database_publication(&mut publications);
                anyhow::bail!(
                    "Failed to inspect live database {}: {error}; rollback errors: {}",
                    publications[index].destination.display(),
                    rollback_errors.join("; ")
                );
            }
        }
        let backup = publications[index]
            .staged
            .with_extension(format!("omg-backup-{index}"));
        if let Err(error) = std::fs::rename(&publications[index].destination, &backup) {
            let rollback_errors = rollback_database_publication(&mut publications);
            anyhow::bail!(
                "Failed to stage live database {} for replacement: {error}; rollback errors: {}",
                publications[index].destination.display(),
                rollback_errors.join("; ")
            );
        }
        publications[index].backup = Some(backup);
    }

    for index in 0..publications.len() {
        if let Err(error) = std::fs::rename(
            &publications[index].staged,
            &publications[index].destination,
        ) {
            let failed_destination = publications[index].destination.display().to_string();
            let rollback_errors = rollback_database_publication(&mut publications);
            anyhow::bail!(
                "Failed to publish staged package database to {failed_destination}: {error}; rollback errors: {}",
                rollback_errors.join("; ")
            );
        }
        publications[index].published = true;
    }

    for publication in &publications {
        if let Some(backup) = &publication.backup {
            std::fs::remove_file(backup).with_context(|| {
                format!("Failed to remove database backup {}", backup.display())
            })?;
        }
    }
    for publication in &publications {
        crate::core::safe_ops::sync_parent_directory_sync(&publication.destination)?;
    }
    Ok(())
}

/// Synchronize configured package databases concurrently.
#[expect(clippy::literal_string_with_formatting_args, clippy::expect_used)] // Static indicatif templates are always valid; braces are template syntax
pub async fn sync_databases_parallel() -> Result<()> {
    let mirrors = get_mirrors()?;

    println!(
        "{} Synchronizing package databases...\n",
        "OMG".cyan().bold()
    );

    // Sync directory (we should already be root at this point)
    let sync_dir = paths::pacman_sync_dir();
    if !sync_dir.exists() {
        tokio::fs::create_dir_all(&sync_dir)
            .await
            .with_context(|| format!("Failed to create {}", sync_dir.display()))?;
    }

    // Download the complete repository set into a same-filesystem staging
    // directory. A failed mirror must not leave a mix of old and new live DBs.
    let staging = tempfile::Builder::new()
        .prefix(".omg-sync-")
        .tempdir_in(&sync_dir)
        .with_context(|| format!("Failed to stage databases in {}", sync_dir.display()))?;

    // Set up progress bars
    let mp = MultiProgress::new();
    let client = download_client().clone();

    // Start AUR metadata sync in background
    let aur_sync_handle = {
        let client = client.clone();
        tokio::spawn(async move {
            let settings = match Settings::load() {
                Ok(settings) => settings,
                Err(error) => {
                    tracing::error!("Failed to load OMG settings for AUR metadata sync: {error}");
                    return;
                }
            };
            if let Err(e) = sync_aur_metadata(&client, &settings, false).await {
                tracing::warn!("Failed to sync AUR metadata: {}", e);
            }
        })
    };

    // Collect all repos to sync from pacman.conf
    let configured_repos = get_configured_repos()?;
    let mut repos_to_sync: Vec<(String, Vec<String>, PathBuf)> =
        Vec::with_capacity(configured_repos.len());

    // Standard repos use the system mirrorlist.
    for repo in &configured_repos {
        if MIRRORLIST_REPOS.contains(&repo.as_str()) {
            let repo_urls: Vec<String> = mirrors
                .iter()
                .map(|m| build_db_url(m, repo))
                .take(MAX_MIRRORS_PER_REPO)
                .collect();
            let dest = sync_dir.join(format!("{repo}.db"));
            repos_to_sync.push((repo.clone(), repo_urls, dest));
        }
    }

    // Custom repos from pacman.conf (have their own Server= lines)
    if let Ok(custom_repos) = get_custom_repos() {
        for (repo_name, mut urls) in custom_repos {
            let dest = sync_dir.join(format!("{repo_name}.db"));
            for url in &mut urls {
                if !url.ends_with('/') {
                    url.push('/');
                }
                url.push_str(&repo_name);
                url.push_str(".db");
            }
            repos_to_sync.push((repo_name, urls, dest));
        }
    }

    // Create progress bars
    let progress_bars: Vec<ProgressBar> = repos_to_sync
        .iter()
        .map(|(name, _, _)| {
            let pb = mp.add(ProgressBar::new_spinner());
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("  {spinner:.green} {msg:12} [connecting...]")
                    .expect("valid template"),
            );
            pb.set_message(name.clone());
            pb.enable_steady_tick(Duration::from_millis(100));
            pb
        })
        .collect();

    // Run all downloads in parallel using JoinSet for structured concurrency
    let repos_count = repos_to_sync.len();
    let mut tasks = tokio::task::JoinSet::new();

    let mut staged_databases = Vec::with_capacity(repos_count);
    for (i, (_, urls, destination)) in repos_to_sync.into_iter().enumerate() {
        let client = client.clone();
        let Some(pb) = progress_bars.get(i).cloned() else {
            continue;
        };
        let staged = staging.path().join(format!("{i}.db"));
        staged_databases.push((staged.clone(), destination));

        tasks.spawn(async move { download_db(&client, urls, &staged, &pb).await });
    }

    // Wait for all downloads
    let mut errors = Vec::with_capacity(repos_count);
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => errors.push(e),
            Err(e) => errors.push(anyhow::anyhow!("Task panicked: {e}")),
        }
    }

    // Wait for AUR sync to complete
    if let Err(e) = aur_sync_handle.await {
        tracing::warn!("AUR sync task panicked: {}", e);
    }

    println!();

    for error in &errors {
        tracing::error!("Sync error: {error}");
    }
    commit_staged_databases(&staged_databases, errors.len())?;

    crate::package_managers::alpm_direct::clear_alpm_cache();
    println!("{} Databases synchronized successfully!\n", "✓".green());
    Ok(())
}

/// Resolve custom (non-mirrorlist) repositories from pacman.conf.
///
/// Uses the shared [`crate::core::pacman_conf::PacmanConfig`] parser and the
/// configured pacman.conf path so custom repos honor path overrides and test
/// mode, and `$repo`/`$arch` placeholders follow the running architecture.
fn get_custom_repos() -> Result<Vec<(String, Vec<String>)>> {
    let conf_path = paths::pacman_conf_path();
    let config = crate::core::pacman_conf::PacmanConfig::parse(&conf_path)
        .with_context(|| format!("Failed to parse {}", conf_path.display()))?;

    let mut repos = Vec::with_capacity(4);
    let arch = std::env::consts::ARCH;
    for repo in &config.repos {
        if MIRRORLIST_REPOS.contains(&repo.name.as_str()) {
            continue;
        }
        match config.resolve_servers(repo, arch) {
            Ok(servers) if !servers.is_empty() => {
                repos.push((repo.name.clone(), servers));
            }
            Ok(_) => {
                tracing::debug!(
                    "Custom repo '{}' has no resolvable servers; skipping",
                    repo.name
                );
            }
            Err(error) => {
                // One broken custom repo must not abort syncing the others.
                tracing::warn!(
                    "Failed to resolve servers for repo '{}': {error}",
                    repo.name
                );
            }
        }
    }

    Ok(repos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_staged_set_does_not_replace_live_databases() {
        let directory = tempfile::tempdir().expect("database directory");
        let live = directory.path().join("core.db");
        let staged = directory.path().join("core.db.staged");
        std::fs::write(&live, b"old").expect("seed live database");
        std::fs::write(&staged, b"new").expect("seed staged database");

        let error = commit_staged_databases(&[(staged, live.clone())], 1)
            .expect_err("failed download set must not publish");

        assert!(error.to_string().contains("1 database"));
        assert_eq!(std::fs::read(live).expect("live database"), b"old");
    }

    #[test]
    fn publication_failure_restores_every_live_database() {
        let directory = tempfile::tempdir().expect("database directory");
        let core_live = directory.path().join("core.db");
        let core_staged = directory.path().join("core.staged");
        let extra_staged = directory.path().join("extra.staged");
        let extra_live = directory.path().join("missing-parent/extra.db");
        std::fs::write(&core_live, b"old-core").expect("seed live core");
        std::fs::write(&core_staged, b"new-core").expect("stage core");
        std::fs::write(&extra_staged, b"new-extra").expect("stage extra");

        let error = commit_staged_databases(
            &[
                (core_staged.clone(), core_live.clone()),
                (extra_staged.clone(), extra_live),
            ],
            0,
        )
        .expect_err("a mid-publication failure must roll back earlier databases");

        assert!(error.to_string().contains("Failed to publish"), "{error}");
        assert_eq!(std::fs::read(core_live).unwrap(), b"old-core");
        assert_eq!(std::fs::read(core_staged).unwrap(), b"new-core");
        assert_eq!(std::fs::read(extra_staged).unwrap(), b"new-extra");
    }

    #[test]
    fn server_line_parser_requires_literal_server_key() {
        assert_eq!(
            parse_server_line("Server = https://m.example.com/$repo/os/$arch"),
            Some("https://m.example.com/$repo/os/$arch")
        );
        assert_eq!(
            parse_server_line("Server=https://m.example.com"),
            Some("https://m.example.com")
        );
        // Regression: a key merely *containing* "Server" must be rejected.
        assert_eq!(parse_server_line("Serverless = https://nope"), None);
        assert_eq!(parse_server_line("# Server = https://commented"), None);
    }

    #[test]
    fn db_urls_substitute_repo_and_runtime_arch() {
        let arch = std::env::consts::ARCH;
        let url = build_db_url("https://mirror.example.com/$repo/os/$arch", "core");
        assert_eq!(
            url,
            format!("https://mirror.example.com/core/os/{arch}/core.db")
        );
    }

    #[test]
    #[serial_test::serial]
    fn custom_repos_honor_configured_path_and_runtime_arch() {
        let dir = tempfile::tempdir().expect("temp dir");
        let conf = dir.path().join("pacman.conf");
        std::fs::write(
            &conf,
            "[options]\n\n[extra]\nInclude = /etc/pacman.d/mirrorlist\n\n\
             [chaotic-aur]\nServer = https://mirror.example.com/$repo/$arch\n",
        )
        .expect("write conf");

        temp_env::with_var("OMG_PACMAN_CONF", Some(conf.as_os_str()), || {
            let repos = get_custom_repos().expect("custom repos should resolve");
            assert_eq!(repos.len(), 1, "mirrorlist-backed 'extra' must be excluded");
            assert_eq!(repos[0].0, "chaotic-aur");
            assert_eq!(
                repos[0].1,
                vec![format!(
                    "https://mirror.example.com/chaotic-aur/{}",
                    std::env::consts::ARCH
                )]
            );
        });
    }
}
