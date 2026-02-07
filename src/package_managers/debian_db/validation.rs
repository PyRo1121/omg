//! Validation and health checks for Debian package operations
//!
//! Provides pre-flight checks before package operations to catch issues early:
//! - Disk space verification
//! - Mirror availability checks
//! - Package integrity validation
//! - Network connectivity tests

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::path::Path;

use anyhow::{Context, Result};

/// Check if sufficient disk space is available for a transaction
///
/// Checks both the temporary download directory and the final installation paths.
/// Returns an error with helpful suggestions if insufficient space is detected.
pub fn check_disk_space(download_size: u64, installed_size: u64, temp_dir: &Path) -> Result<()> {
    use std::fs;

    // Check temp directory space (for downloads)
    if let Ok(_metadata) = fs::metadata(temp_dir) {
        if let Some(parent) = temp_dir.parent() {
            if let Ok(stat) = nix::sys::statvfs::statvfs(parent) {
                let available = stat.blocks_available() * stat.block_size();
                // Add 10% safety margin for metadata and overhead
                let required = download_size + (download_size / 10);

                if available < required {
                    anyhow::bail!(
                        "Insufficient disk space in {}: {} MB available, {} MB required\n\
                        💡 Free up space with:\n\
                        - omg clean (remove cached packages)\n\
                        - sudo apt-get autoclean (Debian/Ubuntu)\n\
                        - Check: df -h {}",
                        parent.display(),
                        available / 1_048_576,
                        required / 1_048_576,
                        parent.display()
                    );
                }
            }
        }
    }

    // Check root filesystem space (for installation)
    if let Ok(stat) = nix::sys::statvfs::statvfs("/") {
        let available = stat.blocks_available() * stat.block_size();
        // Add 20% safety margin for unpacking overhead
        let required = installed_size + (installed_size / 5);

        if available < required {
            anyhow::bail!(
                "Insufficient disk space on /: {} MB available, {} MB required\n\
                💡 Free up space with:\n\
                - sudo apt-get autoremove (remove unused packages)\n\
                - sudo journalctl --vacuum-time=3d (clean logs)\n\
                - Check: du -sh /var/cache/apt/archives",
                available / 1_048_576,
                required / 1_048_576
            );
        }
    }

    Ok(())
}

/// Validate package archive integrity
///
/// Checks that a .deb file is valid and can be extracted.
/// Returns detailed error messages for different failure modes.
pub fn validate_deb_archive(deb_path: &Path, package_name: &str) -> Result<()> {
    use std::fs::File;

    // Check file exists and is readable
    let file = File::open(deb_path).with_context(|| {
        format!(
            "Cannot open package file for {}: {}\n\
            💡 This might indicate:\n\
            - Download was interrupted\n\
            - Corrupted cache\n\
            - Insufficient permissions\n\
            Try: omg clean && omg sync",
            package_name,
            deb_path.display()
        )
    })?;

    // Check file size (minimum valid .deb is ~200 bytes for archive headers)
    let metadata = file.metadata()?;
    if metadata.len() < 200 {
        anyhow::bail!(
            "Package file for {} is too small ({} bytes): {}\n\
            💡 File appears corrupted. Try:\n\
            - omg clean (clear cache)\n\
            - Check network connection\n\
            - Verify mirror availability",
            package_name,
            metadata.len(),
            deb_path.display()
        );
    }

    // Verify ar archive magic (Debian packages are ar archives)
    let mut archive = ar::Archive::new(file);
    let has_valid_entry = matches!(archive.next_entry(), Some(Ok(_)));

    if !has_valid_entry {
        anyhow::bail!(
            "Package file for {} is not a valid .deb archive: {}\n\
            💡 Possible causes:\n\
            - Corrupted download\n\
            - Wrong file format\n\
            - Truncated file\n\
            Try: rm {} && omg install {}",
            package_name,
            deb_path.display(),
            deb_path.display(),
            package_name
        );
    }

    tracing::debug!("Validated .deb archive for {}: {}", package_name, deb_path.display());
    Ok(())
}

/// Check if a mirror/repository URL is accessible
///
/// Performs a lightweight HTTP HEAD request to verify the mirror is responding.
/// Uses a short timeout to fail fast on unreachable mirrors.
pub async fn check_mirror_availability(mirror_url: &str) -> Result<()> {
    use std::time::Duration;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()?;

    let response = client
        .head(mirror_url)
        .send()
        .await
        .with_context(|| {
            format!(
                "Mirror unreachable: {}\n\
                💡 Troubleshooting:\n\
                - Check network: ping $(echo {} | cut -d'/' -f3)\n\
                - Try different mirror in /etc/apt/sources.list\n\
                - Check firewall/proxy settings",
                mirror_url,
                mirror_url
            )
        })?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Mirror returned error: {} {}\n\
            💡 The repository mirror may be temporarily down.\n\
            Try: Edit /etc/apt/sources.list to use a different mirror",
            response.status().as_u16(),
            mirror_url
        );
    }

    tracing::debug!("Mirror available: {}", mirror_url);
    Ok(())
}

