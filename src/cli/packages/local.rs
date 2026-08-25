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
    let alpm = crate::package_managers::alpm_ops::open_default_alpm()?;
    let pkg = alpm.pkg_load(
        path.to_str().context("Invalid path")?,
        true,
        alpm::SigLevel::NONE,
    )?;

    Ok(LocalPackageInfo {
        name: pkg.name().to_string(),
        version: pkg.version().to_string(),
        description: pkg.desc().map(ToString::to_string),
        url: pkg.url().map(ToString::to_string),
        licenses: pkg.licenses().iter().map(ToString::to_string).collect(),
        packager: pkg.packager().map(ToString::to_string),
    })
}

fn extract_with_pure_rust(path: &Path) -> Result<LocalPackageInfo> {
    use std::fs::File;
    use std::io::{BufReader, Read};

    let file = File::open(path).context("Failed to open package file")?;
    let reader = BufReader::new(file);

    // Decoder setup
    let extension = path.extension();
    let decoder: Box<dyn Read> = if extension.is_some_and(|e| e.eq_ignore_ascii_case("zst")) {
        Box::new(
            ruzstd::decoding::StreamingDecoder::new(reader)
                .context("Failed to init zstd decoder")?,
        )
    } else if extension.is_some_and(|e| e.eq_ignore_ascii_case("gz")) {
        Box::new(flate2::read::GzDecoder::new(reader))
    } else if extension.is_some_and(|e| e.eq_ignore_ascii_case("xz")) {
        // xz has no streaming decoder here, so decompress through the shared
        // in-memory budget (crate::runtimes::common::BudgetedSink): a crafted
        // .tar.xz aborts at the cap instead of exhausting memory while we
        // scan for .PKGINFO.
        let mut sink = crate::runtimes::common::BudgetedSink::with_default_budget();
        lzma_rs::xz_decompress(&mut std::io::BufReader::new(reader), &mut sink)
            .context("Failed to decompress xz")?;
        Box::new(std::io::Cursor::new(sink.into_inner()))
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

        // SECURITY: Only a regular, root-level ".PKGINFO" entry is honored.
        // Entries containing parent-directory references (..) or absolute
        // paths are rejected outright; symlink and hardlink entries are
        // skipped so a malicious archive cannot redirect the read.
        let Some(path_str) = entry_path.to_str() else {
            continue;
        };

        if path_str.contains("..") || path_str.starts_with('/') {
            anyhow::bail!("Security: Rejecting malicious path in package archive: {path_str}");
        }

        if path_str != ".PKGINFO" || !entry.header().entry_type().is_file() {
            continue;
        }

        entry
            .read_to_string(&mut pkginfo_content)
            .context("Failed to read .PKGINFO")?;
        found = true;
        break;
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
            let value = value.trim();

            match key {
                "pkgname" => name = value.to_string(),
                "pkgver" => version = value.to_string(),
                "pkgdesc" => description = Some(value.to_string()),
                "url" => url = Some(value.to_string()),
                "license" => licenses.push(value.to_string()),
                "packager" => packager = Some(value.to_string()),
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
