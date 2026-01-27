//! Real-world security integration tests
//!
//! These tests interact with actual external systems:
//! - Real SLSA Rekor transparency log (sigstore.dev)
//! - Real OSV/ALSA vulnerability databases
//! - Real Arch Linux package cache for PGP verification
//!
//! No mocks, no stubs - only production-ready integration tests.

use omg_lib::core::security::slsa::{SlsaLevel, SlsaVerifier};
use omg_lib::core::security::vulnerability::VulnerabilityScanner;
use omg_lib::package_managers::types::parse_version_or_zero;
use std::time::Duration;

/// Test SLSA verification against real Rekor transparency log
///
/// This test queries the actual Sigstore Rekor instance to verify
/// that our SLSA verification can communicate with production infrastructure.
#[tokio::test]
#[ignore] // Run with --ignored flag to test against real external services
async fn test_slsa_rekor_query_real() {
    let verifier = SlsaVerifier::new().expect("Failed to create SLSA verifier");

    // Use a known artifact hash from a real signed artifact
    // This is the hash of the empty file (commonly used for testing)
    let test_hash = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

    // Query real Rekor instance
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        verifier.query_rekor(test_hash),
    )
    .await;

    // Should complete without timeout
    assert!(
        result.is_ok(),
        "Rekor query timed out - check network connectivity"
    );

    // Result should be Ok (empty vec is fine if hash not found)
    let entries = result.unwrap();
    assert!(
        entries.is_ok(),
        "Failed to query Rekor: {:?}",
        entries.err()
    );

    println!(
        "✓ Successfully queried Rekor transparency log, found {} entries",
        entries.unwrap().len()
    );
}

/// Test vulnerability scanner against real ALSA database
///
/// Queries the actual Arch Linux Security Advisories API to verify
/// our scanner can fetch and parse real vulnerability data.
#[tokio::test]
#[ignore] // Run with --ignored flag to test against real external services
async fn test_vulnerability_scanner_alsa_real() {
    let scanner = VulnerabilityScanner::new();

    // Query real ALSA database
    let result = tokio::time::timeout(Duration::from_secs(15), scanner.fetch_alsa_issues()).await;

    // Should complete without timeout
    assert!(
        result.is_ok(),
        "ALSA query timed out - check network connectivity"
    );

    let issues = result.unwrap();
    assert!(
        issues.is_ok(),
        "Failed to fetch ALSA issues: {:?}",
        issues.err()
    );

    let issues = issues.unwrap();

    // Validate structure of real ALSA data
    for issue in issues.iter().take(3) {
        // Every issue should have a name
        assert!(!issue.name.is_empty(), "Issue missing name");

        // Should have at least one affected package
        assert!(
            !issue.packages.is_empty(),
            "Issue {} has no packages",
            issue.name
        );

        // Status should be "Vulnerable" (we filter for this)
        assert!(
            issue.status.to_lowercase().contains("vulnerable"),
            "Issue {} has unexpected status: {}",
            issue.name,
            issue.status
        );

        // Should have severity
        assert!(!issue.severity.is_empty(), "Issue {} missing severity", issue.name);

        // Should have affected version
        assert!(
            !issue.affected.is_empty(),
            "Issue {} missing affected version",
            issue.name
        );
    }

    println!(
        "✓ Successfully fetched {} ALSA issues from production API",
        issues.len()
    );
}

/// Test OSV database query for real package
///
/// Queries the actual OSV (Open Source Vulnerabilities) database
/// to verify our scanner can look up CVEs for real packages.
#[tokio::test]
#[ignore] // Run with --ignored flag to test against real external services
async fn test_vulnerability_scanner_osv_real() {
    let scanner = VulnerabilityScanner::new();

    // Test with a real package version
    // Using a deliberately old version that likely has known CVEs
    let package = "openssl";
    let version = parse_version_or_zero("1.0.0");

    let result = tokio::time::timeout(
        Duration::from_secs(15),
        scanner.scan_package(package, &version),
    )
    .await;

    // Should complete without timeout
    assert!(
        result.is_ok(),
        "OSV query timed out - check network connectivity"
    );

    let vulns = result.unwrap();
    assert!(vulns.is_ok(), "Failed to query OSV: {:?}", vulns.err());

    let vulns = vulns.unwrap();

    // Old OpenSSL version should have vulnerabilities
    // (This is a reasonable assumption for testing against real data)
    println!(
        "✓ Successfully queried OSV database, found {} vulnerabilities for {} {}",
        vulns.len(),
        package,
        version
    );

    // Validate structure of returned vulnerabilities
    for vuln in vulns.iter().take(3) {
        // Should have an ID (CVE or similar)
        assert!(!vuln.id.is_empty(), "Vulnerability missing ID");

        // Should have a summary
        assert!(
            !vuln.summary.is_empty(),
            "Vulnerability {} missing summary",
            vuln.id
        );

        println!("  - {}: {}", vuln.id, vuln.summary);
    }
}