/// Verify SHA256 hash of a downloaded file
///
/// Returns an error with suggestions if the hash doesn't match.
pub fn verify_package_hash(
    file_path: &Path,
    expected_hash: &str,
    package_name: &str,
) -> Result<()> {
    use sha2::{Digest, Sha256};
    use std::fs::File;
    use std::io::Read as _;

    let mut file = File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 8192];

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    let computed_hash = format!("{:x}", hasher.finalize());

    if computed_hash != expected_hash {
        anyhow::bail!(
            "Package verification failed for {}: hash mismatch\n\
            Expected: {}\n\
            Computed: {}\n\
            💡 Security warning:\n\
            - Package may be corrupted or tampered with\n\
            - Repository mirror may be compromised\n\
            - Network may have injected malicious content\n\
            Action: Do NOT install. Try:\n\
            - rm {}\n\
            - omg sync (refresh from trusted mirror)\n\
            - Report to repository maintainers if issue persists",
            package_name,
            expected_hash,
            computed_hash,
            file_path.display()
        );
    }

    tracing::debug!(
        "Verified SHA256 for {}: {} (matches)",
        package_name,
        &computed_hash[..16]
    );
    Ok(())
}

/// Estimate time remaining for download based on current progress
///
/// Returns a human-readable estimate (e.g., "2m 30s", "45s", "< 5s")
#[inline]
pub fn estimate_time_remaining(downloaded: u64, total: u64, elapsed_secs: f64) -> String {
    if downloaded == 0 || elapsed_secs < 0.1 {
        return "calculating...".to_string();
    }

    let bytes_per_sec = downloaded as f64 / elapsed_secs;
    let remaining_bytes = total.saturating_sub(downloaded);
    let remaining_secs = remaining_bytes as f64 / bytes_per_sec;

    if remaining_secs < 5.0 {
        "< 5s".to_string()
    } else if remaining_secs < 60.0 {
        format!("{}s", remaining_secs as u64)
    } else if remaining_secs < 3600.0 {
        let mins = (remaining_secs / 60.0) as u64;
        let secs = (remaining_secs % 60.0) as u64;
        format!("{}m {}s", mins, secs)
    } else {
        let hours = (remaining_secs / 3600.0) as u64;
        let mins = ((remaining_secs % 3600.0) / 60.0) as u64;
        format!("{}h {}m", hours, mins)
    }
}

/// Format bytes as human-readable size (e.g., "1.5 MB", "342 KB")
#[inline]
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Format download speed (e.g., "2.5 MB/s", "850 KB/s")
#[inline]
pub fn format_speed(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;

    if bytes_per_sec >= MB {
        format!("{:.2} MB/s", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.1} KB/s", bytes_per_sec / KB)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1536), "1.50 KB");
        assert_eq!(format_bytes(1_048_576), "1.00 MB");
        assert_eq!(format_bytes(1_073_741_824), "1.00 GB");
    }

    #[test]
    fn test_format_speed() {
        assert_eq!(format_speed(512.0), "512 B/s");
        assert_eq!(format_speed(1024.0 * 100.0), "100.0 KB/s");
        assert_eq!(format_speed(1024.0 * 1024.0 * 2.5), "2.50 MB/s");
    }

    #[test]
    fn test_estimate_time_remaining() {
        assert_eq!(estimate_time_remaining(0, 1000, 1.0), "calculating...");
        // 500 bytes in 1 second = 500 bytes/sec
        // Remaining: 500 bytes = 1 second (but < 5s threshold)
        assert_eq!(estimate_time_remaining(500, 1000, 1.0), "< 5s");

        // 100 bytes in 1 second = 100 bytes/sec
        // Remaining: 900 bytes = 9 seconds
        assert_eq!(estimate_time_remaining(100, 1000, 1.0), "9s");

        // 900 bytes in 1 second = 900 bytes/sec
        // Remaining: 100 bytes = 0.11 seconds (< 5s)
        assert_eq!(estimate_time_remaining(900, 1000, 1.0), "< 5s");

        // Test minute formatting: 10 bytes in 1 second = 10 bytes/sec
        // Remaining: 990 bytes = 99 seconds = 1m 39s
        assert_eq!(estimate_time_remaining(10, 1000, 1.0), "1m 39s");
    }
}
