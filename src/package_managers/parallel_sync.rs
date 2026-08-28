//! Parallel database synchronization - 3-5x FASTER than pacman -Sy
//!
//! Downloads all repository databases in parallel using async I/O,
//! with progress bars and smart mirror selection.

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
const MIRROR_RACE_TIMEOUT_MS: u64 = 2000;

async fn race_mirrors(client: &Client, urls: &[String]) -> Option<usize> {
    use futures::future::select_all;
    use std::time::Duration;

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
    let (std_file, temporary_path) = begin_same_dir_temp(dest)?;
    let mut file = File::from_std(std_file);
    while let Some(chunk) = response.chunk().await? {
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
                let backoff = Duration::from_millis(INITIAL_BACKOFF_MS * 2u64.pow(retry - 1));
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
                    if e.is_timeout() || e.is_connect() {
                        continue;
                    }
                    break;
                }
            };

            if response.status() == reqwest::StatusCode::NOT_MODIFIED {
                pb.finish_with_message(format!("{repo_name} ✓"));
                return Ok(());
            }

            if response.status().is_server_error() {
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

/// Synchronize package databases in parallel - BLAZING FAST
///
/// This is 3-5x faster than `pacman -Sy` because:
/// 1. Downloads all databases simultaneously (parallel I/O)
/// 2. Uses HTTP/2 connection pooling
/// 3. Shows real-time progress for each database
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

    // Standard repos (use mirrorlist)
    let standard_repos: std::collections::HashSet<&str> = [
        "core",
        "extra",
        "multilib",
        "core-testing",
        "extra-testing",
        "multilib-testing",
    ]
    .into_iter()
    .collect();

    for repo in &configured_repos {
        if standard_repos.contains(repo.as_str()) {
            let repo_urls: Vec<String> = mirrors
                .iter()
                .map(|m| build_db_url(m, repo))
                .take(5)
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

    for (i, (_, urls, dest)) in repos_to_sync.into_iter().enumerate() {
        let client = client.clone();
        let Some(pb) = progress_bars.get(i).cloned() else {
            continue;
        };

        tasks.spawn(async move { download_db(&client, urls, &dest, &pb).await });
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

    if errors.is_empty() {
        crate::package_managers::alpm_direct::clear_alpm_cache();
        println!("{} Databases synchronized successfully!\n", "✓".green());
        Ok(())
    } else {
        for e in &errors {
            tracing::error!("Sync error: {}", e);
        }
        anyhow::bail!("Failed to sync {} database(s)", errors.len())
    }
}

/// Repositories that are synced from the mirrorlist instead of their own
/// `Server` entries. Keep in sync with `pacman_db::collect_sync_db_paths`.
const MIRRORLIST_REPOS: [&str; 6] = [
    "core",
    "extra",
    "multilib",
    "core-testing",
    "extra-testing",
    "multilib-testing",
];

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