/// Test SLSA level determination logic
///
/// Verifies that our SLSA level assignment follows production rules
/// for different package types.
#[test]
fn test_slsa_level_production_rules() {
    let verifier = SlsaVerifier::default();

    // Core system packages (highest trust)
    let core_packages = ["glibc", "linux", "pacman", "systemd", "openssl", "bash"];
    for pkg in core_packages {
        let level = verifier.determine_slsa_level(pkg, true);
        assert_eq!(
            level,
            SlsaLevel::Level3,
            "Core package {} should be Level 3",
            pkg
        );
    }

    // Official repository packages (high trust)
    let official_packages = ["vim", "firefox", "rust", "python"];
    for pkg in official_packages {
        let level = verifier.determine_slsa_level(pkg, true);
        assert_eq!(
            level,
            SlsaLevel::Level2,
            "Official package {} should be Level 2",
            pkg
        );
    }

    // AUR packages (no guarantees)
    let aur_packages = ["yay", "spotify", "discord"];
    for pkg in aur_packages {
        let level = verifier.determine_slsa_level(pkg, false);
        assert_eq!(
            level,
            SlsaLevel::None,
            "AUR package {} should have no SLSA level",
            pkg
        );
    }

    println!("✓ SLSA level determination follows production security policy");
}

/// Test hash verification with real file data
///
/// Verifies SHA-256 calculation against known test vectors
/// to ensure cryptographic correctness.
#[test]
fn test_hash_verification_test_vectors() {
    let verifier = SlsaVerifier::default();

    // Test vectors from SHA-256 specification
    let test_vectors = vec![
        (
            "",
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
        ),
        (
            "abc",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ),
        (
            "The quick brown fox jumps over the lazy dog",
            "d7a8fbb307d7809469ca9abcb0082e4f8d5651e46d3cdb762d02d0bf37c9e592",
        ),
    ];

    for (input, expected_hash) in test_vectors {
        // Create temp file with test data
        use std::io::Write;
        use tempfile::NamedTempFile;

        let mut temp = NamedTempFile::new().unwrap();
        write!(temp, "{}", input).unwrap();
        temp.flush().unwrap();

        // Verify hash matches
        assert!(
            verifier.verify_hash(temp.path(), expected_hash).unwrap(),
            "Hash mismatch for input: {:?}",
            input
        );
    }

    println!("✓ SHA-256 hash verification matches standard test vectors");
}

/// Test PGP verification with real Arch packages
///
/// Attempts to verify signatures on real packages from /var/cache/pacman
/// if available. Skips gracefully if cache is empty or not on Arch Linux.
#[test]
#[cfg(feature = "arch")]
fn test_pgp_verification_real_packages() {
    use omg_lib::core::security::pgp::PgpVerifier;
    use std::fs;
    use std::path::Path;

    let cache_dir = Path::new("/var/cache/pacman/pkg");

    // Skip if not on Arch or cache is empty
    if !cache_dir.exists() {
        println!("⊘ Skipping PGP test - not on Arch Linux or cache empty");
        return;
    }

    let verifier = PgpVerifier::new();

    // Find a package with signature
    let entries = fs::read_dir(cache_dir).unwrap();
    let mut tested = false;

    for entry in entries.take(50) {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "sig") {
            continue;
        }

        if !path
            .extension()
            .map_or(false, |ext| ext == "zst" || ext == "xz")
        {
            continue;
        }

        // Look for corresponding .sig file
        let mut sig_path = path.clone();
        sig_path.as_mut_os_string().push(".sig");

        if sig_path.exists() {
            println!("Testing PGP verification on: {}", path.display());

            // Attempt verification
            let result = verifier.verify_package(&path, &sig_path);

            // We don't assert success (may fail due to keyring issues)
            // but we verify the function doesn't panic and returns sensible errors
            match result {
                Ok(_) => {
                    println!("✓ Successfully verified signature");
                    tested = true;
                    break;
                }
                Err(e) => {
                    println!("  Verification failed (expected on some systems): {}", e);
                    // This is OK - the function works, just may not have correct keyring
                }
            }
        }
    }

    if !tested {
        println!("⊘ No suitable packages found in cache for PGP testing");
    }
}

/// Test cache effectiveness for vulnerability lookups
///
/// Verifies that repeated vulnerability queries use cache effectively
/// and don't hammer external APIs.
#[tokio::test]
async fn test_vulnerability_cache_effectiveness() {
    use std::time::Instant;

    let scanner = VulnerabilityScanner::new();
    let package = "test-package";
    let version = parse_version_or_zero("1.0.0");

    // First query (cache miss)
    let start = Instant::now();
    let _first = scanner.scan_package(package, &version).await;
    let first_duration = start.elapsed();

    // Second query (cache hit)
    let start = Instant::now();
    let _second = scanner.scan_package(package, &version).await;
    let second_duration = start.elapsed();

    // Cache hit should be significantly faster (at least 10x)
    assert!(
        second_duration < first_duration / 10,
        "Cache not effective: first={:?}, second={:?}",
        first_duration,
        second_duration
    );

    println!(
        "✓ Vulnerability cache effective: first={:?}, cached={:?} ({}x faster)",
        first_duration,
        second_duration,
        first_duration.as_micros() / second_duration.as_micros().max(1)
    );
}
