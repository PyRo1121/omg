//! Validation and health checks for Debian package operations
//!
//! Provides pre-flight checks before package operations to catch issues early:
//! - Disk space verification
//! - Package integrity validation (SHA256 verification)

#![cfg(any(feature = "debian", feature = "debian-pure"))]

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
    let required = download_size + (download_size / 10);

    if available < required {
        anyhow::bail!(
            "Insufficient disk space in {}: {} MB available, {} MB required\n\
                💡 Free up space with:\n\
                - omg clean (remove cached packages)\n\
                - sudo apt-get autoclean (Debian/Ubuntu)\n\
                - Check: df -h {}",
            download_path.display(),
            available / 1_048_576,
            required / 1_048_576,
            download_path.display()
        );
    }

    let available = available_bytes(Path::new("/"))?;
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

    Ok(())
}

fn available_bytes(path: &Path) -> Result<u64> {
    let stat = nix::sys::statvfs::statvfs(path)
        .with_context(|| format!("Failed to check disk space at {}", path.display()))?;
    Ok(stat.blocks_available() * stat.block_size())
}

/// Verify SHA256 hash of a downloaded file
///
/// Returns an error with suggestions if the hash doesn't match.
fn verify_package_hash(file_path: &Path, expected_hash: &str, package_name: &str) -> Result<()> {
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

    let computed_hash = hex::encode(hasher.finalize());

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
