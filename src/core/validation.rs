//! Input validation utilities for security
//!
//! Provides validation functions to prevent injection attacks and
//! ensure data integrity.

use anyhow::{Result, bail};

/// Validate a package name to prevent command injection
///
/// Package names should only contain:
/// - Alphanumeric characters (a-z, A-Z, 0-9)
/// - Hyphens (-)
/// - Underscores (_)
/// - Periods (.)
/// - Plus signs (+)
/// - At symbols (@) for scoped packages
/// - Forward slashes (/) for npm scoped packages like @angular/cli
///
/// This prevents shell metacharacters that could be used for injection attacks.
pub fn validate_package_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Package name cannot be empty");
    }

    // Maximum reasonable package name length (Arch Linux limit is 255, but be more conservative)
    const MAX_LENGTH: usize = 200;
    if name.len() > MAX_LENGTH {
        bail!("Package name too long (max {} characters)", MAX_LENGTH);
    }

    // Check for dangerous characters
    for c in name.chars() {
        match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '+' | '@' | '/' => {
                // Valid character (/ allowed for npm scoped packages like @angular/cli)
            }
            _ => {
                bail!(
                    "Invalid character '{}' in package name. Only alphanumeric, -, _, ., +, @, / allowed",
                    c
                );
            }
        }
    }

    // Prevent path traversal attempts
    if name.contains("..") {
        bail!("Package name cannot contain '..'");
    }

    // Prevent absolute paths
    if name.starts_with('/') {
        bail!("Package name cannot start with '/'");
    }

    Ok(())
}

/// Validate multiple package names
pub fn validate_package_names(names: &[String]) -> Result<()> {
    for name in names {
        validate_package_name(name)?;
    }
    Ok(())
}

/// Sanitize a package name by removing invalid characters
/// Use this when you need to accept user input but ensure it's safe
#[must_use]
pub fn sanitize_package_name(name: &str) -> String {
    name.chars()
        .filter(|c| matches!(c, 'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '+' | '@' | '/'))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_package_names() {
        assert!(validate_package_name("firefox").is_ok());
        assert!(validate_package_name("python-requests").is_ok());
        assert!(validate_package_name("lib32-gcc-libs").is_ok());
        assert!(validate_package_name("code-1.85.0-1").is_ok());
        assert!(validate_package_name("@angular/cli").is_ok());
        assert!(validate_package_name("rust_analyzer").is_ok());
        assert!(validate_package_name("foo+bar").is_ok());
    }

    #[test]
    fn test_invalid_package_names() {
        // Shell metacharacters
        assert!(validate_package_name("foo;rm -rf /").is_err());
        assert!(validate_package_name("foo|cat /etc/passwd").is_err());
        assert!(validate_package_name("foo&&evil").is_err());
        assert!(validate_package_name("foo$(whoami)").is_err());
        assert!(validate_package_name("foo`whoami`").is_err());
        assert!(validate_package_name("foo>evil").is_err());
        assert!(validate_package_name("foo<evil").is_err());

        // Path traversal
        assert!(validate_package_name("../../../etc/passwd").is_err());
        assert!(validate_package_name("foo/../bar").is_err());

        // Absolute paths
        assert!(validate_package_name("/etc/passwd").is_err());

        // Empty
        assert!(validate_package_name("").is_err());

        // Too long
        let long_name = "a".repeat(201);
        assert!(validate_package_name(&long_name).is_err());
    }

    #[test]
    fn test_sanitize_package_name() {
        assert_eq!(sanitize_package_name("foo;bar"), "foobar");
        assert_eq!(sanitize_package_name("foo&&bar"), "foobar");
        assert_eq!(sanitize_package_name("foo-bar_baz.1+2@org"), "foo-bar_baz.1+2@org");
    }
}
