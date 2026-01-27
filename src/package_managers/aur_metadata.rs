//! AUR Metadata Synchronization
//!
//! Handles downloading, caching, and indexing of the AUR metadata archive
//! (packages-meta-ext-v1.json.gz).

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Result;
use flate2::read::GzDecoder;
use reqwest::header::{ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED};
use serde::{Deserialize, Serialize};
use tokio::fs as tokio_fs;
use tracing::{info, instrument};

use crate::config::Settings;
use crate::core::paths;
use crate::package_managers::aur_index::build_index;

const AUR_META_URL: &str = "https://aur.archlinux.org/packages-meta-ext-v1.json.gz";

#[derive(Debug, Deserialize, Serialize)]
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

#[derive(Debug)]
pub struct AurMetadataResponse {
    pub results: Vec<AurJsonPackage>,
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

    let build_dir = paths::cache_dir().join("aur");
    let cache_path = build_dir.join("_meta").join("packages-meta-ext-v1.json.gz");
    let meta_path = build_dir.join("_meta").join("packages-meta-ext-v1.json.gz.meta");
    let index_path = build_dir.join("_meta").join("packages-meta-ext-v1.rkyv");

    // Check TTL if not forced
    if !force && cache_path.exists() {
        let ttl = settings.aur.metadata_cache_ttl_secs;
        let cache_path_clone = cache_path.clone();
        let is_fresh = tokio::task::spawn_blocking(move || {
            std::fs::metadata(&cache_path_clone)
                .and_then(|m| m.modified())
                .map(|m| m.elapsed().unwrap_or_default() < Duration::from_secs(ttl))
                .unwrap_or(false)
        })
        .await?;

        if is_fresh {
            // Ensure index exists even if cache is fresh
            if !index_path.exists() {
                info!("AUR cache is fresh but index is missing. Rebuilding index...");
                let cache_path_clone = cache_path.clone();
                let index_path_clone = index_path.clone();
                tokio::task::spawn_blocking(move || build_index(&cache_path_clone, &index_path_clone))
                    .await??;
            }
            return Ok(());
        }
    }

    // Load ETags/Last-Modified
    let meta_cache = if meta_path.exists() {
        if let Ok(bytes) = tokio_fs::read(&meta_path).await {
            serde_json::from_slice::<AurMetaCache>(&bytes).unwrap_or(AurMetaCache {
                etag: None,
                last_modified: None,
            })
        } else {
            AurMetaCache { etag: None, last_modified: None }
        }
    } else {
        AurMetaCache { etag: None, last_modified: None }
    };

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
            if let Ok(file) = File::options().write(true).open(&cache_path) {
                let _ = file.set_modified(SystemTime::now());
            }
        }
        
        // Ensure index exists
        if !index_path.exists() && cache_path.exists() {
             info!("Rebuilding missing AUR index...");
             let cache_path_clone = cache_path.clone();
             let index_path_clone = index_path.clone();
             tokio::task::spawn_blocking(move || build_index(&cache_path_clone, &index_path_clone))
                 .await??;
        }
        
        return Ok(());
    }

    let response = response.error_for_status()?;
    
    // Capture headers before consuming body
    let etag = response.headers().get(ETAG).and_then(|v| v.to_str().ok()).map(String::from);
    let last_modified = response.headers().get(LAST_MODIFIED).and_then(|v| v.to_str().ok()).map(String::from);

    // Download to temp file
    let tmp_path = cache_path.with_extension("tmp");
    let bytes = response.bytes().await?;
    tokio_fs::write(&tmp_path, &bytes).await?;
    tokio_fs::rename(&tmp_path, &cache_path).await?;

    // Save meta cache
    let new_meta = AurMetaCache { etag, last_modified };
    if let Ok(meta_bytes) = serde_json::to_vec(&new_meta) {
        let _ = tokio_fs::write(&meta_path, meta_bytes).await;
    }

    // Rebuild index
    info!("Building AUR binary index...");
    let cache_path_clone = cache_path.clone();
    let index_path_clone = index_path.clone();
    
    tokio::task::spawn_blocking(move || build_index(&cache_path_clone, &index_path_clone))
        .await??;

    info!("AUR metadata synced and indexed");
    Ok(())
}

/// Read and parse the metadata archive (if you need the raw JSON)
/// Note: prefer using AurIndex for lookups
pub fn read_metadata_archive(path: &Path) -> Result<Vec<AurJsonPackage>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let decoder = GzDecoder::new(reader);
    let results: Vec<AurJsonPackage> = serde_json::from_reader(decoder)?;
    Ok(results)
}

pub fn get_metadata_path() -> PathBuf {
    paths::cache_dir().join("aur").join("_meta").join("packages-meta-ext-v1.json.gz")
}

pub fn get_index_path() -> PathBuf {
    paths::cache_dir().join("aur").join("_meta").join("packages-meta-ext-v1.rkyv")
}
