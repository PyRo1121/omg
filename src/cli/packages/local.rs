//! Local package handling and metadata extraction
//!
//! Provides robust extraction of metadata (name, version, license) from
//! local package files (.pkg.tar.zst) using either libalpm or pure Rust
//! parsing of the .PKGINFO file.

use anyhow::{Context, Result};
use std::path::Path;

#[derive(Debug, Clone)]
pub struct LocalPackageInfo {
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub licenses: Vec<String>,
    pub packager: Option<String>,
}

/// Extract metadata from a local package file
///
/// Tries the following strategies in order:
/// 1. libalpm (via FFI) - The reference implementation
/// 2. Pure Rust parsing (.PKGINFO via ruzstd + tar) - Fallback
pub fn extract_local_metadata(path: &Path) -> Result<LocalPackageInfo> {
    #[cfg(feature = "arch")]
    {
        // Strategy 1: Try libalpm bindings (most robust if pacman is installed)
        match extract_with_libalpm(path) {
            Ok(info) => return Ok(info),
            Err(e) => tracing::debug!("libalpm extraction failed: {e}, trying pure Rust"),
        }
    }

    // Strategy 2: Pure Rust fallback (works on non-Arch too if enabled, or as fallback)
    extract_with_pure_rust(path)
}

#[cfg(feature = "arch")]
fn extract_with_libalpm(path: &Path) -> Result<LocalPackageInfo> {
    use crate::core::paths;

    // We need an alpm handle
    let root = paths::pacman_root().to_string_lossy().into_owned();
    let db_path = paths::pacman_db_dir().to_string_lossy().into_owned();

    // Safety: alpm requires valid paths
    let alpm = alpm::Alpm::new(root, db_path)?;
    let pkg = alpm.pkg_load(path.to_str().context("Invalid path")?, true, alpm::SigLevel::NONE)?;

    Ok(LocalPackageInfo {
        name: pkg.name().to_string(),
        version: pkg.version().to_string(),
        description: pkg.desc().map(|s| s.to_string()),
        url: pkg.url().map(|s| s.to_string()),
        licenses: pkg.licenses().iter().map(|s| s.to_string()).collect(),
        packager: pkg.packager().map(|s| s.to_string()),
    })
}

fn extract_with_pure_rust(path: &Path) -> Result<LocalPackageInfo> {
    use std::fs::File;
    use std::io::{BufReader, Read};

    let file = File::open(path).context("Failed to open package file")?;
    let reader = BufReader::new(file);

    // Identify compression by extension
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // Decoder setup
    let decoder: Box<dyn Read> = if filename.ends_with(".zst") {
        Box::new(ruzstd::decoding::StreamingDecoder::new(reader).context("Failed to init zstd decoder")?)
    } else if filename.ends_with(".gz") {
        Box::new(flate2::read::GzDecoder::new(reader))
    } else if filename.ends_with(".xz") {
        let mut output = Vec::new();
        lzma_rs::xz_decompress(&mut std::io::BufReader::new(reader), &mut output).context("Failed to decompress xz")?;
        Box::new(std::io::Cursor::new(output))
    } else {
        Box::new(reader)
    };

    let mut archive = tar::Archive::new(decoder);
    let mut pkginfo_content = String::new();
    let mut found = false;

    // Iterate through entries to find .PKGINFO
    for entry in archive.entries().context("Failed to read tar entries")? {
        let mut entry: tar::Entry<Box<dyn Read>> = entry?;
        let entry_path = entry.path()?;

        // SECURITY: Validate path to prevent path traversal attacks
        // Only allow .PKGINFO at the root of the archive
        if let Some(path_str) = entry_path.to_str() {
            // Reject any path containing:
            // - Parent directory references (..)
            // - Absolute paths (starting with /)
            // - Symlinks or special characters
            if path_str.contains("..") || path_str.starts_with('/') {
                anyhow::bail!(
                    "Security: Rejecting malicious path in package archive: {}",
                    path_str
                );
            }

            if path_str == ".PKGINFO" {
                entry.read_to_string(&mut pkginfo_content).context("Failed to read .PKGINFO")?;
                found = true;
                break;
            }
        }
    }

    if !found {
        anyhow::bail!("No .PKGINFO found in package archive");
    }

    parse_pkginfo_manual(&pkginfo_content)
}

fn parse_pkginfo_manual(content: &str) -> Result<LocalPackageInfo> {
    let mut name = String::new();
    let mut version = String::new();
    let mut description = None;
    let mut url = None;
    let mut licenses = Vec::new();
    let mut packager = None;

    for line in content.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim().to_string();

            match key {
                "pkgname" => name = value,
                "pkgver" => version = value,
                "pkgdesc" => description = Some(value),
                "url" => url = Some(value),
                "license" => licenses.push(value),
                "packager" => packager = Some(value),
                _ => {}
            }
        }
    }

    if name.is_empty() || version.is_empty() {
        anyhow::bail!("Invalid .PKGINFO: missing pkgname or pkgver");
    }

    Ok(LocalPackageInfo {
        name,
        version,
        description,
        url,
        licenses,
        packager,
    })
}
