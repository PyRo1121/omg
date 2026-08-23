//! AUR Metadata Synchronization
//!
//! Handles downloading, caching, and indexing of the AUR metadata archive
//! (packages-meta-ext-v1.json.gz).

use std::fs::File;
use std::io::{BufReader, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use serde::{Deserialize, Serialize};
use tokio::fs as tokio_fs;
use tracing::{info, instrument};

use crate::config::Settings;
use crate::core::paths;
use crate::package_managers::aur_index::build_index;

const AUR_META_URL: &str = "https://aur.archlinux.org/packages-meta-ext-v1.json.gz";

#[derive(Debug, Default, Deserialize, Serialize)]
struct AurMetaCache {
    etag: Option<String>,
    last_modified: Option<String>,
}

/// Raw package entry from the AUR JSON dump
#[derive(Debug, Deserialize)]
pub struct AurJsonPackage {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "Description")]
    pub description: Option<String>,
    #[serde(rename = "Maintainer")]
    pub maintainer: Option<String>,
    #[serde(rename = "NumVotes")]
    pub num_votes: Option<i32>,
    #[serde(rename = "Popularity")]
    pub popularity: Option<f64>,
    #[serde(rename = "OutOfDate")]
    pub out_of_date: Option<i64>,
    #[serde(rename = "LastModified")]
    pub last_modified: Option<i64>,
}

/// Sync AUR metadata: Download if newer, update cache, rebuild index
#[instrument(skip(client, settings))]
pub async fn sync_aur_metadata(
    client: &reqwest::Client,
    settings: &Settings,
    force: bool,
) -> Result<()> {
    if !settings.aur.use_metadata_archive {
        return Ok(());
    }

    let cache_path = metadata_path();
    let meta_path = cache_path.with_extension("gz.meta");
    let index_path = index_path();

    // Check TTL if not forced
    if !force && cache_path.exists() {
        let ttl = settings.aur.metadata_cache_ttl_secs;
        let cache_path_clone = cache_path.clone();
        let is_fresh = tokio::task::spawn_blocking(move || {
            std::fs::metadata(&cache_path_clone)
                .and_then(|m| m.modified())
                .is_ok_and(|m| m.elapsed().unwrap_or_default() < Duration::from_secs(ttl))
        })
        .await?;

        if is_fresh {
            // Ensure index exists even if cache is fresh
            if !index_path.exists() {
                info!("AUR cache is fresh but index is missing. Rebuilding index...");
                rebuild_index(&cache_path, &index_path).await?;
            }
            return Ok(());
        }
    }

    // Load ETags/Last-Modified
    let meta_cache = tokio_fs::read(&meta_path)
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice::<AurMetaCache>(&bytes).ok())
        .unwrap_or_default();

    if let Some(parent) = cache_path.parent() {
        tokio_fs::create_dir_all(parent).await?;
    }

    // Prepare request
    let mut req = client.get(AUR_META_URL);
    if let Some(etag) = &meta_cache.etag {
        req = req.header(IF_NONE_MATCH, etag);
    }
    if let Some(last_modified) = &meta_cache.last_modified {
        req = req.header(IF_MODIFIED_SINCE, last_modified);
    }

    let response = req.send().await?;

    if response.status() == reqwest::StatusCode::NOT_MODIFIED {
        // Cache is still valid on server side
        // Touch the file to update mtime so we don't check again immediately
        if cache_path.exists() {
            let touched = tokio::task::spawn_blocking({
                let cache_path = cache_path.clone();
                move || -> std::io::Result<()> {
                    let file = File::options().write(true).open(&cache_path)?;
                    file.set_modified(SystemTime::now())
                }
            })
            .await;
            match touched {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::debug!("Failed to refresh AUR metadata mtime: {error}");
                }
                Err(error) => {
                    tracing::debug!("AUR metadata touch task failed: {error}");
                }
            }
        }

        // Ensure index exists
        if !index_path.exists() && cache_path.exists() {
            info!("Rebuilding missing AUR index...");
            rebuild_index(&cache_path, &index_path).await?;
        }

        return Ok(());
    }

    let response = response.error_for_status()?;

    // Capture headers before consuming body
    let etag = response
        .headers()
        .get(ETAG)
        .and_then(|v| v.to_str().ok())
        .map(String::from);
    let last_modified = response
        .headers()
        .get(LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let bytes = response.bytes().await?;
    {
        // `reqwest::Bytes` is already owned and 'static; pass it straight
        // through instead of copying the (potentially large) archive.
        let cache_path = cache_path.clone();
        tokio::task::spawn_blocking(move || persist_file_atomically(&cache_path, &bytes)).await??;
    }

    // Persist ETag / Last-Modified so the next sync can make a conditional request.
    let new_meta = AurMetaCache {
        etag,
        last_modified,
    };
    match serde_json::to_vec(&new_meta) {
        Ok(meta_bytes) => {
            let result = tokio::task::spawn_blocking(move || {
                persist_file_atomically(&meta_path, &meta_bytes)
            })
            .await;
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    tracing::warn!("Failed to persist AUR metadata sidecar: {error}");
                }
                Err(error) => {
                    tracing::warn!("AUR metadata sidecar task failed: {error}");
                }
            }
        }
        Err(error) => tracing::warn!("Failed to serialize AUR metadata sidecar: {error}"),
    }

    // Rebuild index
    info!("Building AUR binary index...");
    rebuild_index(&cache_path, &index_path).await?;

    info!("AUR metadata synced and indexed");
    Ok(())
}

