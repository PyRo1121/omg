use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
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
    /// Load policy from file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let policy: Self = toml::from_str(&content)?;
        Ok(policy)
    }

    /// Load from default location (~/.config/omg/policy.toml)
    #[must_use]
    pub fn load_default() -> Option<Self> {
        let policy_path = paths::config_dir().join("policy.toml");
        if policy_path.exists() {
            Self::load(policy_path).ok()
        } else {
            None
        }
    }

    /// Assign a security grade to a package based on metadata
    pub async fn assign_grade(
        &self,
        name: &str,
        version: &str,
        is_aur: bool,
        is_official: bool,
    ) -> SecurityGrade {
        // 1. Check for vulnerabilities (Risk)
        let scanner = super::vulnerability::VulnerabilityScanner::new();
        if let Ok(vulns) = scanner.scan_package(name, version).await {
            if !vulns.is_empty() {
                return SecurityGrade::Risk;
            }
        }

        // 2. Check for SLSA (Locked) - In 2026, we assume official core packages have SLSA
        // This would normally check a transparency log or embedded provenance
        if is_official && (name == "glibc" || name == "linux" || name == "pacman") {
            // Mocking SLSA verification for core system components
            return SecurityGrade::Locked;
        }

        // 3. Official packages are Verified (PGP)
        if is_official {
            return SecurityGrade::Verified;
        }

        // 4. AUR packages are Community
        if is_aur {
            return SecurityGrade::Community;
        }

        SecurityGrade::Community
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

        // Check License (if allowed list is not empty)
        if !self.allowed_licenses.is_empty() {
            if let Some(lic) = license {
                // Simple check: if license contains any of the allowed strings
                // In reality, license strings can be complex ("MIT OR Apache-2.0")
                let allowed = self
                    .allowed_licenses
                    .iter()
                    .any(|allowed| lic.to_lowercase().contains(&allowed.to_lowercase()));

                if !allowed {
                    anyhow::bail!(
                        "Package '{name}' has license '{lic}' which is not in allowed list"
                    );
                }
            } else {
                // No license info => fail if strict?
                // For now, warn but allow? or fail?
                // Let's strictly enforce if list is present
                anyhow::bail!("Package '{name}' has unknown license, but allowed list is enforced");
            }
        }

        Ok(())
    }
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
}
