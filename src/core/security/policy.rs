//! Security policy enforcement and package security grading
//!
//! Defines security policies for package approval/rejection based on
//! vulnerabilities, licenses, and trust levels with A-F grading.

use crate::package_managers::types::Version;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

use crate::core::paths;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SecurityGrade {
    Risk = 0,      // Known vulnerabilities
    Community = 1, // AUR/Unsigned
    Verified = 2,  // PGP or Checksum
    Locked = 3,    // SLSA + PGP
}

impl std::fmt::Display for SecurityGrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Locked => write!(f, "LOCKED (SLSA + PGP)"),
            Self::Verified => write!(f, "VERIFIED (PGP/Checksum)"),
            Self::Community => write!(f, "COMMUNITY (AUR/Unsigned)"),
            Self::Risk => write!(f, "RISK (Vulnerabilities)"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SecurityPolicy {
    #[serde(default = "default_minimum_grade")]
    pub minimum_grade: SecurityGrade,
    #[serde(default = "default_true")]
    pub allow_aur: bool,
    #[serde(default)]
    pub require_pgp: bool,
    #[serde(default)]
    pub allowed_licenses: Vec<String>,
    #[serde(default)]
    pub banned_packages: Vec<String>,
}

const fn default_minimum_grade() -> SecurityGrade {
    SecurityGrade::Community
}

const fn default_true() -> bool {
    true
}

fn is_not_found(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<io::Error>()
            .is_some_and(|io_error| io_error.kind() == io::ErrorKind::NotFound)
    })
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            minimum_grade: SecurityGrade::Community,
            allow_aur: true,
            require_pgp: false,
            allowed_licenses: Vec::new(),
            banned_packages: Vec::new(),
        }
    }
}

impl SecurityPolicy {
    /// Load policy from file. A missing file is not handled here; callers that
    /// want defaults for an absent policy should use [`Self::load_optional`].
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read security policy: {}", path.display()))?;
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse security policy: {}", path.display()))
    }

    /// Load a policy file, using the built-in default only when the file is absent.
    pub fn load_optional<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        match Self::load(path) {
            Ok(policy) => Ok(policy),
            Err(error) if is_not_found(&error) => Ok(Self::default()),
            Err(error) => Err(error),
        }
    }

    /// Load from default location (~/.config/omg/policy.toml).
    /// A missing file uses the built-in default; a corrupt or unreadable file fails closed.
    pub fn load_default() -> Result<Self> {
        Self::load_optional(paths::config_dir().join("policy.toml"))
    }

    /// Assign a security grade to a package based on metadata
    pub async fn assign_grade(
        &self,
        scanner: &dyn super::vulnerability::VulnerabilitySource,
        name: &str,
        version: &Version,
        is_aur: bool,
        is_official: bool,
    ) -> Result<SecurityGrade> {
        // 1. Check for vulnerabilities (Risk). An unavailable evidence source
        // must not be treated as a clean package.
        if !scanner.scan_package(name, version).await?.is_empty() {
            return Ok(SecurityGrade::Risk);
        }

        // Official packages are Verified (signed repository metadata). A Locked
        // grade requires provenance evidence, which this function does not have.
        if is_official {
            return Ok(SecurityGrade::Verified);
        }

        // AUR and other unsigned sources are Community
        if is_aur {
            return Ok(SecurityGrade::Community);
        }

        Ok(SecurityGrade::Community)
    }

    /// Check if a package is allowed by policy
    pub fn check_package(
        &self,
        name: &str,
        is_aur: bool,
        license: Option<&str>,
        grade: SecurityGrade,
    ) -> Result<()> {
        // Check Grade
        if grade < self.minimum_grade {
            anyhow::bail!(
                "Security Grade '{}' for '{}' is below required minimum '{}'",
                grade,
                name,
                self.minimum_grade
            );
        }

        // Check if banned
        if self.banned_packages.contains(&name.to_string()) {
            anyhow::bail!("Package '{name}' is banned by security policy");
        }

        // Check AUR
        if is_aur && !self.allow_aur {
            anyhow::bail!("Package '{name}' is from AUR, which is disabled by security policy");
        }

        if self.require_pgp && grade < SecurityGrade::Verified {
            anyhow::bail!(
                "Package '{name}' is unsigned; security policy requires PGP or checksum verification"
            );
        }

        // Check License (if allowed list is not empty)
        if !self.allowed_licenses.is_empty() {
            if let Some(lic) = license {
                if !license_matches_allowlist(lic, &self.allowed_licenses) {
                    anyhow::bail!(
                        "Package '{name}' has license '{lic}' which is not in allowed list"
                    );
                }
            } else {
                anyhow::bail!("Package '{name}' has unknown license, but allowed list is enforced");
            }
        }

        Ok(())
    }
}

