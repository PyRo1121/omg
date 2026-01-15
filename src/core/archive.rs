//! Pure Rust archive extraction utilities
//!
//! No subprocess spawning - all extraction done in Rust.

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use std::fs::{self, File};
use std::path::Path;
use tar::Archive;

/// Extract a .tar.gz archive to a directory
pub fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    fs::create_dir_all(dest_dir)?;

    archive
        .unpack(dest_dir)
        .with_context(|| format!("Failed to extract to: {}", dest_dir.display()))?;

    Ok(())
}

/// Extract a .tar.gz archive, stripping the first N path components
pub fn extract_tar_gz_strip(archive_path: &Path, dest_dir: &Path, strip: usize) -> Result<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

    let decoder = GzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    fs::create_dir_all(dest_dir)?;

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        // Strip N components from path
        let stripped: std::path::PathBuf = path.components().skip(strip).collect();

        if stripped.as_os_str().is_empty() {
            continue;
        }

        let dest_path = dest_dir.join(&stripped);

        // Create parent directories
        if let Some(parent) = dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Extract based on entry type
        if entry.header().entry_type().is_dir() {
            fs::create_dir_all(&dest_path)?;
        } else {
            entry.unpack(&dest_path)?;
        }
    }

    Ok(())
}

/// Extract a .tar.xz archive to a directory
pub fn extract_tar_xz(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

    let decoder = xz2::read::XzDecoder::new(file);
    let mut archive = Archive::new(decoder);

    fs::create_dir_all(dest_dir)?;

    archive
        .unpack(dest_dir)
        .with_context(|| format!("Failed to extract to: {}", dest_dir.display()))?;

    Ok(())
}

/// Extract a .zip archive to a directory
pub fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

    let mut archive = zip::ZipArchive::new(file)?;

    fs::create_dir_all(dest_dir)?;

    archive
        .extract(dest_dir)
        .with_context(|| format!("Failed to extract to: {}", dest_dir.display()))?;

    Ok(())
}

/// Extract a .zip archive, stripping the first N path components
pub fn extract_zip_strip(archive_path: &Path, dest_dir: &Path, strip: usize) -> Result<()> {
    let file = File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

    let mut archive = zip::ZipArchive::new(file)?;

    fs::create_dir_all(dest_dir)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let outpath = match file.enclosed_name() {
            Some(path) => {
                let stripped: std::path::PathBuf = path.components().skip(strip).collect();
                if stripped.as_os_str().is_empty() {
                    continue;
                }
                dest_dir.join(stripped)
            }
            None => continue,
        };

        if file.is_dir() {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)?;
                }
            }
            let mut outfile = File::create(&outpath)?;
            std::io::copy(&mut file, &mut outfile)?;
        }

        // Set permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&outpath, fs::Permissions::from_mode(mode))?;
            }
        }
    }

    Ok(())
}

/// Detect archive type and extract appropriately
pub fn extract_auto(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    let filename = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if filename.ends_with(".tar.gz")
        || Path::new(filename)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("tgz"))
    {
        extract_tar_gz(archive_path, dest_dir)
    } else if filename.ends_with(".tar.xz")
        || Path::new(filename)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("txz"))
    {
        extract_tar_xz(archive_path, dest_dir)
    } else if Path::new(filename)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        extract_zip(archive_path, dest_dir)
    } else {
        anyhow::bail!("Unknown archive format: {filename}")
    }
}

/// Detect archive type and extract with strip
pub fn extract_auto_strip(archive_path: &Path, dest_dir: &Path, strip: usize) -> Result<()> {
    let filename = archive_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    if filename.ends_with(".tar.gz")
        || Path::new(filename)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("tgz"))
    {
        extract_tar_gz_strip(archive_path, dest_dir, strip)
    } else if Path::new(filename)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
    {
        extract_zip_strip(archive_path, dest_dir, strip)
    } else {
        // Fall back to regular extraction for other types
        extract_auto(archive_path, dest_dir)
    }
}
