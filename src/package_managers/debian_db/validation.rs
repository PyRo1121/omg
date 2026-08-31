//! Validation and health checks for Debian package operations
//!
//! Provides pre-flight checks before package operations to catch issues early:
//! - Disk space verification
//! - Package integrity validation (SHA256 verification)

#![cfg(any(feature = "debian", feature = "debian-pure"))]

use std::os::unix::fs::MetadataExt;
use std::path::Path;

use anyhow::{Context, Result};

/// Check if sufficient disk space is available for a transaction
///
/// Checks both the temporary download directory and the final installation paths.
/// Returns an error with helpful suggestions if insufficient space is detected.
pub fn check_disk_space(download_size: u64, installed_size: u64, temp_dir: &Path) -> Result<()> {
    let download_path = if temp_dir.exists() {
        temp_dir
    } else {
        temp_dir.parent().unwrap_or(temp_dir)
    };
    let available = available_bytes(download_path)?;
    // Saturating headroom math: absurd sizes degrade to "needs everything"
    // instead of wrapping into a false pass.
    let download_required = download_size.saturating_add(download_size / 10);

    if available < download_required {
        anyhow::bail!(
            "Insufficient disk space in {}: {} MB available, {} MB required\n\
                💡 Free up space with:\n\
                - omg clean (remove cached packages)\n\
                - sudo apt-get autoclean (Debian/Ubuntu)\n\
                - Check: df -h {}",
            download_path.display(),
            available / 1_048_576,
            download_required / 1_048_576,
            download_path.display()
        );
    }

    let root_path = Path::new("/");
    let available = available_bytes(root_path)?;
    let install_required = installed_size.saturating_add(installed_size / 5);
    let shares_root_filesystem =
        std::fs::metadata(download_path)?.dev() == std::fs::metadata(root_path)?.dev();
    let required = root_space_required(download_required, install_required, shares_root_filesystem);

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

    Ok(())
}

fn root_space_required(
    download_required: u64,
    install_required: u64,
    shares_root_filesystem: bool,
) -> u64 {
    if shares_root_filesystem {
        download_required.saturating_add(install_required)
    } else {
        install_required
    }
}

fn available_bytes(path: &Path) -> Result<u64> {
    let stat = nix::sys::statvfs::statvfs(path)
        .with_context(|| format!("Failed to check disk space at {}", path.display()))?;
    stat.blocks_available()
        .checked_mul(stat.block_size())
        .ok_or_else(|| anyhow::anyhow!("disk space calculation overflow at {}", path.display()))
}

/// Verify SHA256 hash of a downloaded file
///
/// Returns an error with suggestions if the hash doesn't match.
fn verify_package_hash(file_path: &Path, expected_hash: &str, package_name: &str) -> Result<()> {
    let computed_hash = sha256_file(file_path)?;

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

/// Streaming SHA256 of a file (8 KiB chunks), so large `.deb` archives are
/// never buffered whole in memory.
pub(crate) fn sha256_file(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    use std::fs::File;
    use std::io::Read as _;

    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 8192];

    loop {
        let n = file.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    Ok(hex::encode(hasher.finalize()))
}

/// Rejects a `.deb` unless a SHA256 digest is present and matches the file bytes.
pub fn require_verified_deb(path: &Path, package_name: &str, sha256: Option<&str>) -> Result<()> {
    let expected = sha256.ok_or_else(|| {
        anyhow::anyhow!(
            "Package verification failed for {package_name}: missing SHA256\n\
             Refusing to install an unverified package."
        )
    })?;
    verify_package_hash(path, expected, package_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_sha256_rejects_the_package() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("pkg.deb");
        std::fs::write(&path, b"hello").expect("write deb");
        let err = require_verified_deb(&path, "demo", None).expect_err("missing hash");
        assert!(
            err.to_string().contains("SHA256"),
            "missing hash must be an explicit verification error, got: {err}"
        );
    }

    #[test]
    fn matching_sha256_accepts_the_package() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("pkg.deb");
        std::fs::write(&path, b"hello").expect("write deb");
        require_verified_deb(
            &path,
            "demo",
            Some("2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"),
        )
        .expect("matching hash");
    }

    #[test]
    fn mismatched_sha256_rejects_the_package() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("pkg.deb");
        std::fs::write(&path, b"hello").expect("write deb");
        let err = require_verified_deb(&path, "demo", Some("deadbeef")).expect_err("mismatch");
        assert!(
            err.to_string().contains("hash mismatch"),
            "tampered package must fail verification, got: {err}"
        );
    }

    #[test]
    fn shared_filesystem_budget_includes_download_and_install_footprints() {
        assert_eq!(root_space_required(110, 120, true), 230);
        assert_eq!(root_space_required(110, 120, false), 120);
        assert_eq!(root_space_required(u64::MAX, 1, true), u64::MAX);
    }

    #[test]
    fn check_disk_space_allows_small_transaction() {
        let dir = tempfile::tempdir().expect("temp dir");
        check_disk_space(0, 0, dir.path()).expect("zero-size transaction fits");
    }

    #[test]
    fn check_disk_space_rejects_unreadable_path() {
        let error = check_disk_space(1, 1, Path::new("/no/such/omg-disk-check/nested"))
            .expect_err("failed disk-space probe must not look like enough space");
        assert!(
            error.to_string().contains("Failed to check disk space"),
            "got: {error}"
        );
    }
}