/// SPDX-ish tokens from a license expression (operators and punctuation dropped).
pub fn spdx_license_tokens(license: &str) -> Vec<String> {
    license
        .split(|c: char| !(c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')))
        .filter(|token| !token.is_empty())
        .filter(|token| {
            !matches!(
                token.to_ascii_uppercase().as_str(),
                "AND" | "OR" | "WITH" | "TO"
            )
        })
        .map(str::to_ascii_lowercase)
        .collect()
}

/// True when `license` contains an allowed SPDX identifier as a whole token.
pub fn license_matches_allowlist(license: &str, allowed: &[String]) -> bool {
    let tokens = spdx_license_tokens(license);
    allowed.iter().any(|allowed| {
        let needle = allowed.to_ascii_lowercase();
        tokens.iter().any(|token| {
            token == &needle
                || token.strip_suffix('+') == Some(needle.as_str())
                || needle.strip_suffix('+') == Some(token.as_str())
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grade_ordering() {
        assert!(SecurityGrade::Locked > SecurityGrade::Verified);
        assert!(SecurityGrade::Verified > SecurityGrade::Community);
        assert!(SecurityGrade::Community > SecurityGrade::Risk);
    }

    #[test]
    fn test_policy_check_grade() {
        let policy = SecurityPolicy {
            minimum_grade: SecurityGrade::Verified,
            ..SecurityPolicy::default()
        };

        // Verified is allowed
        assert!(
            policy
                .check_package("test", false, None, SecurityGrade::Verified)
                .is_ok()
        );

        // Locked is allowed
        assert!(
            policy
                .check_package("test", false, None, SecurityGrade::Locked)
                .is_ok()
        );

        // Community is blocked
        assert!(
            policy
                .check_package("test", true, None, SecurityGrade::Community)
                .is_err()
        );
    }

    #[test]
    fn load_optional_uses_defaults_when_policy_is_missing() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let policy = SecurityPolicy::load_optional(temp.path().join("policy.toml"))
            .expect("missing policy should use defaults");
        assert_eq!(policy.minimum_grade, SecurityGrade::Community);
        assert!(policy.allow_aur);
    }

    #[test]
    fn load_optional_rejects_corrupt_policy() {
        let temp = tempfile::TempDir::new().expect("temp dir");
        let path = temp.path().join("policy.toml");
        fs::write(&path, "minimum_grade = [").expect("write corrupt policy");
        let error = SecurityPolicy::load_optional(&path).expect_err("corrupt policy must fail");
        assert!(
            error
                .to_string()
                .contains("Failed to parse security policy")
        );
    }

    struct EmptyVulns;

    impl super::super::vulnerability::VulnerabilitySource for EmptyVulns {
        fn scan_package<'a>(
            &'a self,
            _name: &'a str,
            _version: &'a crate::package_managers::types::Version,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            Vec<crate::core::security::vulnerability::VulnerabilityReport>,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    struct FailingVulns;

    impl super::super::vulnerability::VulnerabilitySource for FailingVulns {
        fn scan_package<'a>(
            &'a self,
            _name: &'a str,
            _version: &'a crate::package_managers::types::Version,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<
                        Output = Result<
                            Vec<crate::core::security::vulnerability::VulnerabilityReport>,
                        >,
                    > + Send
                    + 'a,
            >,
        > {
            Box::pin(async { anyhow::bail!("osv unavailable") })
        }
    }

    #[tokio::test]
    async fn official_packages_are_verified_not_locked_by_name() {
        let policy = SecurityPolicy::default();
        let version = crate::package_managers::parse_version_or_zero("2.40");
        let grade = policy
            .assign_grade(&EmptyVulns, "glibc", &version, false, true)
            .await
            .expect("empty vuln source");
        assert_eq!(
            grade,
            SecurityGrade::Verified,
            "package names must not mint a Locked SLSA grade"
        );
    }

    #[tokio::test]
    async fn aur_packages_are_community() {
        let policy = SecurityPolicy::default();
        let version = crate::package_managers::parse_version_or_zero("1.0");
        let grade = policy
            .assign_grade(&EmptyVulns, "yay", &version, true, false)
            .await
            .expect("empty vuln source");
        assert_eq!(grade, SecurityGrade::Community);
    }

    #[tokio::test]
    async fn unavailable_vuln_source_fails_closed() {
        let policy = SecurityPolicy::default();
        let version = crate::package_managers::parse_version_or_zero("1.0");
        let error = policy
            .assign_grade(&FailingVulns, "vim", &version, false, true)
            .await
            .expect_err("missing evidence must not look like a clean package");
        assert!(
            error.to_string().contains("osv unavailable"),
            "scanner error must be preserved, got: {error}"
        );
    }

    #[test]
    fn require_pgp_rejects_unsigned_packages() {
        let policy = SecurityPolicy {
            require_pgp: true,
            ..SecurityPolicy::default()
        };
        let err = policy
            .check_package("yay", true, Some("MIT"), SecurityGrade::Community)
            .expect_err("AUR community packages are unsigned");
        assert!(
            err.to_string().contains("unsigned"),
            "require_pgp must mention missing verification, got: {err}"
        );
        assert!(
            policy
                .check_package("vim", false, Some("MIT"), SecurityGrade::Verified)
                .is_ok()
        );
    }

    #[test]
    fn allowed_license_matches_spdx_tokens_not_substrings() {
        let policy = SecurityPolicy {
            allowed_licenses: vec!["MIT".to_string()],
            ..SecurityPolicy::default()
        };
        let err = policy
            .check_package("foo", false, Some("LIMITED"), SecurityGrade::Verified)
            .expect_err("LIMITED must not match MIT");
        assert!(
            err.to_string().contains("not in allowed list"),
            "got: {err}"
        );
        assert!(
            policy
                .check_package(
                    "foo",
                    false,
                    Some("MIT OR Apache-2.0"),
                    SecurityGrade::Verified
                )
                .is_ok()
        );
    }
}