/// Read and parse the metadata archive (if you need the raw JSON)
/// Note: prefer using `AurIndex` for lookups
pub fn read_metadata_archive(path: &Path) -> Result<Vec<AurJsonPackage>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let decoder = GzDecoder::new(reader);
    serde_json::from_reader(decoder).map_err(Into::into)
}

pub fn metadata_path() -> PathBuf {
    paths::cache_dir()
        .join("aur")
        .join("_meta")
        .join("packages-meta-ext-v1.json.gz")
}

pub fn index_path() -> PathBuf {
    paths::cache_dir()
        .join("aur")
        .join("_meta")
        .join("packages-meta-ext-v1.rkyv")
}

async fn rebuild_index(archive_path: &Path, index_path: &Path) -> Result<()> {
    let archive_path = archive_path.to_owned();
    let index_path = index_path.to_owned();
    tokio::task::spawn_blocking(move || build_index(&archive_path, &index_path)).await?
}

fn persist_file_atomically(dest: &Path, data: &[u8]) -> Result<()> {
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent).with_context(|| {
        format!(
            "Failed to create AUR metadata directory: {}",
            parent.display()
        )
    })?;
    let mut file = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "Failed to create temporary AUR metadata in {}",
            parent.display()
        )
    })?;
    file.write_all(data)?;
    file.as_file_mut().sync_all()?;
    file.persist(dest)
        .map_err(|error| error.error)
        .with_context(|| format!("Failed to persist AUR metadata at {}", dest.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aur_metadata_persist_round_trips() {
        let temp = tempfile::TempDir::new().unwrap();
        let dest = temp
            .path()
            .join("_meta")
            .join("packages-meta-ext-v1.json.gz");
        persist_file_atomically(&dest, b"archive").unwrap();
        assert_eq!(std::fs::read(dest).unwrap(), b"archive");
    }

    #[test]
    fn aur_metadata_persist_refuses_to_clobber_a_directory() {
        let temp = tempfile::TempDir::new().unwrap();
        let dest = temp.path().join("packages-meta-ext-v1.json.gz");
        std::fs::create_dir(&dest).unwrap();
        let error = persist_file_atomically(&dest, b"archive")
            .expect_err("must refuse to persist over a directory");
        assert!(
            error.to_string().contains("persist"),
            "error must name the persist failure, got: {error}"
        );
        assert!(
            dest.is_dir(),
            "the existing directory must not have been clobbered"
        );
    }
}
